//! Policy repository -- CRUD for policies and service-account-policy bindings.

use sqlx::PgPool;

use crate::models::{
    CreatePolicy, Cursor, Page, Policy, ServiceAccountPolicyBinding, UpdatePolicy, clamp_limit,
};
use crate::storage::StorageError;
use crate::types::{PolicyId, ServiceAccountId, TenantId};

/// Repository for the `policies` and `service_account_policy_bindings` tables.
#[derive(Clone)]
pub struct PolicyRepo {
    pool: PgPool,
}

impl PolicyRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Insert a new policy and return the full row.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Database`] on connection or query failure.
    pub async fn create(&self, input: &CreatePolicy) -> Result<Policy, StorageError> {
        sqlx::query_as::<_, Policy>(
            r"
            INSERT INTO policies (
                id, tenant_id, name, description,
                allowed_models_json, denied_models_json,
                max_input_tokens, max_output_tokens,
                allow_streaming, allow_tools, allow_files,
                rpm_limit, concurrency_limit
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            RETURNING *
            ",
        )
        .bind(input.id)
        .bind(input.tenant_id)
        .bind(&input.name)
        .bind(&input.description)
        .bind(&input.allowed_models_json)
        .bind(&input.denied_models_json)
        .bind(input.max_input_tokens)
        .bind(input.max_output_tokens)
        .bind(input.allow_streaming)
        .bind(input.allow_tools)
        .bind(input.allow_files)
        .bind(input.rpm_limit)
        .bind(input.concurrency_limit)
        .fetch_one(&self.pool)
        .await
        .map_err(StorageError::from)
    }

    /// Fetch a single policy by tenant + id.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::NotFound`] if no matching row exists.
    pub async fn get_by_id(
        &self,
        tenant_id: TenantId,
        id: PolicyId,
    ) -> Result<Policy, StorageError> {
        sqlx::query_as::<_, Policy>("SELECT * FROM policies WHERE id = $1 AND tenant_id = $2")
            .bind(id)
            .bind(tenant_id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| StorageError::not_found("policy", "id", id))
    }

    /// List policies for a tenant with cursor-based pagination.
    ///
    /// Results are ordered by `(created_at, id)` ascending.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Database`] on connection or query failure.
    pub async fn list_by_tenant(
        &self,
        tenant_id: TenantId,
        cursor: Option<&Cursor>,
        limit: i64,
    ) -> Result<Page<Policy>, StorageError> {
        let (limit, fetch_limit) = clamp_limit(limit);

        let rows = if let Some(c) = cursor {
            let (ts, id) = c.decode().map_err(StorageError::InvalidCursor)?;
            sqlx::query_as::<_, Policy>(
                r"
                SELECT * FROM policies
                WHERE tenant_id = $1
                  AND (created_at, id) > ($2, $3)
                ORDER BY created_at ASC, id ASC
                LIMIT $4
                ",
            )
            .bind(tenant_id)
            .bind(ts)
            .bind(id)
            .bind(fetch_limit)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, Policy>(
                r"
                SELECT * FROM policies
                WHERE tenant_id = $1
                ORDER BY created_at ASC, id ASC
                LIMIT $2
                ",
            )
            .bind(tenant_id)
            .bind(fetch_limit)
            .fetch_all(&self.pool)
            .await?
        };

        Ok(Page::from_rows(rows, limit, |p| (p.created_at, p.id.0)))
    }

    /// Update mutable fields of a policy. Only non-`None` fields are applied.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::NotFound`] if no matching row exists.
    pub async fn update(
        &self,
        tenant_id: TenantId,
        id: PolicyId,
        input: &UpdatePolicy,
    ) -> Result<Policy, StorageError> {
        sqlx::query_as::<_, Policy>(
            r"
            UPDATE policies SET
                name               = COALESCE($3, name),
                description        = COALESCE($4, description),
                allowed_models_json = COALESCE($5, allowed_models_json),
                denied_models_json  = COALESCE($6, denied_models_json),
                max_input_tokens   = COALESCE($7, max_input_tokens),
                max_output_tokens  = COALESCE($8, max_output_tokens),
                allow_streaming    = COALESCE($9, allow_streaming),
                allow_tools        = COALESCE($10, allow_tools),
                allow_files        = COALESCE($11, allow_files),
                rpm_limit          = COALESCE($12, rpm_limit),
                concurrency_limit  = COALESCE($13, concurrency_limit)
            WHERE id = $1 AND tenant_id = $2
            RETURNING *
            ",
        )
        .bind(id)
        .bind(tenant_id)
        .bind(&input.name)
        .bind(&input.description)
        .bind(&input.allowed_models_json)
        .bind(&input.denied_models_json)
        .bind(input.max_input_tokens)
        .bind(input.max_output_tokens)
        .bind(input.allow_streaming)
        .bind(input.allow_tools)
        .bind(input.allow_files)
        .bind(input.rpm_limit)
        .bind(input.concurrency_limit)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            if matches!(&e, sqlx::Error::RowNotFound) {
                StorageError::not_found("policy", "id", id)
            } else {
                StorageError::from(e)
            }
        })
    }

    /// Delete a policy by tenant + id.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::NotFound`] if no matching row exists, or
    /// [`StorageError::ReferenceError`] if the policy still has bindings
    /// (PG foreign-key violation 23503).
    pub async fn delete(&self, tenant_id: TenantId, id: PolicyId) -> Result<(), StorageError> {
        let result = sqlx::query("DELETE FROM policies WHERE id = $1 AND tenant_id = $2")
            .bind(id)
            .bind(tenant_id)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(StorageError::not_found("policy", "id", id));
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Binding operations
    // -----------------------------------------------------------------------

    /// Create a binding between a service account and a policy.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Conflict`] if the binding already exists.
    pub async fn create_binding(
        &self,
        service_account_id: ServiceAccountId,
        policy_id: PolicyId,
    ) -> Result<ServiceAccountPolicyBinding, StorageError> {
        sqlx::query_as::<_, ServiceAccountPolicyBinding>(
            r"
            INSERT INTO service_account_policy_bindings (service_account_id, policy_id)
            VALUES ($1, $2)
            RETURNING *
            ",
        )
        .bind(service_account_id)
        .bind(policy_id)
        .fetch_one(&self.pool)
        .await
        .map_err(StorageError::from)
    }

    /// Remove a binding between a service account and a policy.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::NotFound`] if no matching binding exists.
    pub async fn delete_binding(
        &self,
        service_account_id: ServiceAccountId,
        policy_id: PolicyId,
    ) -> Result<(), StorageError> {
        let result = sqlx::query(
            "DELETE FROM service_account_policy_bindings WHERE service_account_id = $1 AND policy_id = $2",
        )
        .bind(service_account_id)
        .bind(policy_id)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(StorageError::not_found(
                "policy_binding",
                "service_account_id+policy_id",
                format!("{service_account_id}+{policy_id}"),
            ));
        }
        Ok(())
    }

    /// List all policies bound to a service account, optionally including a
    /// default policy that may not have an explicit binding.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Database`] on connection or query failure.
    pub async fn list_policies_for_service_account(
        &self,
        service_account_id: ServiceAccountId,
        default_policy_id: Option<PolicyId>,
    ) -> Result<Vec<Policy>, StorageError> {
        let mut policies: Vec<Policy> = sqlx::query_as::<_, Policy>(
            r"
            SELECT p.*
            FROM policies p
            JOIN service_account_policy_bindings b ON p.id = b.policy_id
            WHERE b.service_account_id = $1
            ",
        )
        .bind(service_account_id)
        .fetch_all(&self.pool)
        .await?;

        // If a default policy is specified and not already in the result, add it.
        if let Some(default_id) = default_policy_id {
            let already_present = policies.iter().any(|p| p.id == default_id);
            if !already_present {
                let default_policy =
                    sqlx::query_as::<_, Policy>("SELECT * FROM policies WHERE id = $1")
                        .bind(default_id)
                        .fetch_optional(&self.pool)
                        .await?;

                if let Some(dp) = default_policy {
                    policies.push(dp);
                }
            }
        }

        Ok(policies)
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ServiceAccount, Tenant};
    use sqlx::PgPool;

    // -----------------------------------------------------------------------
    // Test helpers
    // -----------------------------------------------------------------------

    async fn create_test_tenant(pool: &PgPool) -> Tenant {
        sqlx::query_as::<_, Tenant>("INSERT INTO tenants (name, slug) VALUES ($1, $2) RETURNING *")
            .bind(format!("Test {}", uuid::Uuid::new_v4()))
            .bind(format!("test-{}", uuid::Uuid::new_v4()))
            .fetch_one(pool)
            .await
            .expect("create tenant")
    }

    async fn create_test_sa(pool: &PgPool, tenant_id: TenantId) -> ServiceAccount {
        sqlx::query_as::<_, ServiceAccount>(
            "INSERT INTO service_accounts (tenant_id, name, slug, environment) VALUES ($1, $2, $3, 'dev') RETURNING *",
        )
        .bind(tenant_id)
        .bind(format!("SA {}", uuid::Uuid::new_v4()))
        .bind(format!("sa-{}", uuid::Uuid::new_v4()))
        .fetch_one(pool)
        .await
        .expect("create sa")
    }

    fn test_create_input(tenant_id: TenantId) -> CreatePolicy {
        CreatePolicy {
            id: PolicyId::new(),
            tenant_id,
            name: format!("policy-{}", uuid::Uuid::new_v4()),
            description: Some("test policy".into()),
            allowed_models_json: serde_json::json!(["gpt-4"]),
            denied_models_json: serde_json::json!([]),
            max_input_tokens: Some(4096),
            max_output_tokens: Some(2048),
            allow_streaming: true,
            allow_tools: false,
            allow_files: false,
            rpm_limit: Some(100),
            concurrency_limit: Some(10),
        }
    }

    // -----------------------------------------------------------------------
    // Tests
    // -----------------------------------------------------------------------

    #[sqlx::test(migrations = "../../migrations")]
    async fn create_and_get(pool: PgPool) {
        let tenant = create_test_tenant(&pool).await;
        let repo = PolicyRepo::new(pool);

        let input = test_create_input(tenant.id);
        let created = repo.create(&input).await.expect("create policy");

        assert_eq!(created.name, input.name);
        assert_eq!(created.tenant_id, tenant.id);
        assert_eq!(created.max_input_tokens, Some(4096));
        assert!(created.allow_streaming);

        let fetched = repo
            .get_by_id(tenant.id, created.id)
            .await
            .expect("get policy");
        assert_eq!(fetched.id, created.id);
        assert_eq!(fetched.name, created.name);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn list_by_tenant(pool: PgPool) {
        let tenant = create_test_tenant(&pool).await;
        let repo = PolicyRepo::new(pool);

        // Create 3 policies
        for _ in 0..3 {
            repo.create(&test_create_input(tenant.id))
                .await
                .expect("create policy");
        }

        // List first page (limit 2)
        let page1 = repo
            .list_by_tenant(tenant.id, None, 2)
            .await
            .expect("list page 1");
        assert_eq!(page1.items.len(), 2);
        assert!(page1.next_cursor.is_some());

        // List second page using cursor
        let page2 = repo
            .list_by_tenant(tenant.id, page1.next_cursor.as_ref(), 2)
            .await
            .expect("list page 2");
        assert_eq!(page2.items.len(), 1);
        assert!(page2.next_cursor.is_none());

        // Ensure no overlap
        let ids1: Vec<_> = page1.items.iter().map(|p| p.id).collect();
        let ids2: Vec<_> = page2.items.iter().map(|p| p.id).collect();
        for id in &ids2 {
            assert!(!ids1.contains(id), "pages must not overlap");
        }
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn update_policy(pool: PgPool) {
        let tenant = create_test_tenant(&pool).await;
        let repo = PolicyRepo::new(pool);

        let created = repo
            .create(&test_create_input(tenant.id))
            .await
            .expect("create policy");

        let update = UpdatePolicy {
            name: Some("updated-name".into()),
            description: None,
            allowed_models_json: None,
            denied_models_json: None,
            max_input_tokens: Some(8192),
            max_output_tokens: None,
            allow_streaming: None,
            allow_tools: Some(true),
            allow_files: None,
            rpm_limit: None,
            concurrency_limit: None,
        };

        let updated = repo
            .update(tenant.id, created.id, &update)
            .await
            .expect("update policy");

        assert_eq!(updated.name, "updated-name");
        assert_eq!(updated.max_input_tokens, Some(8192));
        assert!(updated.allow_tools);
        // Unchanged fields should keep original values
        assert!(updated.allow_streaming);
        assert_eq!(updated.max_output_tokens, Some(2048));
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn delete_policy(pool: PgPool) {
        let tenant = create_test_tenant(&pool).await;
        let repo = PolicyRepo::new(pool.clone());

        let created = repo
            .create(&test_create_input(tenant.id))
            .await
            .expect("create policy");

        repo.delete(tenant.id, created.id)
            .await
            .expect("delete policy");

        let err = repo.get_by_id(tenant.id, created.id).await;
        assert!(
            matches!(err, Err(StorageError::NotFound { .. })),
            "should be not found after deletion"
        );
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn create_and_delete_binding(pool: PgPool) {
        let tenant = create_test_tenant(&pool).await;
        let sa = create_test_sa(&pool, tenant.id).await;
        let repo = PolicyRepo::new(pool);

        let policy = repo
            .create(&test_create_input(tenant.id))
            .await
            .expect("create policy");

        // Create binding
        let binding = repo
            .create_binding(sa.id, policy.id)
            .await
            .expect("create binding");
        assert_eq!(binding.service_account_id, sa.id);
        assert_eq!(binding.policy_id, policy.id);

        // Duplicate binding should conflict
        let dup = repo.create_binding(sa.id, policy.id).await;
        assert!(
            matches!(dup, Err(StorageError::Conflict { .. })),
            "duplicate binding should conflict"
        );

        // Delete binding
        repo.delete_binding(sa.id, policy.id)
            .await
            .expect("delete binding");

        // Deleting again should be NotFound
        let err = repo.delete_binding(sa.id, policy.id).await;
        assert!(
            matches!(err, Err(StorageError::NotFound { .. })),
            "already-deleted binding should be not found"
        );
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn list_policies_for_service_account_with_default(pool: PgPool) {
        let tenant = create_test_tenant(&pool).await;
        let sa = create_test_sa(&pool, tenant.id).await;
        let repo = PolicyRepo::new(pool);

        // Create two policies: one bound, one used as default
        let bound_policy = repo
            .create(&test_create_input(tenant.id))
            .await
            .expect("create bound policy");
        let default_policy = repo
            .create(&test_create_input(tenant.id))
            .await
            .expect("create default policy");

        // Bind only the first one
        repo.create_binding(sa.id, bound_policy.id)
            .await
            .expect("create binding");

        // List with default -- should return both
        let policies = repo
            .list_policies_for_service_account(sa.id, Some(default_policy.id))
            .await
            .expect("list policies");

        assert_eq!(policies.len(), 2);
        let ids: Vec<PolicyId> = policies.iter().map(|p| p.id).collect();
        assert!(ids.contains(&bound_policy.id));
        assert!(ids.contains(&default_policy.id));

        // List without default -- should return only the bound one
        let policies_no_default = repo
            .list_policies_for_service_account(sa.id, None)
            .await
            .expect("list policies no default");
        assert_eq!(policies_no_default.len(), 1);
        assert_eq!(policies_no_default[0].id, bound_policy.id);

        // If default is already bound, should not duplicate
        repo.create_binding(sa.id, default_policy.id)
            .await
            .expect("bind default");
        let policies_both = repo
            .list_policies_for_service_account(sa.id, Some(default_policy.id))
            .await
            .expect("list policies both bound");
        assert_eq!(policies_both.len(), 2);
    }
}

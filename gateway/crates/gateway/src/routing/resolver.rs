//! Model alias resolution -- resolves a model alias to ordered provider routes.
//!
//! Uses the [`RouteCache`] with a database fallback via [`RouteRepo`].

use std::sync::Arc;

use crate::error::GatewayError;
use crate::models::ProviderRoute;
use crate::routing::router::RouteCache;
use crate::storage::repositories::routes::RouteRepo;
use crate::types::TenantId;

/// Resolves model aliases to ordered lists of provider routes.
///
/// This is a stateless namespace — all methods are associated functions
/// that accept their dependencies explicitly, making them easy to test.
pub struct RouteResolver;

impl RouteResolver {
    /// Resolve a model alias to an ordered list of provider routes.
    ///
    /// Resolution uses the route cache with a database fallback. The returned
    /// routes are ordered tenant-scoped first, then by ascending priority
    /// (lower number = higher priority).
    ///
    /// # Errors
    ///
    /// - `GatewayError::Routing` with code `no_route` if no enabled routes
    ///   exist for the given alias.
    /// - `GatewayError::Routing` with code `route_cache_load_failed` if the
    ///   database query fails.
    #[tracing::instrument(
        name = "routing.resolve",
        skip_all,
        fields(%tenant_id, model_alias)
    )]
    pub async fn resolve(
        cache: &RouteCache,
        repo: &RouteRepo,
        tenant_id: TenantId,
        model_alias: &str,
    ) -> Result<Arc<Vec<ProviderRoute>>, GatewayError> {
        let alias = model_alias.to_owned();
        let routes = cache
            .get_or_load(tenant_id, model_alias, || {
                let repo = repo.clone();
                let alias = alias.clone();
                async move {
                    repo.resolve_routes(tenant_id, &alias).await.map_err(|e| {
                        tracing::error!(
                            %tenant_id,
                            model_alias = alias.as_str(),
                            error = %e,
                            "failed to resolve routes from database"
                        );
                        GatewayError::routing(
                            "route_resolve_failed",
                            "failed to resolve routes from database",
                        )
                    })
                }
            })
            .await?;

        if routes.is_empty() {
            return Err(GatewayError::routing(
                "no_route",
                format!("no enabled routes for model alias '{model_alias}'"),
            ));
        }

        Ok(routes)
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::CreateRoute;
    use crate::types::TenantId;

    // -----------------------------------------------------------------------
    // Integration tests (require database)
    // -----------------------------------------------------------------------

    async fn create_test_tenant(pool: &sqlx::PgPool) -> crate::models::Tenant {
        sqlx::query_as::<_, crate::models::Tenant>(
            "INSERT INTO tenants (name, slug) VALUES ($1, $2) RETURNING *",
        )
        .bind(format!("Test {}", uuid::Uuid::new_v4()))
        .bind(format!("test-{}", uuid::Uuid::new_v4()))
        .fetch_one(pool)
        .await
        .expect("create tenant")
    }

    fn test_route_input(
        tenant_id: Option<TenantId>,
        alias: &str,
        provider_model: &str,
        priority: i32,
    ) -> CreateRoute {
        CreateRoute {
            tenant_id,
            model_alias: alias.to_owned(),
            provider: "openai".to_owned(),
            provider_model_name: provider_model.to_owned(),
            priority: Some(priority),
            enabled: Some(true),
            timeout_ms: Some(30000),
            max_retries: Some(1),
            retry_backoff_ms: Some(250),
            config: Some(serde_json::json!({})),
        }
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn resolve_returns_routes_in_priority_order(pool: sqlx::PgPool) {
        let tenant = create_test_tenant(&pool).await;
        let repo = RouteRepo::new(pool);
        let cache = RouteCache::new(60);

        repo.create(&test_route_input(
            Some(tenant.id),
            "gpt-4",
            "gpt-4-low",
            200,
        ))
        .await
        .expect("create low");
        repo.create(&test_route_input(
            Some(tenant.id),
            "gpt-4",
            "gpt-4-high",
            10,
        ))
        .await
        .expect("create high");
        repo.create(&test_route_input(Some(tenant.id), "gpt-4", "gpt-4-mid", 50))
            .await
            .expect("create mid");

        let routes = RouteResolver::resolve(&cache, &repo, tenant.id, "gpt-4")
            .await
            .expect("should resolve");

        assert_eq!(routes.len(), 3);
        assert_eq!(routes[0].provider_model_name, "gpt-4-high");
        assert_eq!(routes[1].provider_model_name, "gpt-4-mid");
        assert_eq!(routes[2].provider_model_name, "gpt-4-low");
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn resolve_returns_error_when_no_routes(pool: sqlx::PgPool) {
        let tenant = create_test_tenant(&pool).await;
        let repo = RouteRepo::new(pool);
        let cache = RouteCache::new(60);

        let result = RouteResolver::resolve(&cache, &repo, tenant.id, "nonexistent").await;

        assert!(result.is_err());
        match result.unwrap_err() {
            GatewayError::Routing { code, .. } => assert_eq!(code, "no_route"),
            other => panic!("expected Routing error, got: {other:?}"),
        }
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn resolve_includes_global_and_tenant_routes(pool: sqlx::PgPool) {
        let tenant = create_test_tenant(&pool).await;
        let repo = RouteRepo::new(pool);
        let cache = RouteCache::new(60);

        // Global route
        repo.create(&test_route_input(None, "gpt-4", "gpt-4-global", 10))
            .await
            .expect("create global");

        // Tenant-scoped route
        repo.create(&test_route_input(
            Some(tenant.id),
            "gpt-4",
            "gpt-4-scoped",
            50,
        ))
        .await
        .expect("create scoped");

        let routes = RouteResolver::resolve(&cache, &repo, tenant.id, "gpt-4")
            .await
            .expect("should resolve");

        assert_eq!(routes.len(), 2);
        // Tenant-scoped should come first regardless of priority
        assert_eq!(routes[0].provider_model_name, "gpt-4-scoped");
        assert_eq!(routes[1].provider_model_name, "gpt-4-global");
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn resolve_uses_cache_on_second_call(pool: sqlx::PgPool) {
        let tenant = create_test_tenant(&pool).await;
        let repo = RouteRepo::new(pool);
        let cache = RouteCache::new(60);

        repo.create(&test_route_input(
            Some(tenant.id),
            "gpt-4",
            "gpt-4-turbo",
            100,
        ))
        .await
        .expect("create route");

        // First call — cache miss
        let routes1 = RouteResolver::resolve(&cache, &repo, tenant.id, "gpt-4")
            .await
            .expect("should resolve");

        // Second call — should hit cache (same Arc)
        let routes2 = RouteResolver::resolve(&cache, &repo, tenant.id, "gpt-4")
            .await
            .expect("should resolve");

        assert_eq!(routes1.len(), 1);
        assert!(
            Arc::ptr_eq(&routes1, &routes2),
            "second call should return same Arc (cache hit)"
        );
    }
}

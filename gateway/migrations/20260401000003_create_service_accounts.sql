-- Migration 3: Service accounts table
CREATE TABLE IF NOT EXISTS service_accounts (
    id                UUID                   PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id         UUID                   NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    name              TEXT                   NOT NULL,
    slug              TEXT                   NOT NULL,
    environment       TEXT                   NOT NULL,  -- 'dev', 'staging', 'prod'
    description       TEXT,
    status            service_account_status NOT NULL DEFAULT 'active',
    default_policy_id UUID,  -- FK added after policies table exists
    created_at        TIMESTAMPTZ            NOT NULL DEFAULT NOW(),
    updated_at        TIMESTAMPTZ            NOT NULL DEFAULT NOW(),
    UNIQUE (tenant_id, slug)
);

-- Index foreign key columns for join performance
CREATE INDEX IF NOT EXISTS idx_service_accounts_tenant_id ON service_accounts(tenant_id);

-- Auto-update updated_at on row modification
CREATE TRIGGER trg_service_accounts_updated_at
    BEFORE UPDATE ON service_accounts
    FOR EACH ROW
    EXECUTE FUNCTION set_updated_at();

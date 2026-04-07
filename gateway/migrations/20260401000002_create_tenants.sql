-- Migration 2: Tenants table
CREATE TABLE IF NOT EXISTS tenants (
    id            UUID          PRIMARY KEY DEFAULT gen_random_uuid(),
    name          TEXT          NOT NULL,
    slug          TEXT          NOT NULL UNIQUE,
    status        tenant_status NOT NULL DEFAULT 'active',
    metadata      JSONB         NOT NULL DEFAULT '{}'::jsonb,
    created_at    TIMESTAMPTZ   NOT NULL DEFAULT NOW(),
    updated_at    TIMESTAMPTZ   NOT NULL DEFAULT NOW()
);

-- Auto-update updated_at on row modification
CREATE TRIGGER trg_tenants_updated_at
    BEFORE UPDATE ON tenants
    FOR EACH ROW
    EXECUTE FUNCTION set_updated_at();

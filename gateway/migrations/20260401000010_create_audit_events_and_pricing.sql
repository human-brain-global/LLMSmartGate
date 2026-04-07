-- Migration 10: Audit Events & Pricing Rules
-- Audit events are append-only (no updates, no deletes).
-- Pricing rules track per-model cost rates with temporal versioning.

-- ============================================================
-- Audit Events
-- ============================================================
CREATE TABLE IF NOT EXISTS audit_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID REFERENCES tenants(id) ON DELETE CASCADE,  -- nullable for system events
    actor_type TEXT NOT NULL,   -- 'admin_user', 'system', 'api'
    actor_id TEXT NOT NULL,
    action TEXT NOT NULL,       -- 'service_account.created', 'key.revoked', etc.
    target_type TEXT NOT NULL,  -- 'service_account', 'policy', 'route', etc.
    target_id TEXT NOT NULL,
    metadata_json JSONB NOT NULL DEFAULT '{}'::jsonb,  -- before/after state for updates
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
    -- NO updated_at: append-only table
);

CREATE INDEX IF NOT EXISTS idx_audit_events_tenant_id ON audit_events(tenant_id);
CREATE INDEX IF NOT EXISTS idx_audit_events_action ON audit_events(action);
CREATE INDEX IF NOT EXISTS idx_audit_events_target_type ON audit_events(target_type);
CREATE INDEX IF NOT EXISTS idx_audit_events_created_at ON audit_events(created_at DESC);

-- Prevent UPDATE and DELETE on audit_events (append-only enforcement)
CREATE OR REPLACE FUNCTION prevent_audit_modification()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'audit_events table is append-only: % operations are not permitted', TG_OP;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_audit_events_no_update
    BEFORE UPDATE ON audit_events
    FOR EACH ROW
    EXECUTE FUNCTION prevent_audit_modification();

CREATE TRIGGER trg_audit_events_no_delete
    BEFORE DELETE ON audit_events
    FOR EACH ROW
    EXECUTE FUNCTION prevent_audit_modification();

-- ============================================================
-- Pricing Rules
-- ============================================================
CREATE TABLE IF NOT EXISTS pricing_rules (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    provider TEXT NOT NULL,
    model_name TEXT NOT NULL,
    input_price_per_1k NUMERIC(18,8) NOT NULL,
    output_price_per_1k NUMERIC(18,8) NOT NULL,
    effective_from TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (provider, model_name, effective_from)
);

CREATE INDEX IF NOT EXISTS idx_pricing_rules_provider_model ON pricing_rules(provider, model_name);

CREATE TRIGGER trg_pricing_rules_updated_at
    BEFORE UPDATE ON pricing_rules
    FOR EACH ROW
    EXECUTE FUNCTION set_updated_at();

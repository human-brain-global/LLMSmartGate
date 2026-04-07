-- Migration 5: Policies table and deferred FK from service_accounts
CREATE TABLE IF NOT EXISTS policies (
    id                  UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id           UUID        NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    name                TEXT        NOT NULL,
    description         TEXT,
    allowed_models_json JSONB       NOT NULL DEFAULT '[]'::jsonb,
    denied_models_json  JSONB       NOT NULL DEFAULT '[]'::jsonb,
    max_input_tokens    INTEGER,
    max_output_tokens   INTEGER,
    allow_streaming     BOOLEAN     NOT NULL DEFAULT TRUE,
    allow_tools         BOOLEAN     NOT NULL DEFAULT FALSE,
    allow_files         BOOLEAN     NOT NULL DEFAULT FALSE,
    rpm_limit           INTEGER,
    concurrency_limit   INTEGER,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_policies_tenant_id ON policies(tenant_id);

-- Add FK from service_accounts to policies (deferred from migration 003)
ALTER TABLE service_accounts
    ADD CONSTRAINT fk_service_accounts_default_policy
    FOREIGN KEY (default_policy_id) REFERENCES policies(id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS idx_service_accounts_default_policy_id ON service_accounts(default_policy_id);

-- Auto-update updated_at on row modification
CREATE TRIGGER trg_policies_updated_at
    BEFORE UPDATE ON policies
    FOR EACH ROW
    EXECUTE FUNCTION set_updated_at();

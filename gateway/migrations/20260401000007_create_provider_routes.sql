-- Migration 7: Provider routes for model alias routing
CREATE TABLE IF NOT EXISTS provider_routes (
    id                  UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id           UUID        REFERENCES tenants(id) ON DELETE CASCADE,  -- nullable for global routes
    model_alias         TEXT        NOT NULL,
    provider            TEXT        NOT NULL,
    provider_model_name TEXT        NOT NULL,
    priority            INTEGER     NOT NULL DEFAULT 100,
    enabled             BOOLEAN     NOT NULL DEFAULT TRUE,
    timeout_ms          INTEGER     NOT NULL DEFAULT 30000,
    max_retries         INTEGER     NOT NULL DEFAULT 1,
    retry_backoff_ms    INTEGER     NOT NULL DEFAULT 250,
    config              JSONB       NOT NULL DEFAULT '{}'::jsonb,  -- provider-specific config (base_url, api_version, etc.)
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_provider_routes_tenant_id ON provider_routes(tenant_id);
CREATE INDEX IF NOT EXISTS idx_provider_routes_model_alias ON provider_routes(model_alias);
CREATE INDEX IF NOT EXISTS idx_provider_routes_enabled ON provider_routes(enabled);

-- Auto-update updated_at on row modification
CREATE TRIGGER trg_provider_routes_updated_at
    BEFORE UPDATE ON provider_routes
    FOR EACH ROW
    EXECUTE FUNCTION set_updated_at();

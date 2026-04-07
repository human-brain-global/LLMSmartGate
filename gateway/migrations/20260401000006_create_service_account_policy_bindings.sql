-- Migration 6: Service account policy bindings (many-to-many)
CREATE TABLE IF NOT EXISTS service_account_policy_bindings (
    id                 UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    service_account_id UUID        NOT NULL REFERENCES service_accounts(id) ON DELETE CASCADE,
    policy_id          UUID        NOT NULL REFERENCES policies(id) ON DELETE CASCADE,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (service_account_id, policy_id)
);

CREATE INDEX IF NOT EXISTS idx_sapb_service_account_id ON service_account_policy_bindings(service_account_id);
CREATE INDEX IF NOT EXISTS idx_sapb_policy_id ON service_account_policy_bindings(policy_id);

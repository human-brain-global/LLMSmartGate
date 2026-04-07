-- Migration 4: Service account keys table
CREATE TABLE IF NOT EXISTS service_account_keys (
    id                 UUID       PRIMARY KEY DEFAULT gen_random_uuid(),
    service_account_id UUID       NOT NULL REFERENCES service_accounts(id) ON DELETE CASCADE,
    key_id             TEXT       NOT NULL UNIQUE,
    algorithm          TEXT       NOT NULL DEFAULT 'ed25519',
    public_key_pem     TEXT       NOT NULL,
    fingerprint        TEXT       NOT NULL,
    status             key_status NOT NULL DEFAULT 'active',
    expires_at         TIMESTAMPTZ,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    revoked_at         TIMESTAMPTZ,
    last_used_at       TIMESTAMPTZ
);

-- Index foreign key and frequently-queried columns
CREATE INDEX IF NOT EXISTS idx_service_account_keys_sa_id ON service_account_keys(service_account_id);
CREATE INDEX IF NOT EXISTS idx_service_account_keys_status ON service_account_keys(status);

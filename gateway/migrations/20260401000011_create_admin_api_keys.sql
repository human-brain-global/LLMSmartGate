CREATE TABLE IF NOT EXISTS admin_api_keys (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    key_hash TEXT NOT NULL,       -- bcrypt hash of the API key
    role TEXT NOT NULL DEFAULT 'viewer',  -- 'admin' or 'viewer'
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_used_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_admin_api_keys_active ON admin_api_keys(is_active);

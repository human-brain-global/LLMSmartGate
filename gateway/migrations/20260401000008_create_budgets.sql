-- Migration 8: Budgets
-- Supports tenant-level and service-account-level spend limits with configurable periods.

-- Enum: budget period type
DO $$ BEGIN
    CREATE TYPE budget_period_type AS ENUM ('daily', 'monthly', 'custom');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

-- Enum: budget status
DO $$ BEGIN
    CREATE TYPE budget_status AS ENUM ('active', 'exhausted', 'expired', 'disabled');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

CREATE TABLE IF NOT EXISTS budgets (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    service_account_id UUID REFERENCES service_accounts(id) ON DELETE CASCADE,  -- nullable = tenant-level
    period_type budget_period_type NOT NULL,
    amount_limit NUMERIC(18,6) NOT NULL,
    currency TEXT NOT NULL DEFAULT 'USD',
    start_at TIMESTAMPTZ NOT NULL,
    end_at TIMESTAMPTZ NOT NULL,
    status budget_status NOT NULL DEFAULT 'active',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_budgets_tenant_id ON budgets(tenant_id);
CREATE INDEX IF NOT EXISTS idx_budgets_service_account_id ON budgets(service_account_id);
CREATE INDEX IF NOT EXISTS idx_budgets_status ON budgets(status);

CREATE TRIGGER trg_budgets_updated_at
    BEFORE UPDATE ON budgets
    FOR EACH ROW
    EXECUTE FUNCTION set_updated_at();

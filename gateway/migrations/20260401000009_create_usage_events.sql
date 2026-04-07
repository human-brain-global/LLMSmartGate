-- Migration 9: Usage Events (partitioned by created_at, monthly ranges)
-- Append-only table for LLM request metering. No updated_at column.
-- Foreign keys omitted: PG < 17 cannot reference partitioned tables; enforced at application level.

CREATE TABLE IF NOT EXISTS usage_events (
    id UUID NOT NULL DEFAULT gen_random_uuid(),
    request_id TEXT NOT NULL,
    tenant_id UUID NOT NULL,
    service_account_id UUID NOT NULL,
    provider_route_id UUID,
    provider TEXT NOT NULL,
    model_alias TEXT NOT NULL,
    provider_model_name TEXT NOT NULL,
    prompt_tokens INTEGER NOT NULL DEFAULT 0,
    completion_tokens INTEGER NOT NULL DEFAULT 0,
    total_tokens INTEGER NOT NULL DEFAULT 0,
    estimated_cost NUMERIC(18,6) NOT NULL DEFAULT 0,
    currency TEXT NOT NULL DEFAULT 'USD',
    latency_ms INTEGER NOT NULL DEFAULT 0,
    retry_count INTEGER NOT NULL DEFAULT 0,
    final_status TEXT NOT NULL,
    is_streaming BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (id, created_at)  -- partition key must be in PK
) PARTITION BY RANGE (created_at);

-- Partitions: current month + next 3 months
CREATE TABLE IF NOT EXISTS usage_events_y2026m04 PARTITION OF usage_events
    FOR VALUES FROM ('2026-04-01') TO ('2026-05-01');
CREATE TABLE IF NOT EXISTS usage_events_y2026m05 PARTITION OF usage_events
    FOR VALUES FROM ('2026-05-01') TO ('2026-06-01');
CREATE TABLE IF NOT EXISTS usage_events_y2026m06 PARTITION OF usage_events
    FOR VALUES FROM ('2026-06-01') TO ('2026-07-01');
CREATE TABLE IF NOT EXISTS usage_events_y2026m07 PARTITION OF usage_events
    FOR VALUES FROM ('2026-07-01') TO ('2026-08-01');

-- Indexes (applied automatically to all partitions)
CREATE INDEX IF NOT EXISTS idx_usage_events_tenant_id ON usage_events(tenant_id);
CREATE INDEX IF NOT EXISTS idx_usage_events_sa_id ON usage_events(service_account_id);
CREATE INDEX IF NOT EXISTS idx_usage_events_created_at ON usage_events(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_usage_events_model_alias ON usage_events(model_alias);
CREATE INDEX IF NOT EXISTS idx_usage_events_provider ON usage_events(provider);
CREATE UNIQUE INDEX IF NOT EXISTS idx_usage_events_request_id ON usage_events(request_id, created_at);

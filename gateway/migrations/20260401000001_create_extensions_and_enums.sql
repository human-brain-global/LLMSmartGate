-- Migration 1: Extensions, enum types, and reusable trigger function
-- Enable pgcrypto for gen_random_uuid()
CREATE EXTENSION IF NOT EXISTS "pgcrypto";

-- Create enum types
CREATE TYPE tenant_status AS ENUM ('active', 'suspended', 'deleted');
CREATE TYPE service_account_status AS ENUM ('active', 'suspended', 'deleted');
CREATE TYPE key_status AS ENUM ('active', 'rotating', 'revoked', 'expired');

-- Reusable trigger function to auto-update updated_at on row modification
CREATE OR REPLACE FUNCTION set_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

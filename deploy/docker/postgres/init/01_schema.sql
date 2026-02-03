-- Fen PostgreSQL Schema
-- User/Tenant Management Database

-- Enable UUID extension
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- =============================================================================
-- Tenants
-- =============================================================================

CREATE TABLE tenants (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name VARCHAR(255) NOT NULL,
    status VARCHAR(50) NOT NULL DEFAULT 'trial',

    -- Feature flags (JSONB for flexibility)
    features JSONB NOT NULL DEFAULT '{
        "ml_anomaly_detection": true,
        "statistical_baselines": true,
        "real_time_notifications": true,
        "api_webhooks": false,
        "custom_rules": true,
        "export_enabled": true
    }'::jsonb,

    -- Rate limits
    rate_limits JSONB NOT NULL DEFAULT '{
        "requests_per_minute": 1000,
        "uploads_per_hour": 500,
        "max_concurrent_jobs": 10
    }'::jsonb,

    -- Quotas
    quotas JSONB NOT NULL DEFAULT '{
        "max_storage_bytes": 10737418240,
        "max_documents": 100000,
        "max_rules": 1000
    }'::jsonb,

    -- Retention policies per resource type
    retention_policies JSONB NOT NULL DEFAULT '{
        "invoice": {"hot_days": 60, "warm_days": 365, "archive_days": 2555},
        "contract": {"hot_days": 90, "warm_days": 730, "archive_days": 2555},
        "anomaly": {"hot_days": 30, "warm_days": 180, "archive_days": 365}
    }'::jsonb,

    -- Metadata
    metadata JSONB NOT NULL DEFAULT '{}',

    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Constraints
    CONSTRAINT valid_status CHECK (status IN ('trial', 'active', 'suspended', 'archived'))
);

CREATE INDEX idx_tenants_status ON tenants(status);
CREATE INDEX idx_tenants_created_at ON tenants(created_at);

-- =============================================================================
-- Users
-- =============================================================================

CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,

    -- Identity
    email VARCHAR(255) NOT NULL,
    display_name VARCHAR(255) NOT NULL,

    -- Status
    status VARCHAR(50) NOT NULL DEFAULT 'active',

    -- Authentication
    auth_provider VARCHAR(50) NOT NULL DEFAULT 'local',
    auth_provider_config JSONB DEFAULT NULL,
    password_hash VARCHAR(255) DEFAULT NULL,

    -- Roles (array of role names)
    roles TEXT[] NOT NULL DEFAULT '{}',

    -- Activity
    last_login_at TIMESTAMPTZ,
    failed_login_attempts INT NOT NULL DEFAULT 0,
    locked_until TIMESTAMPTZ,

    -- Metadata
    metadata JSONB NOT NULL DEFAULT '{}',

    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Constraints
    CONSTRAINT valid_user_status CHECK (status IN ('active', 'pending_verification', 'suspended', 'deactivated', 'locked')),
    CONSTRAINT valid_auth_provider CHECK (auth_provider IN ('local', 'oidc', 'saml', 'api_key')),
    UNIQUE (tenant_id, email)
);

CREATE INDEX idx_users_tenant_id ON users(tenant_id);
CREATE INDEX idx_users_email ON users(email);
CREATE INDEX idx_users_status ON users(status);
CREATE INDEX idx_users_auth_provider ON users(auth_provider);

-- =============================================================================
-- API Keys
-- =============================================================================

CREATE TABLE api_keys (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,

    -- Key info
    name VARCHAR(255) NOT NULL,
    key_hash VARCHAR(255) NOT NULL,
    key_prefix VARCHAR(12) NOT NULL,  -- First 8 chars for identification

    -- Permissions
    scopes TEXT[] NOT NULL DEFAULT '{}',

    -- Usage limits
    rate_limit_per_minute INT DEFAULT NULL,

    -- Validity
    expires_at TIMESTAMPTZ,
    last_used_at TIMESTAMPTZ,

    -- Status
    is_active BOOLEAN NOT NULL DEFAULT true,

    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE (key_prefix)
);

CREATE INDEX idx_api_keys_user_id ON api_keys(user_id);
CREATE INDEX idx_api_keys_tenant_id ON api_keys(tenant_id);
CREATE INDEX idx_api_keys_key_prefix ON api_keys(key_prefix);

-- =============================================================================
-- Audit Log
-- =============================================================================

CREATE TABLE audit_logs (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    tenant_id UUID REFERENCES tenants(id) ON DELETE SET NULL,
    user_id UUID REFERENCES users(id) ON DELETE SET NULL,

    -- Event info
    event_type VARCHAR(100) NOT NULL,
    resource_type VARCHAR(50) NOT NULL,
    resource_id VARCHAR(255),

    -- Details
    action VARCHAR(50) NOT NULL,
    details JSONB NOT NULL DEFAULT '{}',

    -- Request context
    ip_address INET,
    user_agent TEXT,

    -- Timestamp
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_audit_logs_tenant_id ON audit_logs(tenant_id);
CREATE INDEX idx_audit_logs_user_id ON audit_logs(user_id);
CREATE INDEX idx_audit_logs_event_type ON audit_logs(event_type);
CREATE INDEX idx_audit_logs_resource ON audit_logs(resource_type, resource_id);
CREATE INDEX idx_audit_logs_created_at ON audit_logs(created_at DESC);

-- =============================================================================
-- Tenant Usage Statistics (daily aggregates)
-- =============================================================================

CREATE TABLE tenant_usage_daily (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    date DATE NOT NULL,

    -- Document stats
    documents_uploaded INT NOT NULL DEFAULT 0,
    documents_processed INT NOT NULL DEFAULT 0,
    documents_total INT NOT NULL DEFAULT 0,

    -- Anomaly stats
    anomalies_detected INT NOT NULL DEFAULT 0,
    anomalies_resolved INT NOT NULL DEFAULT 0,

    -- API usage
    api_requests INT NOT NULL DEFAULT 0,
    api_errors INT NOT NULL DEFAULT 0,

    -- Storage
    storage_bytes_used BIGINT NOT NULL DEFAULT 0,

    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE (tenant_id, date)
);

CREATE INDEX idx_tenant_usage_daily_tenant_date ON tenant_usage_daily(tenant_id, date DESC);

-- =============================================================================
-- Webhooks
-- =============================================================================

CREATE TABLE webhooks (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,

    -- Endpoint
    url VARCHAR(2048) NOT NULL,
    secret VARCHAR(255),

    -- Event filters
    event_types TEXT[] NOT NULL DEFAULT '{}',

    -- Status
    is_active BOOLEAN NOT NULL DEFAULT true,

    -- Reliability
    failure_count INT NOT NULL DEFAULT 0,
    last_success_at TIMESTAMPTZ,
    last_failure_at TIMESTAMPTZ,
    last_failure_reason TEXT,

    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_webhooks_tenant_id ON webhooks(tenant_id);
CREATE INDEX idx_webhooks_is_active ON webhooks(is_active);

-- =============================================================================
-- Notification Preferences
-- =============================================================================

CREATE TABLE notification_preferences (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    user_id UUID REFERENCES users(id) ON DELETE CASCADE,

    -- Delivery channels
    channels TEXT[] NOT NULL DEFAULT ARRAY['websocket'],

    -- Event filters
    event_types TEXT[] NOT NULL DEFAULT '{}',
    severity_threshold VARCHAR(20) DEFAULT 'medium',

    -- Email settings
    email_enabled BOOLEAN NOT NULL DEFAULT false,
    email_digest_frequency VARCHAR(20) DEFAULT 'daily',

    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Either tenant-wide or user-specific
    UNIQUE (tenant_id, user_id)
);

CREATE INDEX idx_notification_prefs_tenant ON notification_preferences(tenant_id);
CREATE INDEX idx_notification_prefs_user ON notification_preferences(user_id);

-- =============================================================================
-- Functions
-- =============================================================================

-- Update timestamp trigger
CREATE OR REPLACE FUNCTION update_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Apply to tables
CREATE TRIGGER tenants_updated_at BEFORE UPDATE ON tenants
    FOR EACH ROW EXECUTE FUNCTION update_updated_at();

CREATE TRIGGER users_updated_at BEFORE UPDATE ON users
    FOR EACH ROW EXECUTE FUNCTION update_updated_at();

CREATE TRIGGER tenant_usage_daily_updated_at BEFORE UPDATE ON tenant_usage_daily
    FOR EACH ROW EXECUTE FUNCTION update_updated_at();

CREATE TRIGGER webhooks_updated_at BEFORE UPDATE ON webhooks
    FOR EACH ROW EXECUTE FUNCTION update_updated_at();

CREATE TRIGGER notification_preferences_updated_at BEFORE UPDATE ON notification_preferences
    FOR EACH ROW EXECUTE FUNCTION update_updated_at();

-- =============================================================================
-- Default Data
-- =============================================================================

-- System tenant (for system admin users)
INSERT INTO tenants (id, name, status, features, quotas)
VALUES (
    '00000000-0000-0000-0000-000000000000',
    'System',
    'active',
    '{"ml_anomaly_detection": true, "statistical_baselines": true, "real_time_notifications": true, "api_webhooks": true, "custom_rules": true, "export_enabled": true}',
    '{"max_storage_bytes": -1, "max_documents": -1, "max_rules": -1}'
);

-- Demo tenant
INSERT INTO tenants (id, name, status)
VALUES (
    '11111111-1111-1111-1111-111111111111',
    'ACME Corporation',
    'active'
);

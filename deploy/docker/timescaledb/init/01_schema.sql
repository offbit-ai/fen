-- Fen TimescaleDB Schema
-- Observability & Metrics Database

-- Enable TimescaleDB extension
CREATE EXTENSION IF NOT EXISTS timescaledb;
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- =============================================================================
-- API Metrics (request-level telemetry)
-- =============================================================================

CREATE TABLE api_metrics (
    time TIMESTAMPTZ NOT NULL,
    tenant_id UUID NOT NULL,

    -- Request info
    method VARCHAR(10) NOT NULL,
    path VARCHAR(255) NOT NULL,
    status_code SMALLINT NOT NULL,

    -- Performance
    duration_ms DOUBLE PRECISION NOT NULL,

    -- User context
    user_id UUID,

    -- Request metadata
    request_size_bytes INT,
    response_size_bytes INT,

    -- Error info
    error_type VARCHAR(100),

    -- Tags for filtering
    tags JSONB DEFAULT '{}'
);

SELECT create_hypertable('api_metrics', 'time');

CREATE INDEX idx_api_metrics_tenant ON api_metrics(tenant_id, time DESC);
CREATE INDEX idx_api_metrics_path ON api_metrics(path, time DESC);
CREATE INDEX idx_api_metrics_status ON api_metrics(status_code, time DESC);

-- =============================================================================
-- Document Processing Events
-- =============================================================================

CREATE TABLE document_events (
    time TIMESTAMPTZ NOT NULL,
    tenant_id UUID NOT NULL,
    document_id UUID NOT NULL,

    -- Event type
    event_type VARCHAR(50) NOT NULL,

    -- Processing info
    stage VARCHAR(50),
    duration_ms DOUBLE PRECISION,
    success BOOLEAN NOT NULL DEFAULT true,

    -- Document info
    document_type VARCHAR(50),
    file_size_bytes BIGINT,

    -- Error info
    error_message TEXT,

    -- Metadata
    metadata JSONB DEFAULT '{}'
);

SELECT create_hypertable('document_events', 'time');

CREATE INDEX idx_document_events_tenant ON document_events(tenant_id, time DESC);
CREATE INDEX idx_document_events_document ON document_events(document_id, time DESC);
CREATE INDEX idx_document_events_type ON document_events(event_type, time DESC);

-- =============================================================================
-- Anomaly Events
-- =============================================================================

CREATE TABLE anomaly_events (
    time TIMESTAMPTZ NOT NULL,
    tenant_id UUID NOT NULL,
    anomaly_id UUID NOT NULL,
    document_id UUID NOT NULL,

    -- Anomaly info
    anomaly_type VARCHAR(50) NOT NULL,
    severity VARCHAR(20) NOT NULL,
    confidence DOUBLE PRECISION,

    -- Detection info
    detection_method VARCHAR(50),
    z_score DOUBLE PRECISION,
    percentile DOUBLE PRECISION,

    -- Resolution
    status VARCHAR(20) NOT NULL DEFAULT 'open',
    resolved_by UUID,
    resolved_at TIMESTAMPTZ,

    -- Description
    description TEXT,

    -- Metadata
    metadata JSONB DEFAULT '{}'
);

SELECT create_hypertable('anomaly_events', 'time');

CREATE INDEX idx_anomaly_events_tenant ON anomaly_events(tenant_id, time DESC);
CREATE INDEX idx_anomaly_events_document ON anomaly_events(document_id, time DESC);
CREATE INDEX idx_anomaly_events_severity ON anomaly_events(severity, time DESC);
CREATE INDEX idx_anomaly_events_status ON anomaly_events(status, time DESC);

-- =============================================================================
-- System Metrics (infrastructure observability)
-- =============================================================================

CREATE TABLE system_metrics (
    time TIMESTAMPTZ NOT NULL,
    node_id VARCHAR(100) NOT NULL,

    -- Metric identification
    metric_name VARCHAR(100) NOT NULL,
    metric_value DOUBLE PRECISION NOT NULL,

    -- Dimensions
    labels JSONB DEFAULT '{}'
);

SELECT create_hypertable('system_metrics', 'time');

CREATE INDEX idx_system_metrics_node ON system_metrics(node_id, time DESC);
CREATE INDEX idx_system_metrics_name ON system_metrics(metric_name, time DESC);

-- =============================================================================
-- Ingestion Pipeline Metrics
-- =============================================================================

CREATE TABLE ingestion_metrics (
    time TIMESTAMPTZ NOT NULL,
    tenant_id UUID NOT NULL,

    -- Pipeline stage
    stage VARCHAR(50) NOT NULL,

    -- Throughput
    documents_processed INT NOT NULL DEFAULT 0,
    bytes_processed BIGINT NOT NULL DEFAULT 0,

    -- Performance
    avg_duration_ms DOUBLE PRECISION,
    p50_duration_ms DOUBLE PRECISION,
    p95_duration_ms DOUBLE PRECISION,
    p99_duration_ms DOUBLE PRECISION,

    -- Errors
    error_count INT NOT NULL DEFAULT 0,

    -- Queue depth
    queue_depth INT
);

SELECT create_hypertable('ingestion_metrics', 'time');

CREATE INDEX idx_ingestion_metrics_tenant ON ingestion_metrics(tenant_id, time DESC);
CREATE INDEX idx_ingestion_metrics_stage ON ingestion_metrics(stage, time DESC);

-- =============================================================================
-- Rule Engine Metrics
-- =============================================================================

CREATE TABLE rule_metrics (
    time TIMESTAMPTZ NOT NULL,
    tenant_id UUID NOT NULL,

    -- Rule identification
    rule_id VARCHAR(100) NOT NULL,
    rule_name VARCHAR(255),

    -- Execution stats
    executions INT NOT NULL DEFAULT 0,
    matches INT NOT NULL DEFAULT 0,
    errors INT NOT NULL DEFAULT 0,

    -- Performance
    avg_duration_ms DOUBLE PRECISION,
    max_duration_ms DOUBLE PRECISION
);

SELECT create_hypertable('rule_metrics', 'time');

CREATE INDEX idx_rule_metrics_tenant ON rule_metrics(tenant_id, time DESC);
CREATE INDEX idx_rule_metrics_rule ON rule_metrics(rule_id, time DESC);

-- =============================================================================
-- Storage Tier Metrics
-- =============================================================================

CREATE TABLE storage_metrics (
    time TIMESTAMPTZ NOT NULL,
    tenant_id UUID NOT NULL,

    -- Tier info
    tier VARCHAR(20) NOT NULL,  -- hot, warm, cold

    -- Capacity
    documents_count BIGINT NOT NULL DEFAULT 0,
    bytes_used BIGINT NOT NULL DEFAULT 0,

    -- Operations
    reads INT NOT NULL DEFAULT 0,
    writes INT NOT NULL DEFAULT 0,
    migrations INT NOT NULL DEFAULT 0,

    -- Cache stats (for hot tier)
    cache_hits INT,
    cache_misses INT
);

SELECT create_hypertable('storage_metrics', 'time');

CREATE INDEX idx_storage_metrics_tenant ON storage_metrics(tenant_id, time DESC);
CREATE INDEX idx_storage_metrics_tier ON storage_metrics(tier, time DESC);

-- =============================================================================
-- Notification Events
-- =============================================================================

CREATE TABLE notification_events (
    time TIMESTAMPTZ NOT NULL,
    tenant_id UUID NOT NULL,
    notification_id UUID NOT NULL,

    -- Delivery info
    provider VARCHAR(50) NOT NULL,
    channel VARCHAR(50) NOT NULL,
    recipient VARCHAR(255),

    -- Status
    status VARCHAR(20) NOT NULL,  -- sent, delivered, failed, bounced

    -- Performance
    latency_ms DOUBLE PRECISION,

    -- Error info
    error_message TEXT,

    -- Metadata
    metadata JSONB DEFAULT '{}'
);

SELECT create_hypertable('notification_events', 'time');

CREATE INDEX idx_notification_events_tenant ON notification_events(tenant_id, time DESC);
CREATE INDEX idx_notification_events_provider ON notification_events(provider, time DESC);
CREATE INDEX idx_notification_events_status ON notification_events(status, time DESC);

-- =============================================================================
-- Continuous Aggregates (materialized views with automatic refresh)
-- =============================================================================

-- Hourly API metrics rollup
CREATE MATERIALIZED VIEW api_metrics_hourly
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 hour', time) AS bucket,
    tenant_id,
    path,
    COUNT(*) AS request_count,
    AVG(duration_ms) AS avg_duration_ms,
    percentile_cont(0.5) WITHIN GROUP (ORDER BY duration_ms) AS p50_duration_ms,
    percentile_cont(0.95) WITHIN GROUP (ORDER BY duration_ms) AS p95_duration_ms,
    percentile_cont(0.99) WITHIN GROUP (ORDER BY duration_ms) AS p99_duration_ms,
    SUM(CASE WHEN status_code >= 400 THEN 1 ELSE 0 END) AS error_count,
    SUM(request_size_bytes) AS total_request_bytes,
    SUM(response_size_bytes) AS total_response_bytes
FROM api_metrics
GROUP BY bucket, tenant_id, path
WITH NO DATA;

SELECT add_continuous_aggregate_policy('api_metrics_hourly',
    start_offset => INTERVAL '3 hours',
    end_offset => INTERVAL '1 hour',
    schedule_interval => INTERVAL '1 hour');

-- Daily document processing rollup
CREATE MATERIALIZED VIEW document_events_daily
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 day', time) AS bucket,
    tenant_id,
    event_type,
    document_type,
    COUNT(*) AS event_count,
    SUM(CASE WHEN success THEN 1 ELSE 0 END) AS success_count,
    AVG(duration_ms) AS avg_duration_ms,
    SUM(file_size_bytes) AS total_bytes_processed
FROM document_events
GROUP BY bucket, tenant_id, event_type, document_type
WITH NO DATA;

SELECT add_continuous_aggregate_policy('document_events_daily',
    start_offset => INTERVAL '3 days',
    end_offset => INTERVAL '1 day',
    schedule_interval => INTERVAL '1 day');

-- Hourly anomaly summary
CREATE MATERIALIZED VIEW anomaly_events_hourly
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 hour', time) AS bucket,
    tenant_id,
    anomaly_type,
    severity,
    COUNT(*) AS anomaly_count,
    AVG(confidence) AS avg_confidence,
    SUM(CASE WHEN status = 'resolved' THEN 1 ELSE 0 END) AS resolved_count
FROM anomaly_events
GROUP BY bucket, tenant_id, anomaly_type, severity
WITH NO DATA;

SELECT add_continuous_aggregate_policy('anomaly_events_hourly',
    start_offset => INTERVAL '3 hours',
    end_offset => INTERVAL '1 hour',
    schedule_interval => INTERVAL '1 hour');

-- =============================================================================
-- Retention Policies
-- =============================================================================

-- Keep raw data for 30 days, aggregates for longer
SELECT add_retention_policy('api_metrics', INTERVAL '30 days');
SELECT add_retention_policy('document_events', INTERVAL '30 days');
SELECT add_retention_policy('anomaly_events', INTERVAL '90 days');
SELECT add_retention_policy('system_metrics', INTERVAL '7 days');
SELECT add_retention_policy('ingestion_metrics', INTERVAL '30 days');
SELECT add_retention_policy('rule_metrics', INTERVAL '30 days');
SELECT add_retention_policy('storage_metrics', INTERVAL '30 days');
SELECT add_retention_policy('notification_events', INTERVAL '30 days');

-- =============================================================================
-- Compression Policies
-- =============================================================================

-- Compress data older than 7 days
ALTER TABLE api_metrics SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'tenant_id'
);
SELECT add_compression_policy('api_metrics', INTERVAL '7 days');

ALTER TABLE document_events SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'tenant_id'
);
SELECT add_compression_policy('document_events', INTERVAL '7 days');

ALTER TABLE anomaly_events SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'tenant_id'
);
SELECT add_compression_policy('anomaly_events', INTERVAL '7 days');

ALTER TABLE system_metrics SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'node_id'
);
SELECT add_compression_policy('system_metrics', INTERVAL '3 days');

-- =============================================================================
-- Helper Functions
-- =============================================================================

-- Get tenant metrics summary for dashboard
CREATE OR REPLACE FUNCTION get_tenant_metrics_summary(
    p_tenant_id UUID,
    p_start_time TIMESTAMPTZ DEFAULT NOW() - INTERVAL '24 hours',
    p_end_time TIMESTAMPTZ DEFAULT NOW()
)
RETURNS TABLE (
    total_requests BIGINT,
    avg_latency_ms DOUBLE PRECISION,
    error_rate DOUBLE PRECISION,
    documents_processed BIGINT,
    anomalies_detected BIGINT,
    anomalies_open BIGINT
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        (SELECT COUNT(*) FROM api_metrics WHERE tenant_id = p_tenant_id AND time BETWEEN p_start_time AND p_end_time),
        (SELECT AVG(duration_ms) FROM api_metrics WHERE tenant_id = p_tenant_id AND time BETWEEN p_start_time AND p_end_time),
        (SELECT COALESCE(SUM(CASE WHEN status_code >= 400 THEN 1 ELSE 0 END)::DOUBLE PRECISION / NULLIF(COUNT(*), 0), 0) FROM api_metrics WHERE tenant_id = p_tenant_id AND time BETWEEN p_start_time AND p_end_time),
        (SELECT COUNT(*) FROM document_events WHERE tenant_id = p_tenant_id AND event_type = 'processed' AND time BETWEEN p_start_time AND p_end_time),
        (SELECT COUNT(*) FROM anomaly_events WHERE tenant_id = p_tenant_id AND time BETWEEN p_start_time AND p_end_time),
        (SELECT COUNT(*) FROM anomaly_events WHERE tenant_id = p_tenant_id AND status = 'open' AND time BETWEEN p_start_time AND p_end_time);
END;
$$ LANGUAGE plpgsql;

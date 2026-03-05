//! Tenant repository for PostgreSQL.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use fen_core::domain::{FeatureFlags, QuotaConfig, RateLimitConfig, TenantId, TenantStatus};

/// Database representation of a tenant
#[derive(Debug, Clone, FromRow)]
pub struct TenantRow {
    pub id: Uuid,
    pub name: String,
    pub status: String,
    pub features: sqlx::types::Json<FeatureFlags>,
    pub rate_limits: sqlx::types::Json<RateLimitConfig>,
    pub quotas: sqlx::types::Json<QuotaConfig>,
    pub retention_policies: sqlx::types::Json<serde_json::Value>,
    pub metadata: sqlx::types::Json<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Tenant creation parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTenant {
    pub name: String,
    pub status: Option<TenantStatus>,
    pub features: Option<FeatureFlags>,
    pub rate_limits: Option<RateLimitConfig>,
    pub quotas: Option<QuotaConfig>,
}

/// Tenant update parameters
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateTenant {
    pub name: Option<String>,
    pub status: Option<TenantStatus>,
    pub features: Option<FeatureFlags>,
    pub rate_limits: Option<RateLimitConfig>,
    pub quotas: Option<QuotaConfig>,
}

/// Repository for tenant operations
#[derive(Clone)]
pub struct TenantRepository {
    pool: PgPool,
}

impl TenantRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create a new tenant
    pub async fn create(&self, params: CreateTenant) -> Result<TenantRow, sqlx::Error> {
        let id = Uuid::new_v4();
        let status = params
            .status
            .map(|s| format!("{:?}", s).to_lowercase())
            .unwrap_or_else(|| "trial".to_string());

        let row = sqlx::query_as::<_, TenantRow>(
            r#"
            INSERT INTO tenants (id, name, status, features, rate_limits, quotas)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(&params.name)
        .bind(&status)
        .bind(sqlx::types::Json(params.features.unwrap_or_default()))
        .bind(sqlx::types::Json(params.rate_limits.unwrap_or_default()))
        .bind(sqlx::types::Json(params.quotas.unwrap_or_default()))
        .fetch_one(&self.pool)
        .await?;

        Ok(row)
    }

    /// Get a tenant by ID
    pub async fn get(&self, id: &TenantId) -> Result<Option<TenantRow>, sqlx::Error> {
        let row = sqlx::query_as::<_, TenantRow>("SELECT * FROM tenants WHERE id = $1")
            .bind(id.0)
            .fetch_optional(&self.pool)
            .await?;

        Ok(row)
    }

    /// List all tenants with pagination
    pub async fn list(
        &self,
        limit: i64,
        offset: i64,
        status_filter: Option<&str>,
    ) -> Result<Vec<TenantRow>, sqlx::Error> {
        let rows = if let Some(status) = status_filter {
            sqlx::query_as::<_, TenantRow>(
                "SELECT * FROM tenants WHERE status = $1 ORDER BY created_at DESC LIMIT $2 OFFSET $3",
            )
            .bind(status)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, TenantRow>(
                "SELECT * FROM tenants ORDER BY created_at DESC LIMIT $1 OFFSET $2",
            )
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?
        };

        Ok(rows)
    }

    /// Update a tenant
    pub async fn update(
        &self,
        id: &TenantId,
        params: UpdateTenant,
    ) -> Result<Option<TenantRow>, sqlx::Error> {
        // Build dynamic update query
        let mut query = String::from("UPDATE tenants SET updated_at = NOW()");
        let mut param_count = 1;

        if params.name.is_some() {
            param_count += 1;
            query.push_str(&format!(", name = ${}", param_count));
        }
        if params.status.is_some() {
            param_count += 1;
            query.push_str(&format!(", status = ${}", param_count));
        }
        if params.features.is_some() {
            param_count += 1;
            query.push_str(&format!(", features = ${}", param_count));
        }
        if params.rate_limits.is_some() {
            param_count += 1;
            query.push_str(&format!(", rate_limits = ${}", param_count));
        }
        if params.quotas.is_some() {
            param_count += 1;
            query.push_str(&format!(", quotas = ${}", param_count));
        }

        query.push_str(" WHERE id = $1 RETURNING *");

        // Use a simpler approach - separate queries for each field
        // This is more maintainable than dynamic query building
        let row = sqlx::query_as::<_, TenantRow>(
            r#"
            UPDATE tenants SET
                name = COALESCE($2, name),
                status = COALESCE($3, status),
                features = COALESCE($4, features),
                rate_limits = COALESCE($5, rate_limits),
                quotas = COALESCE($6, quotas),
                updated_at = NOW()
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(id.0)
        .bind(params.name)
        .bind(params.status.map(|s| format!("{:?}", s).to_lowercase()))
        .bind(params.features.map(sqlx::types::Json))
        .bind(params.rate_limits.map(sqlx::types::Json))
        .bind(params.quotas.map(sqlx::types::Json))
        .fetch_optional(&self.pool)
        .await?;

        Ok(row)
    }

    /// Suspend a tenant
    pub async fn suspend(&self, id: &TenantId) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            "UPDATE tenants SET status = 'suspended', updated_at = NOW() WHERE id = $1",
        )
        .bind(id.0)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Activate a tenant
    pub async fn activate(&self, id: &TenantId) -> Result<bool, sqlx::Error> {
        let result =
            sqlx::query("UPDATE tenants SET status = 'active', updated_at = NOW() WHERE id = $1")
                .bind(id.0)
                .execute(&self.pool)
                .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Count total tenants
    pub async fn count(&self, status_filter: Option<&str>) -> Result<i64, sqlx::Error> {
        let count: (i64,) = if let Some(status) = status_filter {
            sqlx::query_as("SELECT COUNT(*) FROM tenants WHERE status = $1")
                .bind(status)
                .fetch_one(&self.pool)
                .await?
        } else {
            sqlx::query_as("SELECT COUNT(*) FROM tenants")
                .fetch_one(&self.pool)
                .await?
        };

        Ok(count.0)
    }
}

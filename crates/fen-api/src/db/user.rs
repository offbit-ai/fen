//! User repository for PostgreSQL.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use fen_core::domain::{TenantId, UserId, UserStatus};

/// Database representation of a user
#[derive(Debug, Clone, FromRow)]
pub struct UserRow {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub email: String,
    pub display_name: String,
    pub status: String,
    pub auth_provider: String,
    pub auth_provider_config: Option<sqlx::types::Json<serde_json::Value>>,
    pub password_hash: Option<String>,
    pub roles: Vec<String>,
    pub last_login_at: Option<DateTime<Utc>>,
    pub failed_login_attempts: i32,
    pub locked_until: Option<DateTime<Utc>>,
    pub metadata: sqlx::types::Json<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// User creation parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateUser {
    pub tenant_id: TenantId,
    pub email: String,
    pub display_name: String,
    pub auth_provider: String,
    pub auth_provider_config: Option<serde_json::Value>,
    pub password_hash: Option<String>,
    pub roles: Vec<String>,
}

/// User update parameters
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateUser {
    pub display_name: Option<String>,
    pub status: Option<UserStatus>,
    pub roles: Option<Vec<String>>,
    pub metadata: Option<serde_json::Value>,
}

/// User list filter
#[derive(Debug, Clone, Default)]
pub struct UserFilter {
    pub status: Option<String>,
    pub role: Option<String>,
    pub search: Option<String>,
}

/// Repository for user operations
#[derive(Clone)]
pub struct UserRepository {
    pool: PgPool,
}

impl UserRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create a new user
    pub async fn create(&self, params: CreateUser) -> Result<UserRow, sqlx::Error> {
        let id = Uuid::new_v4();

        let row = sqlx::query_as::<_, UserRow>(
            r#"
            INSERT INTO users (
                id, tenant_id, email, display_name,
                auth_provider, auth_provider_config, password_hash, roles
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(params.tenant_id.0)
        .bind(&params.email)
        .bind(&params.display_name)
        .bind(&params.auth_provider)
        .bind(params.auth_provider_config.map(sqlx::types::Json))
        .bind(params.password_hash)
        .bind(&params.roles)
        .fetch_one(&self.pool)
        .await?;

        Ok(row)
    }

    /// Get a user by ID
    pub async fn get(&self, id: &UserId) -> Result<Option<UserRow>, sqlx::Error> {
        let row = sqlx::query_as::<_, UserRow>("SELECT * FROM users WHERE id = $1")
            .bind(id.0)
            .fetch_optional(&self.pool)
            .await?;

        Ok(row)
    }

    /// Get a user by email within a tenant
    pub async fn get_by_email(
        &self,
        tenant_id: &TenantId,
        email: &str,
    ) -> Result<Option<UserRow>, sqlx::Error> {
        let row = sqlx::query_as::<_, UserRow>(
            "SELECT * FROM users WHERE tenant_id = $1 AND email = $2",
        )
        .bind(tenant_id.0)
        .bind(email)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row)
    }

    /// Get a user by external provider ID
    pub async fn get_by_external_id(
        &self,
        provider: &str,
        external_id: &str,
    ) -> Result<Option<UserRow>, sqlx::Error> {
        let row = sqlx::query_as::<_, UserRow>(
            r#"
            SELECT * FROM users
            WHERE auth_provider = $1
            AND auth_provider_config->>'external_id' = $2
            "#,
        )
        .bind(provider)
        .bind(external_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row)
    }

    /// List users for a tenant with pagination
    pub async fn list_for_tenant(
        &self,
        tenant_id: &TenantId,
        limit: i64,
        offset: i64,
        filter: UserFilter,
    ) -> Result<Vec<UserRow>, sqlx::Error> {
        // Use a simpler approach with optional filters in the query itself
        let rows = sqlx::query_as::<_, UserRow>(
            r#"
            SELECT * FROM users
            WHERE tenant_id = $1
              AND ($2::text IS NULL OR status = $2)
              AND ($3::text IS NULL OR $3 = ANY(roles))
              AND ($4::text IS NULL OR email ILIKE $4 OR display_name ILIKE $4)
            ORDER BY created_at DESC
            LIMIT $5 OFFSET $6
            "#,
        )
        .bind(tenant_id.0)
        .bind(filter.status)
        .bind(filter.role)
        .bind(filter.search.map(|s| format!("%{}%", s)))
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    /// Update a user
    pub async fn update(
        &self,
        id: &UserId,
        params: UpdateUser,
    ) -> Result<Option<UserRow>, sqlx::Error> {
        let row = sqlx::query_as::<_, UserRow>(
            r#"
            UPDATE users SET
                display_name = COALESCE($2, display_name),
                status = COALESCE($3, status),
                roles = COALESCE($4, roles),
                metadata = COALESCE($5, metadata),
                updated_at = NOW()
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(id.0)
        .bind(params.display_name)
        .bind(params.status.map(|s| format!("{:?}", s).to_lowercase()))
        .bind(params.roles)
        .bind(params.metadata.map(sqlx::types::Json))
        .fetch_optional(&self.pool)
        .await?;

        Ok(row)
    }

    /// Update user roles
    pub async fn update_roles(
        &self,
        id: &UserId,
        roles: Vec<String>,
    ) -> Result<Option<UserRow>, sqlx::Error> {
        let row = sqlx::query_as::<_, UserRow>(
            "UPDATE users SET roles = $2, updated_at = NOW() WHERE id = $1 RETURNING *",
        )
        .bind(id.0)
        .bind(&roles)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row)
    }

    /// Suspend a user
    pub async fn suspend(&self, id: &UserId) -> Result<bool, sqlx::Error> {
        let result =
            sqlx::query("UPDATE users SET status = 'suspended', updated_at = NOW() WHERE id = $1")
                .bind(id.0)
                .execute(&self.pool)
                .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Activate a user
    pub async fn activate(&self, id: &UserId) -> Result<bool, sqlx::Error> {
        let result =
            sqlx::query("UPDATE users SET status = 'active', updated_at = NOW() WHERE id = $1")
                .bind(id.0)
                .execute(&self.pool)
                .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Delete a user
    pub async fn delete(&self, id: &UserId) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(id.0)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Record a login attempt
    pub async fn record_login(&self, id: &UserId) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE users SET last_login_at = NOW(), failed_login_attempts = 0 WHERE id = $1",
        )
        .bind(id.0)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Record a failed login attempt
    pub async fn record_failed_login(&self, id: &UserId) -> Result<i32, sqlx::Error> {
        let row: (i32,) = sqlx::query_as(
            r#"
            UPDATE users SET
                failed_login_attempts = failed_login_attempts + 1,
                locked_until = CASE
                    WHEN failed_login_attempts >= 4 THEN NOW() + INTERVAL '15 minutes'
                    ELSE locked_until
                END
            WHERE id = $1
            RETURNING failed_login_attempts
            "#,
        )
        .bind(id.0)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.0)
    }

    /// Count users for a tenant
    pub async fn count_for_tenant(&self, tenant_id: &TenantId) -> Result<i64, sqlx::Error> {
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users WHERE tenant_id = $1")
            .bind(tenant_id.0)
            .fetch_one(&self.pool)
            .await?;

        Ok(count.0)
    }
}

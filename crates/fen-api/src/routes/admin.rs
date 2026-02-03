//! Admin routes for tenant and user management.
//!
//! These routes are protected and require TenantAdmin or SystemAdmin role.
//!
//! ## Tenant Management (SystemAdmin only)
//! - Create, list, update, suspend tenants
//!
//! ## User Management (TenantAdmin or SystemAdmin)
//! - Create, list, update, delete users within a tenant
//! - Assign/revoke roles

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Extension, Json,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use fen_core::domain::{
    acl::Role, CreateUserRequest, FeatureFlags, QuotaConfig, RateLimitConfig, TenantConfig,
    TenantId, TenantStatus, UpdateUserRequest, User, UserId, UserStatus,
};

use crate::error::ApiError;
use crate::middleware::AuthContext;
use crate::state::AppState;

// ============================================================================
// Tenant Management (SystemAdmin only)
// ============================================================================

/// Request to create a new tenant.
#[derive(Debug, Deserialize)]
pub struct CreateTenantRequest {
    /// Tenant name
    pub name: String,
    /// Initial status (default: Trial)
    pub status: Option<TenantStatus>,
    /// Feature flags
    pub features: Option<FeatureFlags>,
    /// Rate limits
    pub rate_limits: Option<RateLimitConfig>,
    /// Quotas
    pub quotas: Option<QuotaConfig>,
}

/// Response for tenant operations.
#[derive(Debug, Serialize)]
pub struct TenantResponse {
    pub id: String,
    pub name: String,
    pub status: TenantStatus,
    pub features: FeatureFlags,
    pub rate_limits: RateLimitConfig,
    pub quotas: QuotaConfig,
    pub created_at: chrono::DateTime<Utc>,
    pub updated_at: chrono::DateTime<Utc>,
}

impl From<TenantConfig> for TenantResponse {
    fn from(config: TenantConfig) -> Self {
        Self {
            id: config.tenant_id.to_string(),
            name: config.name,
            status: config.status,
            features: config.features,
            rate_limits: config.rate_limits,
            quotas: config.quotas,
            created_at: config.created_at,
            updated_at: config.updated_at,
        }
    }
}

/// POST /admin/tenants - Create a new tenant (SystemAdmin only).
pub async fn create_tenant(
    State(_state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Json(request): Json<CreateTenantRequest>,
) -> Result<impl IntoResponse, ApiError> {
    // Verify SystemAdmin role
    if !auth.permissions.has_role(&Role::SystemAdmin) {
        return Err(ApiError::Forbidden("SystemAdmin role required".to_string()));
    }

    let tenant_id = TenantId::new();
    let config = TenantConfig::new(tenant_id.clone(), request.name)
        .with_status(request.status.unwrap_or(TenantStatus::Trial))
        .with_features(request.features.unwrap_or_default())
        .with_rate_limits(request.rate_limits.unwrap_or_default())
        .with_quotas(request.quotas.unwrap_or_default());

    // TODO: Persist tenant config to storage

    Ok((StatusCode::CREATED, Json(TenantResponse::from(config))))
}

/// GET /admin/tenants - List all tenants (SystemAdmin only).
pub async fn list_tenants(
    State(_state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
) -> Result<Json<Vec<TenantResponse>>, ApiError> {
    if !auth.permissions.has_role(&Role::SystemAdmin) {
        return Err(ApiError::Forbidden("SystemAdmin role required".to_string()));
    }

    // TODO: Load from storage
    // For now, return empty list
    Ok(Json(vec![]))
}

/// GET /admin/tenants/:tenant_id - Get tenant details (SystemAdmin only).
pub async fn get_tenant(
    State(_state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(tenant_id): Path<String>,
) -> Result<Json<TenantResponse>, ApiError> {
    if !auth.permissions.has_role(&Role::SystemAdmin) {
        return Err(ApiError::Forbidden("SystemAdmin role required".to_string()));
    }

    let _tenant_id = TenantId::from_string(&tenant_id)
        .map_err(|_| ApiError::BadRequest("Invalid tenant ID".to_string()))?;

    // TODO: Load from storage
    Err(ApiError::NotFound("Tenant not found".to_string()))
}

/// Request to update a tenant.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct UpdateTenantRequest {
    pub name: Option<String>,
    pub status: Option<TenantStatus>,
    pub features: Option<FeatureFlags>,
    pub rate_limits: Option<RateLimitConfig>,
    pub quotas: Option<QuotaConfig>,
}

/// PUT /admin/tenants/:tenant_id - Update tenant (SystemAdmin only).
pub async fn update_tenant(
    State(_state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(tenant_id): Path<String>,
    Json(_request): Json<UpdateTenantRequest>,
) -> Result<Json<TenantResponse>, ApiError> {
    if !auth.permissions.has_role(&Role::SystemAdmin) {
        return Err(ApiError::Forbidden("SystemAdmin role required".to_string()));
    }

    let _tenant_id = TenantId::from_string(&tenant_id)
        .map_err(|_| ApiError::BadRequest("Invalid tenant ID".to_string()))?;

    // TODO: Load, update, and persist
    Err(ApiError::NotFound("Tenant not found".to_string()))
}

/// POST /admin/tenants/:tenant_id/suspend - Suspend a tenant (SystemAdmin only).
pub async fn suspend_tenant(
    State(_state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(tenant_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    if !auth.permissions.has_role(&Role::SystemAdmin) {
        return Err(ApiError::Forbidden("SystemAdmin role required".to_string()));
    }

    let _tenant_id = TenantId::from_string(&tenant_id)
        .map_err(|_| ApiError::BadRequest("Invalid tenant ID".to_string()))?;

    // TODO: Update tenant status
    Ok((StatusCode::OK, Json(serde_json::json!({"message": "Tenant suspended"}))))
}

/// POST /admin/tenants/:tenant_id/activate - Activate a tenant (SystemAdmin only).
pub async fn activate_tenant(
    State(_state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(tenant_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    if !auth.permissions.has_role(&Role::SystemAdmin) {
        return Err(ApiError::Forbidden("SystemAdmin role required".to_string()));
    }

    let _tenant_id = TenantId::from_string(&tenant_id)
        .map_err(|_| ApiError::BadRequest("Invalid tenant ID".to_string()))?;

    // TODO: Update tenant status
    Ok((StatusCode::OK, Json(serde_json::json!({"message": "Tenant activated"}))))
}

// ============================================================================
// User Management (TenantAdmin or SystemAdmin)
// ============================================================================

/// Response for user operations.
#[derive(Debug, Serialize)]
pub struct UserResponse {
    pub id: String,
    pub tenant_id: String,
    pub email: String,
    pub display_name: String,
    pub status: UserStatus,
    pub roles: Vec<String>,
    pub created_at: chrono::DateTime<Utc>,
    pub last_login_at: Option<chrono::DateTime<Utc>>,
}

impl From<User> for UserResponse {
    fn from(user: User) -> Self {
        Self {
            id: user.id.to_string(),
            tenant_id: user.tenant_id.to_string(),
            email: user.email,
            display_name: user.display_name,
            status: user.status,
            roles: user.roles.iter().map(|r| r.to_string()).collect(),
            created_at: user.created_at,
            last_login_at: user.last_login_at,
        }
    }
}

/// POST /admin/users - Create a new user (TenantAdmin or SystemAdmin).
pub async fn create_user(
    State(_state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Json(request): Json<CreateUserRequest>,
) -> Result<impl IntoResponse, ApiError> {
    // Verify admin role
    if !auth.permissions.is_admin() {
        return Err(ApiError::Forbidden("Admin role required".to_string()));
    }

    // Parse roles from request
    let roles: std::collections::HashSet<Role> = request
        .roles
        .iter()
        .map(|r| parse_role(r))
        .collect();

    // SystemAdmin can create users in any tenant
    // TenantAdmin can only create users in their own tenant
    let tenant_id = if auth.permissions.has_role(&Role::SystemAdmin) {
        // For SystemAdmin, we could accept a tenant_id in the request
        // For now, use the auth context's tenant
        auth.tenant_id.clone()
    } else {
        auth.tenant_id.clone()
    };

    let user = User::new_local(tenant_id, &request.email, &request.display_name)
        .with_roles(roles);

    // TODO: Persist user to storage

    Ok((StatusCode::CREATED, Json(UserResponse::from(user))))
}

/// GET /admin/users - List users in the current tenant.
pub async fn list_users(
    State(_state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
) -> Result<Json<Vec<UserResponse>>, ApiError> {
    if !auth.permissions.is_admin() {
        return Err(ApiError::Forbidden("Admin role required".to_string()));
    }

    // TODO: Load users from storage for auth.tenant_id
    Ok(Json(vec![]))
}

/// GET /admin/users/:user_id - Get user details.
pub async fn get_user(
    State(_state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(user_id): Path<String>,
) -> Result<Json<UserResponse>, ApiError> {
    if !auth.permissions.is_admin() {
        return Err(ApiError::Forbidden("Admin role required".to_string()));
    }

    let _user_id = UserId::from_string(&user_id)
        .map_err(|_| ApiError::BadRequest("Invalid user ID".to_string()))?;

    // TODO: Load user from storage
    Err(ApiError::NotFound("User not found".to_string()))
}

/// PUT /admin/users/:user_id - Update user.
pub async fn update_user(
    State(_state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(user_id): Path<String>,
    Json(_request): Json<UpdateUserRequest>,
) -> Result<Json<UserResponse>, ApiError> {
    if !auth.permissions.is_admin() {
        return Err(ApiError::Forbidden("Admin role required".to_string()));
    }

    let _user_id = UserId::from_string(&user_id)
        .map_err(|_| ApiError::BadRequest("Invalid user ID".to_string()))?;

    // TODO: Load, update, and persist user
    Err(ApiError::NotFound("User not found".to_string()))
}

/// DELETE /admin/users/:user_id - Delete user.
pub async fn delete_user(
    State(_state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(user_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    if !auth.permissions.is_admin() {
        return Err(ApiError::Forbidden("Admin role required".to_string()));
    }

    let _user_id = UserId::from_string(&user_id)
        .map_err(|_| ApiError::BadRequest("Invalid user ID".to_string()))?;

    // TODO: Delete user from storage
    Ok((StatusCode::NO_CONTENT, ()))
}

/// Request to assign roles to a user.
#[derive(Debug, Deserialize)]
pub struct AssignRolesRequest {
    pub roles: Vec<String>,
}

/// PUT /admin/users/:user_id/roles - Assign roles to a user.
pub async fn assign_roles(
    State(_state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(user_id): Path<String>,
    Json(request): Json<AssignRolesRequest>,
) -> Result<Json<UserResponse>, ApiError> {
    if !auth.permissions.is_admin() {
        return Err(ApiError::Forbidden("Admin role required".to_string()));
    }

    let _user_id = UserId::from_string(&user_id)
        .map_err(|_| ApiError::BadRequest("Invalid user ID".to_string()))?;

    // Parse new roles
    let _roles: std::collections::HashSet<Role> = request
        .roles
        .iter()
        .map(|r| parse_role(r))
        .collect();

    // TODO: Load user, update roles, persist
    Err(ApiError::NotFound("User not found".to_string()))
}

/// POST /admin/users/:user_id/suspend - Suspend a user.
pub async fn suspend_user(
    State(_state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(user_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    if !auth.permissions.is_admin() {
        return Err(ApiError::Forbidden("Admin role required".to_string()));
    }

    let _user_id = UserId::from_string(&user_id)
        .map_err(|_| ApiError::BadRequest("Invalid user ID".to_string()))?;

    // TODO: Load user, suspend, persist
    Ok((StatusCode::OK, Json(serde_json::json!({"message": "User suspended"}))))
}

/// POST /admin/users/:user_id/activate - Activate a user.
pub async fn activate_user(
    State(_state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    Path(user_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    if !auth.permissions.is_admin() {
        return Err(ApiError::Forbidden("Admin role required".to_string()));
    }

    let _user_id = UserId::from_string(&user_id)
        .map_err(|_| ApiError::BadRequest("Invalid user ID".to_string()))?;

    // TODO: Load user, activate, persist
    Ok((StatusCode::OK, Json(serde_json::json!({"message": "User activated"}))))
}

// ============================================================================
// Dashboard/Stats
// ============================================================================

/// GET /admin/dashboard - Get admin dashboard statistics.
pub async fn dashboard(
    State(_state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
) -> Result<Json<DashboardResponse>, ApiError> {
    if !auth.permissions.is_admin() {
        return Err(ApiError::Forbidden("Admin role required".to_string()));
    }

    // TODO: Aggregate stats from storage
    Ok(Json(DashboardResponse {
        tenant_id: auth.tenant_id.to_string(),
        user_count: 0,
        document_count: 0,
        anomaly_count: 0,
        storage_used_bytes: 0,
        api_calls_today: 0,
    }))
}

#[derive(Debug, Serialize)]
pub struct DashboardResponse {
    pub tenant_id: String,
    pub user_count: u64,
    pub document_count: u64,
    pub anomaly_count: u64,
    pub storage_used_bytes: u64,
    pub api_calls_today: u64,
}

// ============================================================================
// Helper functions
// ============================================================================

fn parse_role(role_str: &str) -> Role {
    match role_str.to_lowercase().as_str() {
        "tenant_admin" | "tenantadmin" => Role::TenantAdmin,
        "analyst" => Role::Analyst,
        "viewer" => Role::Viewer,
        "rule_manager" | "rulemanager" => Role::RuleManager,
        "system_admin" | "systemadmin" => Role::SystemAdmin,
        other => {
            if let Some(custom_name) = other.strip_prefix("custom:") {
                Role::Custom(custom_name.to_string())
            } else {
                Role::Custom(other.to_string())
            }
        }
    }
}

// Extension trait for TenantConfig builder pattern
#[allow(dead_code)]
trait TenantConfigExt {
    fn with_status(self, status: TenantStatus) -> Self;
    fn with_features(self, features: FeatureFlags) -> Self;
    fn with_rate_limits(self, limits: RateLimitConfig) -> Self;
    fn with_quotas(self, quotas: QuotaConfig) -> Self;
}

#[allow(dead_code)]
impl TenantConfigExt for TenantConfig {
    fn with_status(mut self, status: TenantStatus) -> Self {
        self.status = status;
        self
    }

    fn with_features(mut self, features: FeatureFlags) -> Self {
        self.features = features;
        self
    }

    fn with_rate_limits(mut self, limits: RateLimitConfig) -> Self {
        self.rate_limits = limits;
        self
    }

    fn with_quotas(mut self, quotas: QuotaConfig) -> Self {
        self.quotas = quotas;
        self
    }
}

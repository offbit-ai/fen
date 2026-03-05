//! JWT Authentication middleware for multi-tenant authorization.
//!
//! This module provides JWT-based authentication with tenant context extraction.

use axum::{
    extract::{Request, State},
    http::{header, StatusCode},
    middleware::Next,
    response::Response,
};
use fen_core::domain::{
    acl::{build_policies_from_roles, Role, TenantPermissions},
    cluster::TenantId,
};
use jsonwebtoken::{decode, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;

/// JWT claims structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// Subject (user ID)
    pub sub: String,
    /// Tenant ID
    pub tenant_id: String,
    /// User roles within the tenant
    pub roles: Vec<String>,
    /// Expiration timestamp (Unix epoch)
    pub exp: usize,
    /// Issued at timestamp (Unix epoch)
    pub iat: usize,
}

/// Authenticated user context extracted from JWT.
#[derive(Clone, Debug)]
pub struct AuthContext {
    /// The authenticated user's ID
    pub user_id: String,
    /// The tenant the user belongs to
    pub tenant_id: TenantId,
    /// The user's effective permissions within the tenant
    pub permissions: TenantPermissions,
}

/// Configuration for JWT authentication.
#[derive(Clone)]
pub struct AuthConfig {
    /// JWT secret key for validation
    pub jwt_secret: String,
    /// Whether to require authentication (false allows anonymous access)
    pub require_auth: bool,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            jwt_secret: String::new(),
            require_auth: true,
        }
    }
}

/// Authentication middleware that validates JWT tokens and extracts tenant context.
///
/// This middleware:
/// 1. Extracts the Bearer token from the Authorization header
/// 2. Validates the JWT signature and expiration
/// 3. Extracts tenant_id and roles from claims
/// 4. Builds effective permissions from roles
/// 5. Inserts AuthContext into request extensions
pub async fn auth_middleware(
    State(config): State<Arc<AuthConfig>>,
    mut request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Extract token from Authorization header
    let auth_header = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok());

    let token = match auth_header {
        Some(header) => header
            .strip_prefix("Bearer ")
            .ok_or(StatusCode::UNAUTHORIZED)?,
        None => {
            if config.require_auth {
                return Err(StatusCode::UNAUTHORIZED);
            } else {
                // Anonymous access allowed - skip auth
                return Ok(next.run(request).await);
            }
        }
    };

    // Validate JWT
    let claims = validate_jwt(token, &config.jwt_secret).map_err(|e| {
        tracing::warn!(error = %e, "JWT validation failed");
        StatusCode::UNAUTHORIZED
    })?;

    // Parse tenant ID
    let tenant_id = TenantId::from_string(&claims.tenant_id).map_err(|_| {
        tracing::warn!(tenant_id = %claims.tenant_id, "Invalid tenant ID in JWT");
        StatusCode::BAD_REQUEST
    })?;

    // Parse roles from claims
    let roles: HashSet<Role> = claims.roles.iter().map(|r| parse_role(r)).collect();

    // Build effective policies from roles
    let effective_policies = build_policies_from_roles(tenant_id.clone(), &roles);

    // Create permissions
    let permissions = TenantPermissions::new(tenant_id.clone(), &claims.sub)
        .with_roles(roles)
        .with_policies(effective_policies);

    // Create auth context
    let auth_context = AuthContext {
        user_id: claims.sub,
        tenant_id,
        permissions,
    };

    // Insert into request extensions
    request.extensions_mut().insert(auth_context);

    Ok(next.run(request).await)
}

/// Validate a JWT token and extract claims.
fn validate_jwt(token: &str, secret: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    let key = DecodingKey::from_secret(secret.as_bytes());
    let mut validation = Validation::default();
    // Allow some clock skew
    validation.leeway = 60;
    let token_data = decode::<Claims>(token, &key, &validation)?;
    Ok(token_data.claims)
}

/// Parse a role string into a Role enum.
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

/// Extension trait for TenantPermissions to support builder pattern with collections.
trait TenantPermissionsExt {
    fn with_roles(self, roles: HashSet<Role>) -> Self;
    fn with_policies(self, policies: Vec<fen_core::domain::acl::AclPolicy>) -> Self;
}

impl TenantPermissionsExt for TenantPermissions {
    fn with_roles(mut self, roles: HashSet<Role>) -> Self {
        self.roles = roles;
        self
    }

    fn with_policies(mut self, policies: Vec<fen_core::domain::acl::AclPolicy>) -> Self {
        self.effective_policies = policies;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonwebtoken::{encode, EncodingKey, Header};

    fn create_test_token(claims: &Claims, secret: &str) -> String {
        encode(
            &Header::default(),
            claims,
            &EncodingKey::from_secret(secret.as_bytes()),
        )
        .unwrap()
    }

    #[test]
    fn test_validate_jwt_success() {
        let secret = "test-secret";
        let claims = Claims {
            sub: "user-123".to_string(),
            tenant_id: uuid::Uuid::new_v4().to_string(),
            roles: vec!["analyst".to_string()],
            exp: (chrono::Utc::now() + chrono::Duration::hours(1)).timestamp() as usize,
            iat: chrono::Utc::now().timestamp() as usize,
        };

        let token = create_test_token(&claims, secret);
        let result = validate_jwt(&token, secret);

        assert!(result.is_ok());
        let decoded = result.unwrap();
        assert_eq!(decoded.sub, "user-123");
    }

    #[test]
    fn test_validate_jwt_invalid_secret() {
        let claims = Claims {
            sub: "user-123".to_string(),
            tenant_id: uuid::Uuid::new_v4().to_string(),
            roles: vec!["analyst".to_string()],
            exp: (chrono::Utc::now() + chrono::Duration::hours(1)).timestamp() as usize,
            iat: chrono::Utc::now().timestamp() as usize,
        };

        let token = create_test_token(&claims, "correct-secret");
        let result = validate_jwt(&token, "wrong-secret");

        assert!(result.is_err());
    }

    #[test]
    fn test_parse_role() {
        assert_eq!(parse_role("tenant_admin"), Role::TenantAdmin);
        assert_eq!(parse_role("TenantAdmin"), Role::TenantAdmin);
        assert_eq!(parse_role("analyst"), Role::Analyst);
        assert_eq!(parse_role("viewer"), Role::Viewer);
        assert_eq!(parse_role("rule_manager"), Role::RuleManager);
        assert_eq!(parse_role("system_admin"), Role::SystemAdmin);
        assert_eq!(
            parse_role("custom:auditor"),
            Role::Custom("auditor".to_string())
        );
        assert_eq!(
            parse_role("unknown_role"),
            Role::Custom("unknown_role".to_string())
        );
    }
}

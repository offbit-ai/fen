//! Authorization guards for resource-level access control.
//!
//! This module provides middleware and utilities for enforcing ACL policies
//! on API endpoints.

use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use fen_core::domain::acl::{Action, ResourceType};

use super::auth::AuthContext;

/// Error type for authorization failures.
#[derive(Debug)]
pub enum AuthzError {
    /// No authentication context found
    Unauthenticated,
    /// User lacks permission for the requested action
    Forbidden {
        resource: ResourceType,
        action: Action,
    },
}

impl IntoResponse for AuthzError {
    fn into_response(self) -> Response {
        match self {
            AuthzError::Unauthenticated => StatusCode::UNAUTHORIZED.into_response(),
            AuthzError::Forbidden { resource, action } => {
                tracing::warn!(
                    resource = %resource,
                    action = %action,
                    "Authorization denied"
                );
                StatusCode::FORBIDDEN.into_response()
            }
        }
    }
}

/// Authorization guard that checks if user can perform an action on a resource type.
///
/// Use this as middleware on routes that require specific permissions:
///
/// ```ignore
/// use fen_core::domain::acl::{ResourceType, Action};
/// use crate::middleware::authz::require_permission;
///
/// let router = Router::new()
///     .route("/invoices", get(list_invoices))
///     .layer(axum::middleware::from_fn(
///         require_permission(ResourceType::Invoice, Action::List)
///     ));
/// ```
#[allow(dead_code)]
pub fn require_permission(
    resource_type: ResourceType,
    action: Action,
) -> impl Fn(
    Request,
    Next,
) -> std::pin::Pin<
    Box<dyn std::future::Future<Output = Result<Response, AuthzError>> + Send>,
> + Clone
       + Send {
    move |request: Request, next: Next| {
        let resource_type = resource_type.clone();
        let action = action.clone();
        Box::pin(async move {
            let auth_context = request
                .extensions()
                .get::<AuthContext>()
                .ok_or(AuthzError::Unauthenticated)?;

            if !auth_context
                .permissions
                .can_perform(&resource_type, &action)
            {
                return Err(AuthzError::Forbidden {
                    resource: resource_type,
                    action,
                });
            }

            Ok(next.run(request).await)
        })
    }
}

/// Check if the authenticated user can access a specific resource.
///
/// This performs a resource-level ACL check, including pattern matching.
///
/// # Example
///
/// ```ignore
/// use fen_core::domain::acl::{ResourceType, Action};
/// use crate::middleware::authz::check_resource_access;
///
/// async fn get_invoice(
///     Extension(auth): Extension<AuthContext>,
///     Path(invoice_id): Path<String>,
/// ) -> Result<Json<Invoice>, StatusCode> {
///     check_resource_access(&auth, &ResourceType::Invoice, &invoice_id, &Action::Read)?;
///     // ... fetch and return invoice
/// }
/// ```
#[allow(dead_code)]
pub fn check_resource_access(
    auth_context: &AuthContext,
    resource_type: &ResourceType,
    resource_id: &str,
    action: &Action,
) -> Result<(), StatusCode> {
    if !auth_context
        .permissions
        .can_access_resource(resource_type, resource_id, action)
    {
        tracing::warn!(
            user_id = %auth_context.user_id,
            tenant_id = %auth_context.tenant_id,
            resource_type = %resource_type,
            resource_id = %resource_id,
            action = %action,
            "Resource access denied"
        );
        return Err(StatusCode::FORBIDDEN);
    }
    Ok(())
}

/// Check if the authenticated user has a specific role.
#[allow(dead_code)]
pub fn check_role(
    auth_context: &AuthContext,
    required_role: &fen_core::domain::acl::Role,
) -> Result<(), StatusCode> {
    if !auth_context.permissions.has_role(required_role) {
        tracing::warn!(
            user_id = %auth_context.user_id,
            required_role = %required_role,
            "Role check failed"
        );
        return Err(StatusCode::FORBIDDEN);
    }
    Ok(())
}

/// Check if the authenticated user is an admin (TenantAdmin or SystemAdmin).
#[allow(dead_code)]
pub fn require_admin(auth_context: &AuthContext) -> Result<(), StatusCode> {
    if !auth_context.permissions.is_admin() {
        tracing::warn!(
            user_id = %auth_context.user_id,
            "Admin access required"
        );
        return Err(StatusCode::FORBIDDEN);
    }
    Ok(())
}

/// Verify that a resource belongs to the authenticated user's tenant.
///
/// This is a critical security check to prevent cross-tenant data access.
#[allow(dead_code)]
pub fn verify_tenant_ownership(
    auth_context: &AuthContext,
    resource_tenant_id: &fen_core::domain::cluster::TenantId,
) -> Result<(), StatusCode> {
    if auth_context.tenant_id != *resource_tenant_id {
        tracing::error!(
            user_tenant = %auth_context.tenant_id,
            resource_tenant = %resource_tenant_id,
            "Cross-tenant access attempt detected"
        );
        return Err(StatusCode::FORBIDDEN);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use fen_core::domain::acl::{AclPolicy, TenantPermissions};
    use fen_core::domain::cluster::TenantId;
    use std::collections::HashSet;

    fn create_test_auth_context(
        resource_type: ResourceType,
        actions: HashSet<Action>,
    ) -> AuthContext {
        let tenant_id = TenantId::new();
        let policy = AclPolicy::new(
            tenant_id.clone(),
            fen_core::domain::acl::Role::Analyst,
            resource_type,
            actions,
        );
        let permissions =
            TenantPermissions::new(tenant_id.clone(), "test-user").with_policy(policy);

        AuthContext {
            user_id: "test-user".to_string(),
            tenant_id,
            permissions,
        }
    }

    #[test]
    fn test_check_resource_access_allowed() {
        let auth = create_test_auth_context(
            ResourceType::Invoice,
            HashSet::from([Action::Read, Action::List]),
        );

        let result = check_resource_access(&auth, &ResourceType::Invoice, "inv-123", &Action::Read);
        assert!(result.is_ok());
    }

    #[test]
    fn test_check_resource_access_denied() {
        let auth = create_test_auth_context(ResourceType::Invoice, HashSet::from([Action::Read]));

        let result =
            check_resource_access(&auth, &ResourceType::Invoice, "inv-123", &Action::Delete);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), StatusCode::FORBIDDEN);
    }

    #[test]
    fn test_verify_tenant_ownership_same_tenant() {
        let auth = create_test_auth_context(ResourceType::Invoice, HashSet::new());

        let result = verify_tenant_ownership(&auth, &auth.tenant_id);
        assert!(result.is_ok());
    }

    #[test]
    fn test_verify_tenant_ownership_different_tenant() {
        let auth = create_test_auth_context(ResourceType::Invoice, HashSet::new());
        let other_tenant = TenantId::new();

        let result = verify_tenant_ownership(&auth, &other_tenant);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), StatusCode::FORBIDDEN);
    }
}

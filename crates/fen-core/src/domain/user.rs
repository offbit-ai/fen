//! User domain model for multi-tenant authentication.
//!
//! Users are always scoped to a tenant and have assigned roles that
//! determine their permissions within that tenant.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

use super::acl::Role;
use super::cluster::TenantId;

/// Unique identifier for a user.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UserId(pub Uuid);

impl UserId {
    /// Create a new random user ID.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Parse a user ID from a string.
    pub fn from_string(s: &str) -> Result<Self, uuid::Error> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl Default for UserId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for UserId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// User account status.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UserStatus {
    /// User is active and can authenticate
    #[default]
    Active,
    /// User account is pending email verification
    PendingVerification,
    /// User account is suspended (admin action)
    Suspended,
    /// User account is deactivated
    Deactivated,
    /// User account is locked due to failed login attempts
    Locked,
}

/// Authentication provider type.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthProvider {
    /// Local username/password authentication
    #[default]
    Local,
    /// OAuth2/OIDC provider (e.g., Keycloak)
    Oidc {
        /// Provider name (e.g., "keycloak", "okta", "azure-ad")
        provider: String,
        /// External user ID from the provider
        external_id: String,
    },
    /// SAML provider
    Saml {
        /// Provider name
        provider: String,
        /// External user ID
        external_id: String,
    },
    /// API key authentication
    ApiKey {
        /// Key ID (not the key itself)
        key_id: String,
    },
}

/// A user account in the system.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct User {
    /// Unique user identifier
    pub id: UserId,
    /// Tenant this user belongs to
    pub tenant_id: TenantId,
    /// User's email address (unique within tenant)
    pub email: String,
    /// User's display name
    pub display_name: String,
    /// Account status
    pub status: UserStatus,
    /// Assigned roles
    pub roles: HashSet<Role>,
    /// Authentication provider
    pub auth_provider: AuthProvider,
    /// When the user was created
    pub created_at: DateTime<Utc>,
    /// When the user was last updated
    pub updated_at: DateTime<Utc>,
    /// Last login timestamp
    pub last_login_at: Option<DateTime<Utc>>,
    /// Additional metadata
    #[serde(default)]
    pub metadata: std::collections::HashMap<String, serde_json::Value>,
}

impl User {
    /// Create a new local user.
    pub fn new_local(
        tenant_id: TenantId,
        email: impl Into<String>,
        display_name: impl Into<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: UserId::new(),
            tenant_id,
            email: email.into(),
            display_name: display_name.into(),
            status: UserStatus::Active,
            roles: HashSet::new(),
            auth_provider: AuthProvider::Local,
            created_at: now,
            updated_at: now,
            last_login_at: None,
            metadata: std::collections::HashMap::new(),
        }
    }

    /// Create a new OIDC user.
    pub fn new_oidc(
        tenant_id: TenantId,
        email: impl Into<String>,
        display_name: impl Into<String>,
        provider: impl Into<String>,
        external_id: impl Into<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: UserId::new(),
            tenant_id,
            email: email.into(),
            display_name: display_name.into(),
            status: UserStatus::Active,
            roles: HashSet::new(),
            auth_provider: AuthProvider::Oidc {
                provider: provider.into(),
                external_id: external_id.into(),
            },
            created_at: now,
            updated_at: now,
            last_login_at: None,
            metadata: std::collections::HashMap::new(),
        }
    }

    /// Add a role to this user.
    pub fn with_role(mut self, role: Role) -> Self {
        self.roles.insert(role);
        self
    }

    /// Add multiple roles to this user.
    pub fn with_roles(mut self, roles: impl IntoIterator<Item = Role>) -> Self {
        self.roles.extend(roles);
        self
    }

    /// Check if user has a specific role.
    pub fn has_role(&self, role: &Role) -> bool {
        self.roles.contains(role)
    }

    /// Check if user is an admin (TenantAdmin or SystemAdmin).
    pub fn is_admin(&self) -> bool {
        self.roles.contains(&Role::TenantAdmin) || self.roles.contains(&Role::SystemAdmin)
    }

    /// Check if user can authenticate.
    pub fn can_authenticate(&self) -> bool {
        matches!(self.status, UserStatus::Active)
    }

    /// Record a login event.
    pub fn record_login(&mut self) {
        self.last_login_at = Some(Utc::now());
    }

    /// Suspend the user.
    pub fn suspend(&mut self) {
        self.status = UserStatus::Suspended;
        self.updated_at = Utc::now();
    }

    /// Reactivate the user.
    pub fn activate(&mut self) {
        self.status = UserStatus::Active;
        self.updated_at = Utc::now();
    }
}

/// Request to create a new user.
#[derive(Clone, Debug, Deserialize)]
pub struct CreateUserRequest {
    /// User's email address
    pub email: String,
    /// User's display name
    pub display_name: String,
    /// Initial roles to assign
    #[serde(default)]
    pub roles: Vec<String>,
    /// Authentication provider configuration
    #[serde(default)]
    pub auth_provider: Option<AuthProviderConfig>,
}

/// Authentication provider configuration for user creation.
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AuthProviderConfig {
    /// Local authentication
    Local,
    /// OIDC provider
    Oidc {
        provider: String,
        external_id: String,
    },
}

/// Request to update a user.
#[derive(Clone, Debug, Deserialize)]
pub struct UpdateUserRequest {
    /// New display name
    pub display_name: Option<String>,
    /// New roles (replaces existing)
    pub roles: Option<Vec<String>>,
    /// New status
    pub status: Option<UserStatus>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_local_user() {
        let tenant_id = TenantId::new();
        let user = User::new_local(tenant_id.clone(), "test@example.com", "Test User");

        assert_eq!(user.email, "test@example.com");
        assert_eq!(user.display_name, "Test User");
        assert_eq!(user.tenant_id, tenant_id);
        assert!(matches!(user.auth_provider, AuthProvider::Local));
        assert!(user.roles.is_empty());
        assert!(user.can_authenticate());
    }

    #[test]
    fn test_new_oidc_user() {
        let tenant_id = TenantId::new();
        let user = User::new_oidc(
            tenant_id,
            "test@example.com",
            "Test User",
            "keycloak",
            "ext-123",
        );

        assert!(matches!(
            user.auth_provider,
            AuthProvider::Oidc { provider, external_id }
            if provider == "keycloak" && external_id == "ext-123"
        ));
    }

    #[test]
    fn test_user_roles() {
        let tenant_id = TenantId::new();
        let user = User::new_local(tenant_id, "test@example.com", "Test User")
            .with_role(Role::Analyst)
            .with_role(Role::Viewer);

        assert!(user.has_role(&Role::Analyst));
        assert!(user.has_role(&Role::Viewer));
        assert!(!user.has_role(&Role::TenantAdmin));
        assert!(!user.is_admin());
    }

    #[test]
    fn test_admin_check() {
        let tenant_id = TenantId::new();
        let user =
            User::new_local(tenant_id, "admin@example.com", "Admin").with_role(Role::TenantAdmin);

        assert!(user.is_admin());
    }

    #[test]
    fn test_suspend_user() {
        let tenant_id = TenantId::new();
        let mut user = User::new_local(tenant_id, "test@example.com", "Test User");

        assert!(user.can_authenticate());
        user.suspend();
        assert!(!user.can_authenticate());
        user.activate();
        assert!(user.can_authenticate());
    }
}

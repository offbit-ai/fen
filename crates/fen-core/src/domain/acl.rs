//! Access Control List (ACL) types for multi-tenant authorization.
//!
//! This module provides role-based access control (RBAC) with support for:
//! - Predefined roles (TenantAdmin, Analyst, Viewer, etc.)
//! - Resource-based permissions
//! - Action-level granularity
//! - Policy conditions (time-based, IP-based, etc.)

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use super::cluster::TenantId;

/// Resource types that can be protected by ACL.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ResourceType {
    /// Invoice documents
    Invoice,
    /// Contract documents
    Contract,
    /// Raw uploaded documents
    Document,
    /// Vendor baselines
    Baseline,
    /// Detected anomalies
    Anomaly,
    /// Validation rules
    Rule,
    /// Tenant configuration
    Tenant,
    /// User accounts
    User,
}

impl std::fmt::Display for ResourceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResourceType::Invoice => write!(f, "invoice"),
            ResourceType::Contract => write!(f, "contract"),
            ResourceType::Document => write!(f, "document"),
            ResourceType::Baseline => write!(f, "baseline"),
            ResourceType::Anomaly => write!(f, "anomaly"),
            ResourceType::Rule => write!(f, "rule"),
            ResourceType::Tenant => write!(f, "tenant"),
            ResourceType::User => write!(f, "user"),
        }
    }
}

/// Actions that can be performed on resources.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Action {
    /// Create new resource
    Create,
    /// Read/view resource
    Read,
    /// Update existing resource
    Update,
    /// Delete resource
    Delete,
    /// List resources
    List,
    /// Export resource data
    Export,
    /// Approve an item (e.g., anomaly resolution)
    Approve,
    /// Reject an item
    Reject,
}

impl std::fmt::Display for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Action::Create => write!(f, "create"),
            Action::Read => write!(f, "read"),
            Action::Update => write!(f, "update"),
            Action::Delete => write!(f, "delete"),
            Action::List => write!(f, "list"),
            Action::Export => write!(f, "export"),
            Action::Approve => write!(f, "approve"),
            Action::Reject => write!(f, "reject"),
        }
    }
}

/// Role-based permissions.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Role {
    /// Full access to tenant resources
    TenantAdmin,
    /// Can view and process documents
    Analyst,
    /// Read-only access
    Viewer,
    /// Can manage rules and baselines
    RuleManager,
    /// System-level admin (cross-tenant)
    SystemAdmin,
    /// Custom role with specific permissions
    Custom(String),
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Role::TenantAdmin => write!(f, "tenant_admin"),
            Role::Analyst => write!(f, "analyst"),
            Role::Viewer => write!(f, "viewer"),
            Role::RuleManager => write!(f, "rule_manager"),
            Role::SystemAdmin => write!(f, "system_admin"),
            Role::Custom(name) => write!(f, "custom:{}", name),
        }
    }
}

/// Conditions that must be met for a policy to apply.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PolicyCondition {
    /// Time window when policy is active (UTC hours)
    TimeWindow { start_hour: u8, end_hour: u8 },
    /// IP allowlist (CIDR notation supported)
    IpAllowlist(Vec<String>),
    /// Minimum confidence score for anomaly access
    MinConfidence(f32),
    /// Custom attribute check
    AttributeMatch { key: String, value: String },
}

/// An ACL policy entry.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AclPolicy {
    /// Unique policy identifier
    pub id: String,
    /// Tenant this policy belongs to
    pub tenant_id: TenantId,
    /// Role this policy applies to
    pub role: Role,
    /// Resource type this policy covers
    pub resource_type: ResourceType,
    /// Allowed actions
    pub actions: HashSet<Action>,
    /// Optional resource ID pattern (supports wildcards like "inv-*")
    pub resource_pattern: Option<String>,
    /// Optional conditions (all must be satisfied)
    pub conditions: Vec<PolicyCondition>,
}

impl AclPolicy {
    /// Create a new policy for a role on a resource type.
    pub fn new(
        tenant_id: TenantId,
        role: Role,
        resource_type: ResourceType,
        actions: HashSet<Action>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            tenant_id,
            role,
            resource_type,
            actions,
            resource_pattern: None,
            conditions: Vec::new(),
        }
    }

    /// Set a resource pattern filter.
    pub fn with_pattern(mut self, pattern: impl Into<String>) -> Self {
        self.resource_pattern = Some(pattern.into());
        self
    }

    /// Add a condition to this policy.
    pub fn with_condition(mut self, condition: PolicyCondition) -> Self {
        self.conditions.push(condition);
        self
    }
}

/// User's effective permissions within a tenant.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TenantPermissions {
    /// Tenant ID
    pub tenant_id: TenantId,
    /// User ID
    pub user_id: String,
    /// Assigned roles
    pub roles: HashSet<Role>,
    /// Effective policies (computed from roles)
    pub effective_policies: Vec<AclPolicy>,
}

impl TenantPermissions {
    /// Create new permissions for a user.
    pub fn new(tenant_id: TenantId, user_id: impl Into<String>) -> Self {
        Self {
            tenant_id,
            user_id: user_id.into(),
            roles: HashSet::new(),
            effective_policies: Vec::new(),
        }
    }

    /// Add a role to this user.
    pub fn with_role(mut self, role: Role) -> Self {
        self.roles.insert(role);
        self
    }

    /// Add an effective policy.
    pub fn with_policy(mut self, policy: AclPolicy) -> Self {
        self.effective_policies.push(policy);
        self
    }

    /// Check if user can perform action on resource type.
    pub fn can_perform(&self, resource_type: &ResourceType, action: &Action) -> bool {
        self.effective_policies
            .iter()
            .any(|policy| policy.resource_type == *resource_type && policy.actions.contains(action))
    }

    /// Check if user can access a specific resource.
    pub fn can_access_resource(
        &self,
        resource_type: &ResourceType,
        resource_id: &str,
        action: &Action,
    ) -> bool {
        self.effective_policies.iter().any(|policy| {
            if policy.resource_type != *resource_type || !policy.actions.contains(action) {
                return false;
            }
            match &policy.resource_pattern {
                None => true, // No pattern = all resources
                Some(pattern) => Self::matches_pattern(pattern, resource_id),
            }
        })
    }

    /// Check if a resource ID matches a pattern.
    fn matches_pattern(pattern: &str, resource_id: &str) -> bool {
        if pattern == "*" {
            return true;
        }
        if let Some(prefix) = pattern.strip_suffix('*') {
            return resource_id.starts_with(prefix);
        }
        pattern == resource_id
    }

    /// Check if user has a specific role.
    pub fn has_role(&self, role: &Role) -> bool {
        self.roles.contains(role)
    }

    /// Check if user is a tenant admin.
    pub fn is_admin(&self) -> bool {
        self.roles.contains(&Role::TenantAdmin) || self.roles.contains(&Role::SystemAdmin)
    }
}

/// Get default permissions for a role.
pub fn default_role_permissions(role: &Role) -> Vec<(ResourceType, HashSet<Action>)> {
    match role {
        Role::TenantAdmin => vec![
            (
                ResourceType::Invoice,
                HashSet::from([
                    Action::Create,
                    Action::Read,
                    Action::Update,
                    Action::Delete,
                    Action::List,
                    Action::Export,
                ]),
            ),
            (
                ResourceType::Contract,
                HashSet::from([
                    Action::Create,
                    Action::Read,
                    Action::Update,
                    Action::Delete,
                    Action::List,
                ]),
            ),
            (
                ResourceType::Document,
                HashSet::from([Action::Create, Action::Read, Action::Delete, Action::List]),
            ),
            (
                ResourceType::Baseline,
                HashSet::from([Action::Read, Action::List]),
            ),
            (
                ResourceType::Anomaly,
                HashSet::from([Action::Read, Action::List, Action::Approve, Action::Reject]),
            ),
            (
                ResourceType::Rule,
                HashSet::from([
                    Action::Create,
                    Action::Read,
                    Action::Update,
                    Action::Delete,
                    Action::List,
                ]),
            ),
            (
                ResourceType::User,
                HashSet::from([
                    Action::Create,
                    Action::Read,
                    Action::Update,
                    Action::Delete,
                    Action::List,
                ]),
            ),
        ],
        Role::Analyst => vec![
            (
                ResourceType::Invoice,
                HashSet::from([Action::Create, Action::Read, Action::Update, Action::List]),
            ),
            (
                ResourceType::Contract,
                HashSet::from([Action::Read, Action::List]),
            ),
            (
                ResourceType::Document,
                HashSet::from([Action::Create, Action::Read, Action::List]),
            ),
            (
                ResourceType::Anomaly,
                HashSet::from([Action::Read, Action::List, Action::Approve, Action::Reject]),
            ),
        ],
        Role::Viewer => vec![
            (
                ResourceType::Invoice,
                HashSet::from([Action::Read, Action::List]),
            ),
            (
                ResourceType::Contract,
                HashSet::from([Action::Read, Action::List]),
            ),
            (
                ResourceType::Anomaly,
                HashSet::from([Action::Read, Action::List]),
            ),
        ],
        Role::RuleManager => vec![
            (
                ResourceType::Rule,
                HashSet::from([
                    Action::Create,
                    Action::Read,
                    Action::Update,
                    Action::Delete,
                    Action::List,
                ]),
            ),
            (
                ResourceType::Baseline,
                HashSet::from([Action::Read, Action::Update, Action::List]),
            ),
        ],
        Role::SystemAdmin => vec![(
            ResourceType::Tenant,
            HashSet::from([
                Action::Create,
                Action::Read,
                Action::Update,
                Action::Delete,
                Action::List,
            ]),
        )],
        Role::Custom(_) => vec![], // Custom roles have explicitly assigned permissions
    }
}

/// Build effective policies from roles.
pub fn build_policies_from_roles(tenant_id: TenantId, roles: &HashSet<Role>) -> Vec<AclPolicy> {
    let mut policies = Vec::new();

    for role in roles {
        for (resource_type, actions) in default_role_permissions(role) {
            policies.push(AclPolicy::new(
                tenant_id.clone(),
                role.clone(),
                resource_type,
                actions,
            ));
        }
    }

    policies
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_can_perform() {
        let tenant_id = TenantId::new();
        let mut permissions = TenantPermissions::new(tenant_id.clone(), "user-1");

        let policy = AclPolicy::new(
            tenant_id,
            Role::Analyst,
            ResourceType::Invoice,
            HashSet::from([Action::Read, Action::List]),
        );
        permissions = permissions.with_policy(policy);

        assert!(permissions.can_perform(&ResourceType::Invoice, &Action::Read));
        assert!(permissions.can_perform(&ResourceType::Invoice, &Action::List));
        assert!(!permissions.can_perform(&ResourceType::Invoice, &Action::Delete));
        assert!(!permissions.can_perform(&ResourceType::Contract, &Action::Read));
    }

    #[test]
    fn test_can_access_resource_with_pattern() {
        let tenant_id = TenantId::new();
        let mut permissions = TenantPermissions::new(tenant_id.clone(), "user-1");

        let policy = AclPolicy::new(
            tenant_id,
            Role::Analyst,
            ResourceType::Invoice,
            HashSet::from([Action::Read]),
        )
        .with_pattern("inv-2024-*");
        permissions = permissions.with_policy(policy);

        assert!(permissions.can_access_resource(
            &ResourceType::Invoice,
            "inv-2024-001",
            &Action::Read
        ));
        assert!(permissions.can_access_resource(
            &ResourceType::Invoice,
            "inv-2024-999",
            &Action::Read
        ));
        assert!(!permissions.can_access_resource(
            &ResourceType::Invoice,
            "inv-2023-001",
            &Action::Read
        ));
    }

    #[test]
    fn test_wildcard_pattern() {
        assert!(TenantPermissions::matches_pattern("*", "anything"));
        assert!(TenantPermissions::matches_pattern("inv-*", "inv-123"));
        assert!(!TenantPermissions::matches_pattern("inv-*", "con-123"));
        assert!(TenantPermissions::matches_pattern("exact", "exact"));
        assert!(!TenantPermissions::matches_pattern("exact", "other"));
    }

    #[test]
    fn test_build_policies_from_roles() {
        let tenant_id = TenantId::new();
        let roles = HashSet::from([Role::Viewer]);
        let policies = build_policies_from_roles(tenant_id, &roles);

        assert!(!policies.is_empty());
        assert!(policies
            .iter()
            .any(|p| p.resource_type == ResourceType::Invoice));
    }
}

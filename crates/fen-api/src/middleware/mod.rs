mod auth;
mod authz;
pub mod metrics;
mod rate_limit;

pub use auth::{auth_middleware, AuthConfig, AuthContext, Claims};
#[allow(unused_imports)]
pub use authz::{
    check_resource_access, check_role, require_admin, require_permission, verify_tenant_ownership,
    AuthzError,
};
pub use rate_limit::RateLimitLayer;

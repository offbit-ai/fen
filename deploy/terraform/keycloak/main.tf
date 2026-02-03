# Keycloak Terraform Configuration for Fen
#
# This configuration manages the Keycloak realm, clients, roles, and users
# for the Fen Invoice Intelligence platform.
#
# Usage:
#   cd deploy/terraform/keycloak
#   terraform init
#   terraform plan
#   terraform apply
#
# Prerequisites:
#   - Keycloak running (docker-compose -f deploy/docker/docker-compose.keycloak.yml up -d)
#   - Set KEYCLOAK_USER and KEYCLOAK_PASSWORD environment variables

terraform {
  required_version = ">= 1.0"

  required_providers {
    keycloak = {
      source  = "mrparkers/keycloak"
      version = ">= 4.0.0"
    }
  }
}

provider "keycloak" {
  client_id = "admin-cli"
  url       = var.keycloak_url
  username  = var.keycloak_admin_user
  password  = var.keycloak_admin_password
}

# =============================================================================
# Realm
# =============================================================================

resource "keycloak_realm" "fen" {
  realm                    = "fen"
  enabled                  = true
  display_name             = "Fen Invoice Intelligence"
  display_name_html        = "<b>Fen</b> Invoice Intelligence"
  login_with_email_allowed = true
  duplicate_emails_allowed = false
  reset_password_allowed   = true
  edit_username_allowed    = false

  # Security settings
  ssl_required    = var.ssl_required
  login_theme     = "keycloak"
  account_theme   = "keycloak"
  admin_theme     = "keycloak"
  email_theme     = "keycloak"

  # Brute force protection
  brute_force_detection {
    permanent_lockout                = false
    max_login_failures               = 5
    wait_increment_seconds           = 60
    quick_login_check_milli_seconds  = 1000
    minimum_quick_login_wait_seconds = 60
    max_failure_wait_seconds         = 900
    failure_reset_time_seconds       = 43200
  }

  # Token settings
  access_token_lifespan               = "1h"
  access_token_lifespan_for_implicit_flow = "15m"
  sso_session_idle_timeout            = "30m"
  sso_session_max_lifespan            = "10h"
  offline_session_idle_timeout        = "720h"
  offline_session_max_lifespan_enabled = true
  offline_session_max_lifespan        = "1440h"

  # Internationalization
  internationalization {
    supported_locales = ["en"]
    default_locale    = "en"
  }
}

# =============================================================================
# Realm Roles
# =============================================================================

resource "keycloak_role" "system_admin" {
  realm_id    = keycloak_realm.fen.id
  name        = "system_admin"
  description = "System administrator with full access to all tenants"
}

resource "keycloak_role" "tenant_admin" {
  realm_id    = keycloak_realm.fen.id
  name        = "tenant_admin"
  description = "Tenant administrator with full access to their tenant"
}

resource "keycloak_role" "analyst" {
  realm_id    = keycloak_realm.fen.id
  name        = "analyst"
  description = "Can view and process documents, approve anomalies"
}

resource "keycloak_role" "viewer" {
  realm_id    = keycloak_realm.fen.id
  name        = "viewer"
  description = "Read-only access to documents and reports"
}

resource "keycloak_role" "rule_manager" {
  realm_id    = keycloak_realm.fen.id
  name        = "rule_manager"
  description = "Can manage validation rules and baselines"
}

# =============================================================================
# OIDC Clients
# =============================================================================

# Backend API Client (Confidential)
resource "keycloak_openid_client" "fen_api" {
  realm_id  = keycloak_realm.fen.id
  client_id = "fen-api"
  name      = "Fen API"
  enabled   = true

  access_type                  = "CONFIDENTIAL"
  client_secret                = var.fen_api_client_secret
  standard_flow_enabled        = true
  implicit_flow_enabled        = false
  direct_access_grants_enabled = true
  service_accounts_enabled     = false

  valid_redirect_uris = var.api_redirect_uris
  web_origins         = var.api_web_origins

  login_theme = "keycloak"

  access_token_lifespan = "3600"
}

# Frontend Web Client (Public with PKCE)
resource "keycloak_openid_client" "fen_web" {
  realm_id  = keycloak_realm.fen.id
  client_id = "fen-web"
  name      = "Fen Web Application"
  enabled   = true

  access_type                  = "PUBLIC"
  standard_flow_enabled        = true
  implicit_flow_enabled        = false
  direct_access_grants_enabled = false
  service_accounts_enabled     = false

  # PKCE required for public clients
  pkce_code_challenge_method = "S256"

  valid_redirect_uris          = var.web_redirect_uris
  valid_post_logout_redirect_uris = var.web_redirect_uris
  web_origins                  = var.web_origins

  login_theme = "keycloak"

  access_token_lifespan = "3600"
}

# =============================================================================
# Protocol Mappers - Custom Claims
# =============================================================================

# tenant_id claim for API client
resource "keycloak_openid_user_attribute_protocol_mapper" "api_tenant_id" {
  realm_id  = keycloak_realm.fen.id
  client_id = keycloak_openid_client.fen_api.id
  name      = "tenant_id"

  user_attribute       = "tenant_id"
  claim_name           = "tenant_id"
  claim_value_type     = "String"
  add_to_id_token      = true
  add_to_access_token  = true
  add_to_userinfo      = true
}

# realm roles claim for API client
resource "keycloak_openid_user_realm_role_protocol_mapper" "api_roles" {
  realm_id  = keycloak_realm.fen.id
  client_id = keycloak_openid_client.fen_api.id
  name      = "realm_roles"

  claim_name          = "roles"
  claim_value_type    = "String"
  multivalued         = true
  add_to_id_token     = true
  add_to_access_token = true
  add_to_userinfo     = true
}

# tenant_id claim for Web client
resource "keycloak_openid_user_attribute_protocol_mapper" "web_tenant_id" {
  realm_id  = keycloak_realm.fen.id
  client_id = keycloak_openid_client.fen_web.id
  name      = "tenant_id"

  user_attribute       = "tenant_id"
  claim_name           = "tenant_id"
  claim_value_type     = "String"
  add_to_id_token      = true
  add_to_access_token  = true
  add_to_userinfo      = true
}

# realm roles claim for Web client
resource "keycloak_openid_user_realm_role_protocol_mapper" "web_roles" {
  realm_id  = keycloak_realm.fen.id
  client_id = keycloak_openid_client.fen_web.id
  name      = "realm_roles"

  claim_name          = "roles"
  claim_value_type    = "String"
  multivalued         = true
  add_to_id_token     = true
  add_to_access_token = true
  add_to_userinfo     = true
}

# =============================================================================
# Default Users (for development/testing)
# =============================================================================

resource "keycloak_user" "admin" {
  count      = var.create_test_users ? 1 : 0
  realm_id   = keycloak_realm.fen.id
  username   = "admin"
  enabled    = true
  email      = "admin@fen.local"
  first_name = "System"
  last_name  = "Admin"
  email_verified = true

  attributes = {
    tenant_id = "00000000-0000-0000-0000-000000000000"
  }

  initial_password {
    value     = var.admin_password
    temporary = false
  }
}

resource "keycloak_user_roles" "admin_roles" {
  count    = var.create_test_users ? 1 : 0
  realm_id = keycloak_realm.fen.id
  user_id  = keycloak_user.admin[0].id
  role_ids = [keycloak_role.system_admin.id]
}

resource "keycloak_user" "tenant_admin" {
  count      = var.create_test_users ? 1 : 0
  realm_id   = keycloak_realm.fen.id
  username   = "tenant-admin"
  enabled    = true
  email      = "tenant-admin@acme.local"
  first_name = "Tenant"
  last_name  = "Admin"
  email_verified = true

  attributes = {
    tenant_id = "11111111-1111-1111-1111-111111111111"
  }

  initial_password {
    value     = var.tenant_admin_password
    temporary = false
  }
}

resource "keycloak_user_roles" "tenant_admin_roles" {
  count    = var.create_test_users ? 1 : 0
  realm_id = keycloak_realm.fen.id
  user_id  = keycloak_user.tenant_admin[0].id
  role_ids = [keycloak_role.tenant_admin.id]
}

resource "keycloak_user" "analyst" {
  count      = var.create_test_users ? 1 : 0
  realm_id   = keycloak_realm.fen.id
  username   = "analyst"
  enabled    = true
  email      = "analyst@acme.local"
  first_name = "Jane"
  last_name  = "Analyst"
  email_verified = true

  attributes = {
    tenant_id = "11111111-1111-1111-1111-111111111111"
  }

  initial_password {
    value     = var.analyst_password
    temporary = false
  }
}

resource "keycloak_user_roles" "analyst_roles" {
  count    = var.create_test_users ? 1 : 0
  realm_id = keycloak_realm.fen.id
  user_id  = keycloak_user.analyst[0].id
  role_ids = [keycloak_role.analyst.id]
}

resource "keycloak_user" "viewer" {
  count      = var.create_test_users ? 1 : 0
  realm_id   = keycloak_realm.fen.id
  username   = "viewer"
  enabled    = true
  email      = "viewer@acme.local"
  first_name = "Bob"
  last_name  = "Viewer"
  email_verified = true

  attributes = {
    tenant_id = "11111111-1111-1111-1111-111111111111"
  }

  initial_password {
    value     = var.viewer_password
    temporary = false
  }
}

resource "keycloak_user_roles" "viewer_roles" {
  count    = var.create_test_users ? 1 : 0
  realm_id = keycloak_realm.fen.id
  user_id  = keycloak_user.viewer[0].id
  role_ids = [keycloak_role.viewer.id]
}

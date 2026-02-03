# Keycloak Terraform Variables

# =============================================================================
# Keycloak Connection
# =============================================================================

variable "keycloak_url" {
  description = "URL of the Keycloak server"
  type        = string
  default     = "http://localhost:8080"
}

variable "keycloak_admin_user" {
  description = "Keycloak admin username"
  type        = string
  default     = "admin"
  sensitive   = true
}

variable "keycloak_admin_password" {
  description = "Keycloak admin password"
  type        = string
  sensitive   = true
}

# =============================================================================
# Realm Settings
# =============================================================================

variable "ssl_required" {
  description = "SSL requirement for the realm (none, external, all)"
  type        = string
  default     = "none"  # Use "external" or "all" in production
}

# =============================================================================
# Client Configuration
# =============================================================================

variable "fen_api_client_secret" {
  description = "Client secret for the fen-api client"
  type        = string
  default     = "fen-api-secret"
  sensitive   = true
}

variable "api_redirect_uris" {
  description = "Valid redirect URIs for the API client"
  type        = list(string)
  default     = [
    "http://localhost:3000/*",
    "http://localhost:8000/*"
  ]
}

variable "api_web_origins" {
  description = "Allowed web origins for the API client"
  type        = list(string)
  default     = [
    "http://localhost:3000",
    "http://localhost:8000"
  ]
}

variable "web_redirect_uris" {
  description = "Valid redirect URIs for the web client"
  type        = list(string)
  default     = [
    "http://localhost:3001/*",
    "http://localhost:5173/*"
  ]
}

variable "web_origins" {
  description = "Allowed web origins for the web client"
  type        = list(string)
  default     = [
    "http://localhost:3001",
    "http://localhost:5173"
  ]
}

# =============================================================================
# Test Users
# =============================================================================

variable "create_test_users" {
  description = "Whether to create test users (disable in production)"
  type        = bool
  default     = true
}

variable "admin_password" {
  description = "Password for the admin test user"
  type        = string
  default     = "admin123"
  sensitive   = true
}

variable "tenant_admin_password" {
  description = "Password for the tenant-admin test user"
  type        = string
  default     = "tenant123"
  sensitive   = true
}

variable "analyst_password" {
  description = "Password for the analyst test user"
  type        = string
  default     = "analyst123"
  sensitive   = true
}

variable "viewer_password" {
  description = "Password for the viewer test user"
  type        = string
  default     = "viewer123"
  sensitive   = true
}

# =============================================================================
# Environment-specific Overrides
# =============================================================================

variable "environment" {
  description = "Environment name (dev, staging, prod)"
  type        = string
  default     = "dev"

  validation {
    condition     = contains(["dev", "staging", "prod"], var.environment)
    error_message = "Environment must be one of: dev, staging, prod"
  }
}

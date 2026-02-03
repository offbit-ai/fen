# Production Terraform variables for Keycloak
#
# IMPORTANT: This file contains only non-sensitive, static configuration.
# All environment-specific and sensitive values should be set via
# environment variables using the TF_VAR_ prefix.
#
# Usage:
#   1. Copy .env.terraform.prod.example to .env.terraform.prod
#   2. Configure values in .env.terraform.prod
#   3. Run: source .env.terraform.prod && terraform apply -var-file=environments/prod.tfvars
#
# Or using set -a for automatic export:
#   set -a && source .env.terraform.prod && set +a
#   terraform apply -var-file=environments/prod.tfvars

# =============================================================================
# Environment
# =============================================================================

environment = "prod"

# Require SSL for all connections in production
ssl_required = "all"

# =============================================================================
# Test Users - DISABLED in production
# =============================================================================

create_test_users = false

# =============================================================================
# All other values should come from environment variables:
#
# Required (set in .env.terraform.prod):
#   TF_VAR_keycloak_url          - Keycloak server URL
#   TF_VAR_keycloak_admin_password - Admin password
#   TF_VAR_fen_api_client_secret  - API client secret (generate with: openssl rand -base64 32)
#   TF_VAR_api_redirect_uris      - API redirect URIs (JSON array)
#   TF_VAR_api_web_origins        - API web origins (JSON array)
#   TF_VAR_web_redirect_uris      - Web redirect URIs (JSON array)
#   TF_VAR_web_origins            - Web origins (JSON array)
#
# Optional:
#   TF_VAR_keycloak_admin_user    - Admin username (default: admin)
# =============================================================================

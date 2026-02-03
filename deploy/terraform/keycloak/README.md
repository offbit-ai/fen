# Keycloak Terraform Configuration

Infrastructure-as-code for managing Keycloak authentication configuration for the Fen platform.

## Prerequisites

1. **Terraform** >= 1.0
2. **Keycloak** running and accessible

## Quick Start (Local Development)

1. Start Keycloak:
   ```bash
   docker-compose -f deploy/docker/docker-compose.full.yml up -d keycloak
   ```

2. Wait for Keycloak to be ready:
   ```bash
   until curl -s http://localhost:8080/health/ready | grep -q UP; do sleep 2; done
   ```

3. Initialize Terraform:
   ```bash
   cd deploy/terraform/keycloak
   terraform init
   ```

4. Configure environment variables:
   ```bash
   cp .env.terraform.example .env.terraform
   # Edit .env.terraform as needed (defaults work for local dev)
   ```

5. Plan and apply:
   ```bash
   source .env.terraform && terraform plan
   source .env.terraform && terraform apply
   ```

6. Get configuration for fen-api:
   ```bash
   terraform output -raw env_file_content > .env.keycloak
   ```

## Configuration

### Environment Variables

All configuration is done via environment variables using the `TF_VAR_` prefix. This keeps sensitive values out of version control.

#### Development (`.env.terraform.example`)

Copy to `.env.terraform` and source before running Terraform:

```bash
# Keycloak Connection
export TF_VAR_keycloak_url="http://localhost:8080"
export TF_VAR_keycloak_admin_user="admin"
export TF_VAR_keycloak_admin_password="admin"

# Environment
export TF_VAR_environment="dev"
export TF_VAR_ssl_required="none"

# Client Secrets
export TF_VAR_fen_api_client_secret="fen-api-secret"

# Redirect URIs (JSON arrays)
export TF_VAR_api_redirect_uris='["http://localhost:3000/*", "http://localhost:8000/*"]'
export TF_VAR_api_web_origins='["http://localhost:3000", "http://localhost:8000"]'
export TF_VAR_web_redirect_uris='["http://localhost:3001/*", "http://localhost:5173/*"]'
export TF_VAR_web_origins='["http://localhost:3001", "http://localhost:5173"]'

# Test Users
export TF_VAR_create_test_users="true"
export TF_VAR_admin_password="admin123"
```

#### Production (`.env.terraform.prod.example`)

Copy to `.env.terraform.prod` and configure with your production values:

```bash
# Keycloak Connection
export TF_VAR_keycloak_url="https://auth.yourdomain.com"
export TF_VAR_keycloak_admin_password=""  # Set securely!

# Environment
export TF_VAR_environment="prod"
export TF_VAR_ssl_required="all"

# Client Secrets (generate with: openssl rand -base64 32)
export TF_VAR_fen_api_client_secret=""  # Generate a strong secret!

# Redirect URIs
export TF_VAR_api_redirect_uris='["https://api.yourdomain.com/*"]'
export TF_VAR_api_web_origins='["https://api.yourdomain.com"]'
export TF_VAR_web_redirect_uris='["https://app.yourdomain.com/*"]'
export TF_VAR_web_origins='["https://app.yourdomain.com"]'

# Test Users - DISABLED in production
export TF_VAR_create_test_users="false"
```

### Variables Reference

| Variable | Description | Default |
|----------|-------------|---------|
| `keycloak_url` | Keycloak server URL | `http://localhost:8080` |
| `keycloak_admin_user` | Admin username | `admin` |
| `keycloak_admin_password` | Admin password | (required) |
| `environment` | Environment name | `dev` |
| `ssl_required` | SSL requirement (`none`, `external`, `all`) | `none` |
| `fen_api_client_secret` | API client secret | `fen-api-secret` |
| `api_redirect_uris` | API client redirect URIs | `["http://localhost:3000/*"]` |
| `api_web_origins` | API client web origins | `["http://localhost:3000"]` |
| `web_redirect_uris` | Web client redirect URIs | `["http://localhost:3001/*"]` |
| `web_origins` | Web client web origins | `["http://localhost:3001"]` |
| `create_test_users` | Create test users | `true` |

### Environment-specific Configuration

```bash
# Development (default)
source .env.terraform && terraform apply

# Production
source .env.terraform.prod && terraform apply -var-file=environments/prod.tfvars
```

## Resources Created

### Realm
- **fen** - Main authentication realm

### Roles
- `system_admin` - Full system access
- `tenant_admin` - Full tenant access
- `analyst` - Document processing
- `viewer` - Read-only access
- `rule_manager` - Rule configuration

### Clients
- **fen-api** - Backend API (confidential)
- **fen-web** - Frontend app (public with PKCE)

### Protocol Mappers
- `tenant_id` - User's tenant ID claim
- `roles` - User's realm roles

### Test Users (dev only)

| Username | Password | Role | Tenant ID |
|----------|----------|------|-----------|
| admin | admin123 | system_admin | 00000000-0000-0000-0000-000000000000 |
| tenant-admin | tenant123 | tenant_admin | 11111111-1111-1111-1111-111111111111 |
| analyst | analyst123 | analyst | 11111111-1111-1111-1111-111111111111 |
| viewer | viewer123 | viewer | 11111111-1111-1111-1111-111111111111 |

## Outputs

```bash
# Get OIDC issuer URL
terraform output oidc_issuer_url

# Get all OIDC endpoints
terraform output -json

# Generate .env file for fen-api
terraform output -raw env_file_content
```

## Production Considerations

1. **SSL**: Set `ssl_required = "all"` via `TF_VAR_ssl_required`
2. **Secrets**: Generate strong secrets with `openssl rand -base64 32`
3. **Test Users**: Set `create_test_users = false`
4. **State**: Use remote backend (S3, GCS, etc.)
5. **Environment Files**: Never commit `.env.terraform.prod` (it's gitignored)

### Remote State Example

```hcl
terraform {
  backend "s3" {
    bucket = "fen-terraform-state"
    key    = "keycloak/terraform.tfstate"
    region = "us-east-1"
  }
}
```

## Importing Existing Resources

If you have an existing Keycloak realm:

```bash
# Import realm
terraform import keycloak_realm.fen fen

# Import client
terraform import keycloak_openid_client.fen_api fen/fen-api
```

## Troubleshooting

### Connection refused
Ensure Keycloak is running and accessible at the configured URL.

### 401 Unauthorized
Check admin credentials in your `.env.terraform` file.

### Realm already exists
Import the existing realm:
```bash
terraform import keycloak_realm.fen fen
```

### Variable not set

Ensure you sourced the environment file before running Terraform:
```bash
source .env.terraform && terraform plan
```

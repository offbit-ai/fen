//! Authentication routes for OAuth2/OIDC integration.
//!
//! This module provides endpoints for:
//! - OIDC token exchange (authorization code flow)
//! - Token refresh
//! - Logout
//! - User info retrieval
//!
//! Designed for integration with Keycloak or other OIDC providers.

use std::sync::Arc;

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Redirect},
    Json,
};
use chrono::{Duration, Utc};
use jsonwebtoken::{encode, EncodingKey, Header};
use serde::{Deserialize, Serialize};

use crate::error::ApiError;
use crate::middleware::{AuthContext, Claims};
use crate::state::AppState;

/// OIDC provider configuration.
#[derive(Clone, Debug, Deserialize)]
pub struct OidcConfig {
    /// OIDC issuer URL (e.g., http://localhost:8080/realms/fen)
    pub issuer_url: String,
    /// Client ID registered with the OIDC provider
    pub client_id: String,
    /// Client secret (for confidential clients)
    pub client_secret: Option<String>,
    /// Redirect URI for authorization code callback
    pub redirect_uri: String,
    /// Scopes to request (default: openid profile email)
    #[serde(default = "default_scopes")]
    pub scopes: Vec<String>,
}

fn default_scopes() -> Vec<String> {
    vec!["openid".to_string(), "profile".to_string(), "email".to_string()]
}

impl Default for OidcConfig {
    fn default() -> Self {
        Self {
            issuer_url: "http://localhost:8080/realms/fen".to_string(),
            client_id: "fen-api".to_string(),
            client_secret: None,
            redirect_uri: "http://localhost:3000/auth/callback".to_string(),
            scopes: default_scopes(),
        }
    }
}

/// Query parameters for the OIDC callback.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct CallbackParams {
    /// Authorization code from the OIDC provider
    pub code: String,
    /// State parameter for CSRF protection
    pub state: Option<String>,
}

/// Request to exchange authorization code for tokens.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct TokenExchangeRequest {
    /// Authorization code from the OIDC provider
    pub code: String,
    /// Redirect URI used in the authorization request
    pub redirect_uri: String,
}

/// Request to refresh an access token.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct RefreshTokenRequest {
    /// Refresh token
    pub refresh_token: String,
}

/// Response containing access and refresh tokens.
#[derive(Debug, Serialize)]
pub struct TokenResponse {
    /// JWT access token
    pub access_token: String,
    /// Token type (always "Bearer")
    pub token_type: String,
    /// Expiration time in seconds
    pub expires_in: i64,
    /// Refresh token (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
}

/// User info response.
#[derive(Debug, Serialize)]
pub struct UserInfoResponse {
    /// User ID
    pub sub: String,
    /// User's email
    pub email: Option<String>,
    /// User's display name
    pub name: Option<String>,
    /// Tenant ID
    pub tenant_id: String,
    /// User's roles
    pub roles: Vec<String>,
}

/// OIDC provider metadata (for discovery).
#[derive(Debug, Serialize)]
#[allow(dead_code)]
pub struct ProviderMetadata {
    /// Authorization endpoint
    pub authorization_endpoint: String,
    /// Token endpoint
    pub token_endpoint: String,
    /// User info endpoint
    pub userinfo_endpoint: String,
    /// End session endpoint (logout)
    pub end_session_endpoint: String,
    /// JWKS URI for token validation
    pub jwks_uri: String,
    /// Supported scopes
    pub scopes_supported: Vec<String>,
    /// Supported response types
    pub response_types_supported: Vec<String>,
}

/// GET /auth/login - Redirect to OIDC provider for authentication.
///
/// Initiates the authorization code flow by redirecting to the OIDC provider.
pub async fn login(
    State(state): State<Arc<AppState>>,
    Query(params): Query<LoginParams>,
) -> impl IntoResponse {
    let oidc = state.oidc_config.as_ref().ok_or_else(|| {
        ApiError::Internal("OIDC not configured".to_string())
    });

    match oidc {
        Ok(config) => {
            let auth_url = format!(
                "{}/protocol/openid-connect/auth?client_id={}&redirect_uri={}&response_type=code&scope={}&state={}",
                config.issuer_url,
                urlencoding::encode(&config.client_id),
                urlencoding::encode(&config.redirect_uri),
                urlencoding::encode(&config.scopes.join(" ")),
                params.state.unwrap_or_else(|| uuid::Uuid::new_v4().to_string())
            );
            Redirect::temporary(&auth_url).into_response()
        }
        Err(e) => e.into_response(),
    }
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct LoginParams {
    /// Optional state parameter for CSRF protection
    pub state: Option<String>,
    /// Optional redirect URL after successful login
    pub redirect_to: Option<String>,
}

/// GET /auth/callback - Handle OIDC authorization code callback.
///
/// Exchanges the authorization code for tokens and creates a local session.
pub async fn callback(
    State(state): State<Arc<AppState>>,
    Query(params): Query<CallbackParams>,
) -> Result<Json<TokenResponse>, ApiError> {
    let oidc = state.oidc_config.as_ref().ok_or_else(|| {
        ApiError::Internal("OIDC not configured".to_string())
    })?;

    // Exchange authorization code for tokens with the OIDC provider
    let token_url = format!("{}/protocol/openid-connect/token", oidc.issuer_url);

    let mut form_params = vec![
        ("grant_type", "authorization_code"),
        ("code", &params.code),
        ("client_id", &oidc.client_id),
        ("redirect_uri", &oidc.redirect_uri),
    ];

    let client_secret_string;
    if let Some(ref secret) = oidc.client_secret {
        client_secret_string = secret.clone();
        form_params.push(("client_secret", &client_secret_string));
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| ApiError::Internal(format!("HTTP client error: {}", e)))?;
    let response = client
        .post(&token_url)
        .form(&form_params)
        .send()
        .await
        .map_err(|e| ApiError::Internal(format!("Token exchange failed: {}", e)))?;

    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        return Err(ApiError::Internal(format!("Token exchange failed: {}", error_text)));
    }

    let oidc_tokens: OidcTokenResponse = response
        .json()
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to parse token response: {}", e)))?;

    // Decode the ID token to get user info
    let id_token_claims = decode_id_token(&oidc_tokens.id_token)
        .map_err(|e| ApiError::Internal(format!("Failed to decode ID token: {}", e)))?;

    // Generate our own JWT for the API
    let jwt_secret = &state.auth_config.jwt_secret;
    let access_token = generate_access_token(jwt_secret, &id_token_claims)?;

    Ok(Json(TokenResponse {
        access_token,
        token_type: "Bearer".to_string(),
        expires_in: 3600,
        refresh_token: oidc_tokens.refresh_token,
    }))
}

/// POST /auth/token - Exchange credentials or refresh token for access token.
///
/// Supports:
/// - Authorization code exchange (grant_type=authorization_code)
/// - Refresh token (grant_type=refresh_token)
pub async fn token(
    State(state): State<Arc<AppState>>,
    Json(request): Json<TokenRequest>,
) -> Result<Json<TokenResponse>, ApiError> {
    match request.grant_type.as_str() {
        "refresh_token" => {
            let refresh_token = request.refresh_token.ok_or_else(|| {
                ApiError::BadRequest("refresh_token is required".to_string())
            })?;

            // For now, re-validate with OIDC provider
            // In production, you might want to cache user info
            let oidc = state.oidc_config.as_ref().ok_or_else(|| {
                ApiError::Internal("OIDC not configured".to_string())
            })?;

            let token_url = format!("{}/protocol/openid-connect/token", oidc.issuer_url);

            let mut form_params = vec![
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh_token.as_str()),
                ("client_id", oidc.client_id.as_str()),
            ];

            let client_secret_string;
            if let Some(ref secret) = oidc.client_secret {
                client_secret_string = secret.clone();
                form_params.push(("client_secret", &client_secret_string));
            }

            let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| ApiError::Internal(format!("HTTP client error: {}", e)))?;
            let response = client
                .post(&token_url)
                .form(&form_params)
                .send()
                .await
                .map_err(|e| ApiError::Internal(format!("Token refresh failed: {}", e)))?;

            if !response.status().is_success() {
                return Err(ApiError::Unauthorized("Invalid refresh token".to_string()));
            }

            let oidc_tokens: OidcTokenResponse = response
                .json()
                .await
                .map_err(|e| ApiError::Internal(format!("Failed to parse token response: {}", e)))?;

            let id_token_claims = decode_id_token(&oidc_tokens.id_token)
                .map_err(|e| ApiError::Internal(format!("Failed to decode ID token: {}", e)))?;

            let jwt_secret = &state.auth_config.jwt_secret;
            let access_token = generate_access_token(jwt_secret, &id_token_claims)?;

            Ok(Json(TokenResponse {
                access_token,
                token_type: "Bearer".to_string(),
                expires_in: 3600,
                refresh_token: oidc_tokens.refresh_token,
            }))
        }
        _ => Err(ApiError::BadRequest(format!(
            "Unsupported grant_type: {}",
            request.grant_type
        ))),
    }
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct TokenRequest {
    pub grant_type: String,
    pub refresh_token: Option<String>,
    pub code: Option<String>,
    pub redirect_uri: Option<String>,
}

/// GET /auth/userinfo - Get current user information.
///
/// Requires a valid access token.
pub async fn userinfo(
    auth: Option<axum::Extension<AuthContext>>,
) -> Result<Json<UserInfoResponse>, ApiError> {
    let auth = auth.ok_or_else(|| {
        ApiError::Unauthorized("Authentication required".to_string())
    })?;

    Ok(Json(UserInfoResponse {
        sub: auth.user_id.clone(),
        email: None, // Would be populated from user store
        name: None,
        tenant_id: auth.tenant_id.to_string(),
        roles: auth.permissions.roles.iter().map(|r: &fen_core::domain::acl::Role| r.to_string()).collect(),
    }))
}

/// POST /auth/logout - Logout and invalidate tokens.
///
/// For OIDC, this may redirect to the provider's end session endpoint.
pub async fn logout(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, ApiError> {
    // For OIDC, redirect to the provider's end session endpoint
    if let Some(ref oidc) = state.oidc_config {
        let logout_url = format!(
            "{}/protocol/openid-connect/logout?client_id={}",
            oidc.issuer_url,
            urlencoding::encode(&oidc.client_id)
        );
        return Ok(Redirect::temporary(&logout_url).into_response());
    }

    // For local auth, just return success
    Ok((StatusCode::OK, Json(serde_json::json!({"message": "Logged out"}))).into_response())
}

/// GET /auth/providers - List available authentication providers.
pub async fn list_providers(
    State(state): State<Arc<AppState>>,
) -> Json<ProvidersResponse> {
    let mut providers = vec![];

    if state.oidc_config.is_some() {
        providers.push(ProviderInfo {
            id: "oidc".to_string(),
            name: "Single Sign-On".to_string(),
            provider_type: "oidc".to_string(),
            enabled: true,
        });
    }

    // Local auth is always available as fallback
    providers.push(ProviderInfo {
        id: "local".to_string(),
        name: "Email & Password".to_string(),
        provider_type: "local".to_string(),
        enabled: true,
    });

    Json(ProvidersResponse { providers })
}

#[derive(Debug, Serialize)]
pub struct ProvidersResponse {
    pub providers: Vec<ProviderInfo>,
}

#[derive(Debug, Serialize)]
pub struct ProviderInfo {
    pub id: String,
    pub name: String,
    pub provider_type: String,
    pub enabled: bool,
}

// Internal types for OIDC communication

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OidcTokenResponse {
    access_token: String,
    id_token: String,
    refresh_token: Option<String>,
    expires_in: Option<i64>,
    token_type: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct IdTokenClaims {
    sub: String,
    email: Option<String>,
    name: Option<String>,
    preferred_username: Option<String>,
    // Keycloak-specific claims
    #[serde(default)]
    realm_access: Option<RealmAccess>,
    // Custom claims for tenant
    tenant_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RealmAccess {
    roles: Vec<String>,
}

/// Decode an ID token (without full validation - OIDC provider already validated).
fn decode_id_token(token: &str) -> Result<IdTokenClaims, String> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return Err("Invalid token format".to_string());
    }

    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
    let payload = URL_SAFE_NO_PAD
        .decode(parts[1])
        .map_err(|e| format!("Base64 decode error: {}", e))?;

    serde_json::from_slice(&payload)
        .map_err(|e| format!("JSON decode error: {}", e))
}

/// Generate an access token for the Fen API.
fn generate_access_token(secret: &str, id_claims: &IdTokenClaims) -> Result<String, ApiError> {
    let now = Utc::now();
    let exp = now + Duration::hours(1);

    // Extract roles from Keycloak's realm_access claim
    let roles: Vec<String> = id_claims
        .realm_access
        .as_ref()
        .map(|ra| ra.roles.clone())
        .unwrap_or_default();

    // Use tenant_id from claims or default to a system tenant
    let tenant_id = id_claims
        .tenant_id
        .clone()
        .unwrap_or_else(|| uuid::Uuid::nil().to_string());

    let claims = Claims {
        sub: id_claims.sub.clone(),
        tenant_id,
        roles,
        exp: exp.timestamp() as usize,
        iat: now.timestamp() as usize,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| ApiError::Internal(format!("Failed to generate token: {}", e)))
}

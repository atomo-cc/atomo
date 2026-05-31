//! OAuth2/OIDC provider integration for enterprise SSO

use anyhow::Result;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Redirect,
    Json,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// OAuth2 provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthProvider {
    pub name: String,
    pub client_id: String,
    pub client_secret: String,
    pub auth_url: String,
    pub token_url: String,
    pub userinfo_url: String,
    pub redirect_uri: String,
    pub scopes: Vec<String>,
}

impl OAuthProvider {
    /// Load from environment variables (e.g., OAUTH_GOOGLE_CLIENT_ID)
    pub fn from_env(name: &str) -> Option<Self> {
        let prefix = format!("OAUTH_{}", name.to_uppercase());
        Some(Self {
            name: name.to_string(),
            client_id: std::env::var(format!("{}_CLIENT_ID", prefix)).ok()?,
            client_secret: std::env::var(format!("{}_CLIENT_SECRET", prefix)).ok()?,
            auth_url: std::env::var(format!("{}_AUTH_URL", prefix)).ok()?,
            token_url: std::env::var(format!("{}_TOKEN_URL", prefix)).ok()?,
            userinfo_url: std::env::var(format!("{}_USERINFO_URL", prefix)).ok()?,
            redirect_uri: std::env::var(format!("{}_REDIRECT_URI", prefix))
                .unwrap_or_else(|_| format!("http://localhost:3000/auth/callback/{}", name)),
            scopes: vec!["openid".into(), "email".into(), "profile".into()],
        })
    }

    /// Build the authorization URL
    pub fn authorize_url(&self, state: &str) -> String {
        format!(
            "{}?client_id={}&redirect_uri={}&response_type=code&scope={}&state={}",
            self.auth_url,
            urlencoding::encode(&self.client_id),
            urlencoding::encode(&self.redirect_uri),
            urlencoding::encode(&self.scopes.join(" ")),
            urlencoding::encode(state),
        )
    }
}

/// OAuth2 token response
#[derive(Debug, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: Option<u64>,
    pub id_token: Option<String>,
}

/// User info from OIDC provider
#[derive(Debug, Deserialize, Serialize)]
pub struct OAuthUserInfo {
    pub sub: String,
    pub email: Option<String>,
    pub name: Option<String>,
    pub picture: Option<String>,
}

/// OAuth state manager
#[derive(Clone)]
pub struct OAuthManager {
    providers: HashMap<String, OAuthProvider>,
    pool: Option<sqlx::PgPool>,
    jwt_secret: Option<String>,
}

impl Default for OAuthManager {
    fn default() -> Self {
        Self::new()
    }
}

impl OAuthManager {
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
            pool: None,
            jwt_secret: None,
        }
    }

    /// Load all configured providers from environment
    pub fn from_env() -> Self {
        let mut mgr = Self::new();
        mgr.jwt_secret = std::env::var("JWT_SECRET").ok();
        for name in ["google", "github", "microsoft", "okta"] {
            if let Some(provider) = OAuthProvider::from_env(name) {
                mgr.providers.insert(name.to_string(), provider);
            }
        }
        mgr
    }

    pub fn with_pool(mut self, pool: sqlx::PgPool) -> Self {
        self.pool = Some(pool);
        self
    }

    /// Find existing user by email or create a new one with viewer role
    pub async fn find_or_create_user(
        &self,
        info: &OAuthUserInfo,
    ) -> Result<(String, String, String)> {
        let pool = self
            .pool
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No database pool configured"))?;
        let email = info
            .email
            .clone()
            .unwrap_or_else(|| format!("{}@oauth.local", info.sub));
        let existing = sqlx::query_as::<_, (String, String, String)>(
            "SELECT id, email, role FROM users WHERE email = $1",
        )
        .bind(&email)
        .fetch_optional(pool)
        .await?;
        if let Some(row) = existing {
            return Ok(row);
        }
        let id = atomo_core::types::EntityId::new().to_string();
        let name = info.name.clone().unwrap_or_default();
        sqlx::query(
            "INSERT INTO users (id, email, password_hash, first_name, last_name, role, is_active, created_at, updated_at) VALUES ($1, $2, '', $3, '', 'viewer', true, NOW(), NOW())"
        ).bind(&id).bind(&email).bind(&name).execute(pool).await?;
        Ok((id, email, "viewer".to_string()))
    }

    /// Issue a JWT access token for the given user
    pub fn issue_jwt(&self, user_id: &str, email: &str, role: &str) -> Result<String> {
        use jsonwebtoken::{encode, EncodingKey, Header};
        let secret = self
            .jwt_secret
            .clone()
            .unwrap_or_else(|| "dev-insecure-secret".to_string());
        let now = chrono::Utc::now();
        let claims = serde_json::json!({
            "sub": user_id,
            "email": email,
            "role": role,
            "exp": (now + chrono::Duration::hours(24)).timestamp(),
            "iat": now.timestamp(),
            "jti": uuid::Uuid::new_v4().to_string(),
        });
        Ok(encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(secret.as_ref()),
        )?)
    }

    pub fn get_provider(&self, name: &str) -> Option<&OAuthProvider> {
        self.providers.get(name)
    }

    pub fn available_providers(&self) -> Vec<&str> {
        self.providers.keys().map(|s| s.as_str()).collect()
    }

    /// Exchange authorization code for tokens
    pub async fn exchange_code(&self, provider_name: &str, code: &str) -> Result<TokenResponse> {
        let provider = self
            .providers
            .get(provider_name)
            .ok_or_else(|| anyhow::anyhow!("Provider '{}' not configured", provider_name))?;

        let client = reqwest::Client::new();
        let resp = client
            .post(&provider.token_url)
            .form(&[
                ("grant_type", "authorization_code"),
                ("code", code),
                ("client_id", &provider.client_id),
                ("client_secret", &provider.client_secret),
                ("redirect_uri", &provider.redirect_uri),
            ])
            .send()
            .await?
            .json::<TokenResponse>()
            .await?;
        Ok(resp)
    }

    /// Fetch user info using access token
    pub async fn get_user_info(
        &self,
        provider_name: &str,
        access_token: &str,
    ) -> Result<OAuthUserInfo> {
        let provider = self
            .providers
            .get(provider_name)
            .ok_or_else(|| anyhow::anyhow!("Provider '{}' not configured", provider_name))?;

        let client = reqwest::Client::new();
        let resp = client
            .get(&provider.userinfo_url)
            .bearer_auth(access_token)
            .send()
            .await?
            .json::<OAuthUserInfo>()
            .await?;
        Ok(resp)
    }
}

// ============================================================================
// HTTP Handlers
// ============================================================================

#[derive(Deserialize)]
pub struct AuthorizeParams {
    pub provider: String,
}

#[derive(Deserialize)]
pub struct CallbackParams {
    pub code: String,
    pub state: Option<String>,
}

/// GET /auth/oauth/authorize?provider=google
pub async fn oauth_authorize(
    State(oauth): State<OAuthManager>,
    Query(params): Query<AuthorizeParams>,
) -> Result<Redirect, StatusCode> {
    let provider = oauth
        .get_provider(&params.provider)
        .ok_or(StatusCode::NOT_FOUND)?;
    let state = uuid::Uuid::new_v4().to_string();
    let url = provider.authorize_url(&state);
    Ok(Redirect::temporary(&url))
}

/// GET /auth/oauth/callback/:provider?code=...&state=...
pub async fn oauth_callback(
    State(oauth): State<OAuthManager>,
    axum::extract::Path(provider_name): axum::extract::Path<String>,
    Query(params): Query<CallbackParams>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let tokens = oauth
        .exchange_code(&provider_name, &params.code)
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;
    let user_info = oauth
        .get_user_info(&provider_name, &tokens.access_token)
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;
    let (user_id, email, role) = oauth
        .find_or_create_user(&user_info)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let jwt = oauth
        .issue_jwt(&user_id, &email, &role)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(serde_json::json!({
        "access_token": jwt,
        "user": { "id": user_id, "email": email, "role": role }
    })))
}

/// GET /auth/oauth/providers - list available providers
pub async fn oauth_providers(State(oauth): State<OAuthManager>) -> Json<Vec<String>> {
    Json(
        oauth
            .available_providers()
            .into_iter()
            .map(String::from)
            .collect(),
    )
}


#[cfg(test)]
mod tests {
    use super::*;

    // D3 (infra-free slice): the authorization URL must carry the security-relevant params —
    // client_id, redirect_uri, response_type=code, scope, and the CSRF state. A full token
    // round-trip needs a mock IdP (deferred); this proves the request we send is well-formed.
    #[test]
    fn authorize_url_contains_required_params() {
        let p = OAuthProvider {
            name: "google".into(),
            client_id: "cid-123".into(),
            client_secret: "secret".into(),
            auth_url: "https://accounts.example.com/o/oauth2/auth".into(),
            token_url: "https://example.com/token".into(),
            userinfo_url: "https://example.com/userinfo".into(),
            redirect_uri: "http://localhost:3000/auth/callback/google".into(),
            scopes: vec!["openid".into(), "email".into()],
        };
        let url = p.authorize_url("csrf-state-xyz");
        assert!(url.starts_with("https://accounts.example.com/o/oauth2/auth?"), "base auth_url: {}", url);
        assert!(url.contains("client_id=cid-123"), "client_id missing: {}", url);
        assert!(url.contains("response_type=code"), "response_type missing: {}", url);
        assert!(url.contains("state=csrf-state-xyz"), "CSRF state missing: {}", url);
        assert!(url.contains("redirect_uri=http%3A%2F%2Flocalhost%3A3000%2Fauth%2Fcallback%2Fgoogle"), "redirect not encoded: {}", url);
        assert!(url.contains("scope=openid%20email"), "scopes not encoded: {}", url);
    }
}

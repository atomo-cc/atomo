//! OAuth2/OIDC provider integration for enterprise SSO

use anyhow::Result;
use axum::{extract::{Query, State}, http::StatusCode, response::Redirect, Json};
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
}

impl OAuthManager {
    pub fn new() -> Self {
        Self { providers: HashMap::new() }
    }

    /// Load all configured providers from environment
    pub fn from_env() -> Self {
        let mut mgr = Self::new();
        for name in ["google", "github", "microsoft", "okta"] {
            if let Some(provider) = OAuthProvider::from_env(name) {
                mgr.providers.insert(name.to_string(), provider);
            }
        }
        mgr
    }

    pub fn get_provider(&self, name: &str) -> Option<&OAuthProvider> {
        self.providers.get(name)
    }

    pub fn available_providers(&self) -> Vec<&str> {
        self.providers.keys().map(|s| s.as_str()).collect()
    }

    /// Exchange authorization code for tokens
    pub async fn exchange_code(&self, provider_name: &str, code: &str) -> Result<TokenResponse> {
        let provider = self.providers.get(provider_name)
            .ok_or_else(|| anyhow::anyhow!("Provider '{}' not configured", provider_name))?;

        let client = reqwest::Client::new();
        let resp = client.post(&provider.token_url)
            .form(&[
                ("grant_type", "authorization_code"),
                ("code", code),
                ("client_id", &provider.client_id),
                ("client_secret", &provider.client_secret),
                ("redirect_uri", &provider.redirect_uri),
            ])
            .send().await?
            .json::<TokenResponse>().await?;
        Ok(resp)
    }

    /// Fetch user info using access token
    pub async fn get_user_info(&self, provider_name: &str, access_token: &str) -> Result<OAuthUserInfo> {
        let provider = self.providers.get(provider_name)
            .ok_or_else(|| anyhow::anyhow!("Provider '{}' not configured", provider_name))?;

        let client = reqwest::Client::new();
        let resp = client.get(&provider.userinfo_url)
            .bearer_auth(access_token)
            .send().await?
            .json::<OAuthUserInfo>().await?;
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
    let provider = oauth.get_provider(&params.provider)
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
    let tokens = oauth.exchange_code(&provider_name, &params.code).await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;
    let user_info = oauth.get_user_info(&provider_name, &tokens.access_token).await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;
    // In production: find-or-create user, issue JWT, return session
    Ok(Json(serde_json::json!({
        "provider": provider_name,
        "user": user_info,
        "access_token": tokens.access_token,
    })))
}

/// GET /auth/oauth/providers - list available providers
pub async fn oauth_providers(
    State(oauth): State<OAuthManager>,
) -> Json<Vec<String>> {
    Json(oauth.available_providers().into_iter().map(String::from).collect())
}

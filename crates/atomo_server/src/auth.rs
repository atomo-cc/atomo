//! Authentication and Authorization HTTP Implementation
//!
//! This module provides HTTP-specific implementations of the authentication
//! and authorization interfaces defined in atomo_core.

use crate::platform_models::{PlatformUser, UserRole, UserSession};
use anyhow::Result;
use argon2::{
    password_hash::{rand_core::OsRng, SaltString},
    Argon2, PasswordHasher, PasswordVerifier,
};
use async_trait::async_trait;
use atomo_core::{AuthCredentials, AuthProvider, AuthorizationService, EntityId};
use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use chrono::{Duration, Utc};
use jsonwebtoken::{
    decode, encode, Algorithm, DecodingKey, EncodingKey, Header, TokenData, Validation,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

/// JWT Claims structure
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,   // User ID
    pub email: String, // User email
    pub role: String,  // User role
    pub exp: i64,      // Expiration time
    pub iat: i64,      // Issued at
    pub jti: String,   // JWT ID (session ID)
}

/// Stateless claims for the realtime relay. Unlike [`Claims`], there is no
/// `jti`/DB session — so `atomo-realtime-server` can verify these with signature
/// + expiry alone (no database). `sid` is the matchmaker's session assignment.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RealtimeClaims {
    pub sub: String, // User ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sid: Option<String>, // Assigned session (optional)
    pub exp: i64,
    pub iat: i64,
}

/// User context extracted from JWT - HTTP layer specific
#[derive(Debug, Clone)]
pub struct AuthUser {
    pub id: String,
    pub email: String,
    pub role: UserRole,
    pub session_id: String,
    /// The user's tenant (from users.tenant_id), if any — used to validate the x-tenant-id header.
    pub tenant_id: Option<String>,
}

/// HTTP Authentication service implementation
#[derive(Clone)]
pub struct HttpAuthService {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    db_pool: PgPool,
}

/// Error type for HttpAuthService
pub type AuthError = anyhow::Error;

impl HttpAuthService {
    fn validate_password_policy(&self, password: &str) -> Result<(), anyhow::Error> {
        let min_len: usize = std::env::var("PASSWORD_MIN_LENGTH")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(8);
        let require_complexity = std::env::var("PASSWORD_REQUIRE_COMPLEXITY")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(true);
        if password.len() < min_len {
            return Err(anyhow::anyhow!(format!(
                "Password must be at least {} characters",
                min_len
            )));
        }
        if require_complexity {
            let has_letter = password.chars().any(|c| c.is_ascii_alphabetic());
            let has_number = password.chars().any(|c| c.is_ascii_digit());
            if !(has_letter && has_number) {
                return Err(anyhow::anyhow!("Password must contain letters and numbers"));
            }
        }
        Ok(())
    }
    pub fn new(secret: &str, db_pool: PgPool) -> Self {
        Self {
            encoding_key: EncodingKey::from_secret(secret.as_ref()),
            decoding_key: DecodingKey::from_secret(secret.as_ref()),
            db_pool,
        }
    }

    /// Get the database connection pool
    pub fn db_pool(&self) -> &PgPool {
        &self.db_pool
    }

    /// Generate access token and refresh token for a user (rotates/creates session)
    pub async fn issue_tokens(
        &self,
        user_id: &str,
        email: &str,
        role: &str,
    ) -> Result<(String, String)> {
        // Create a real session via DB and use its ID as JWT jti
        let entity_id = EntityId::from_string(user_id)?;
        let session = self.create_session(&entity_id).await?;

        let now = Utc::now();
        let exp = now + Duration::hours(24); // Token expires in 24 hours

        let claims = Claims {
            sub: user_id.to_string(),
            email: email.to_string(),
            role: role.to_string(),
            exp: exp.timestamp(),
            iat: now.timestamp(),
            jti: session.id.to_string(),
        };

        let token = encode(&Header::default(), &claims, &self.encoding_key)?;
        Ok((token, session.token))
    }

    /// Mint a short-lived, **stateless** token for the realtime relay
    /// (`atomo-realtime-server`). Signed with the same secret, but with no DB
    /// session, so the relay verifies it without a database. `session` becomes
    /// the `sid` claim (the matchmaker's assignment).
    pub fn mint_realtime_token(
        &self,
        user_id: &str,
        session: Option<&str>,
        ttl_seconds: i64,
    ) -> Result<String> {
        let now = Utc::now();
        let claims = RealtimeClaims {
            sub: user_id.to_string(),
            sid: session.map(|s| s.to_string()),
            exp: (now + Duration::seconds(ttl_seconds)).timestamp(),
            iat: now.timestamp(),
        };
        Ok(encode(&Header::default(), &claims, &self.encoding_key)?)
    }

    /// Verify JWT token and extract user information
    pub async fn verify_token(&self, token: &str) -> Result<AuthUser> {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.set_required_spec_claims(&["sub", "exp", "iat", "jti"]);

        let token_data: TokenData<Claims> = decode(token, &self.decoding_key, &validation)?;
        let claims = token_data.claims;

        // Verify session using core interface
        if let Some(user) = self.validate_session(token).await? {
            // Look up the user's tenant binding (used to validate the x-tenant-id header).
            let tenant_id = sqlx::query_scalar::<_, Option<String>>(
                "SELECT tenant_id FROM users WHERE id = $1",
            )
            .bind(user.id.to_string())
            .fetch_optional(&self.db_pool)
            .await?
            .flatten();
            Ok(AuthUser {
                id: user.id.to_string(),
                email: user.email,
                role: user.role,
                session_id: claims.jti,
                tenant_id,
            })
        } else {
            Err(anyhow::anyhow!("Session expired or invalid"))
        }
    }

    /// Refresh access token using a refresh token (rotates refresh token and extends session)
    pub async fn refresh_access_token(
        &self,
        refresh_token: &str,
    ) -> Result<(String, String, PlatformUser)> {
        // Validate session by refresh token
        let session = sqlx::query_as::<_, (String, String, chrono::DateTime<chrono::Utc>)>(
            "SELECT id, user_id, expires_at FROM sessions WHERE token = $1 AND is_revoked = false",
        )
        .bind(refresh_token)
        .fetch_optional(&self.db_pool)
        .await?;

        let (session_id, user_id, _expires_at) = match session {
            Some(s) => s,
            None => return Err(anyhow::anyhow!("Invalid refresh token")),
        };

        // Check session expiry
        let still_valid = sqlx::query_as::<_, (bool,)>(
            "SELECT expires_at > NOW() FROM sessions WHERE token = $1",
        )
        .bind(refresh_token)
        .fetch_one(&self.db_pool)
        .await?;
        if !still_valid.0 {
            return Err(anyhow::anyhow!("Refresh token expired"));
        }

        // Fetch user
        let user_row = sqlx::query_as::<_, (String, String, String, String, String, bool, Option<chrono::DateTime<chrono::Utc>>, chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>)>(
            "SELECT id, email, first_name, last_name, role, is_active, last_login_at, created_at, updated_at FROM users WHERE id = $1"
        )
        .bind(&user_id)
        .fetch_optional(&self.db_pool)
        .await?;

        let (
            id,
            email,
            first_name,
            last_name,
            role,
            is_active,
            last_login_at,
            created_at,
            updated_at,
        ) = user_row.ok_or_else(|| anyhow::anyhow!("User not found"))?;

        let platform_user = PlatformUser {
            id: EntityId::from_string(&id)?,
            email: email.clone(),
            password_hash: "".to_string(),
            first_name: Some(first_name),
            last_name: Some(last_name),
            role: UserRole::from(role.clone()),
            is_active,
            last_login_at,
            created_at,
            updated_at,
        };

        // Create new access token with same session id
        let now = Utc::now();
        let exp = now + Duration::hours(24);
        let claims = Claims {
            sub: id.clone(),
            email: email.clone(),
            role: role.clone(),
            exp: exp.timestamp(),
            iat: now.timestamp(),
            jti: session_id.clone(),
        };
        let access_token = encode(&Header::default(), &claims, &self.encoding_key)?;

        // Rotate refresh token and extend expiry
        let new_refresh = Uuid::new_v4().to_string();
        let new_expires = now + Duration::hours(24);
        sqlx::query("UPDATE sessions SET token = $1, expires_at = $2 WHERE id = $3")
            .bind(&new_refresh)
            .bind(new_expires)
            .bind(&session_id)
            .execute(&self.db_pool)
            .await?;

        Ok((access_token, new_refresh, platform_user))
    }
}

// Implement AuthProvider trait for HttpAuthService
#[async_trait]
impl AuthProvider<PlatformUser, UserSession> for HttpAuthService {
    type Error = anyhow::Error;

    async fn authenticate(
        &self,
        credentials: &AuthCredentials,
    ) -> Result<Option<PlatformUser>, Self::Error> {
        let user = sqlx::query_as::<_, (String, String, String, String, String, String, bool, Option<chrono::DateTime<chrono::Utc>>, chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>)>(
            "SELECT id, email, password_hash, first_name, last_name, role, is_active, last_login_at, created_at, updated_at FROM users 
             WHERE email = $1 AND is_active = true"
        )
        .bind(&credentials.email)
        .fetch_optional(&self.db_pool)
        .await?;

        if let Some((
            user_id,
            user_email,
            password_hash,
            first_name,
            last_name,
            role,
            is_active,
            last_login_at,
            created_at,
            updated_at,
        )) = user
        {
            if self.verify_password(&credentials.password, &password_hash)? {
                // Update last login
                sqlx::query("UPDATE users SET last_login_at = NOW() WHERE id = $1")
                    .bind(&user_id)
                    .execute(&self.db_pool)
                    .await?;

                let user = PlatformUser {
                    id: EntityId::from_string(&user_id)?,
                    email: user_email,
                    password_hash,
                    first_name: Some(first_name),
                    last_name: Some(last_name),
                    role: UserRole::from(role),
                    is_active,
                    last_login_at,
                    created_at,
                    updated_at,
                };

                return Ok(Some(user));
            }
        }

        Ok(None)
    }

    async fn authorize(
        &self,
        user: &PlatformUser,
        _resource: &str,
        action: &str,
    ) -> Result<bool, Self::Error> {
        // For now, implement basic role-based authorization
        let access_pattern = match action {
            "read" => "admin|manager|sales|support|viewer",
            "write" => "admin|manager|sales",
            "delete" => "admin|manager",
            "admin" => "admin",
            _ => return Ok(false),
        };

        Ok(user.role.has_permission(access_pattern))
    }

    async fn create_session(&self, user_id: &EntityId) -> Result<UserSession, Self::Error> {
        let session_id = EntityId::new();
        let token = Uuid::new_v4().to_string();
        let now = Utc::now();
        let expires_at = now + Duration::hours(24);

        sqlx::query(
            "INSERT INTO sessions (id, user_id, token, expires_at, created_at, is_revoked) 
             VALUES ($1, $2, $3, $4, $5, false)",
        )
        .bind(session_id.to_string())
        .bind(user_id.to_string())
        .bind(&token)
        .bind(expires_at)
        .bind(now)
        .execute(&self.db_pool)
        .await?;

        Ok(UserSession {
            id: session_id,
            user_id: *user_id,
            token,
            expires_at,
            ip_address: None,
            user_agent: None,
            created_at: now,
        })
    }

    async fn validate_session(
        &self,
        session_token: &str,
    ) -> Result<Option<PlatformUser>, Self::Error> {
        // First try to decode JWT to get session info
        let mut validation = Validation::new(Algorithm::HS256);
        validation.set_required_spec_claims(&["sub", "exp", "iat", "jti"]);

        if let Ok(token_data) = decode::<Claims>(session_token, &self.decoding_key, &validation) {
            let claims = token_data.claims;

            // Check if session exists and is active
            let session =
                sqlx::query_as::<_, (String, String, chrono::DateTime<chrono::Utc>, bool)>(
                    "SELECT id, user_id, expires_at, is_revoked FROM sessions 
                 WHERE id = $1 AND expires_at > NOW() AND is_revoked = false",
                )
                .bind(&claims.jti)
                .fetch_optional(&self.db_pool)
                .await?;

            if let Some((_, user_id_str, _, _)) = session {
                let user_id = EntityId::from_string(&user_id_str)?;

                // Get user directly instead of calling self.get_user
                let user = sqlx::query_as::<_, (String, String, String, String, String, bool, Option<chrono::DateTime<chrono::Utc>>, chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>)>(
                    "SELECT id, email, first_name, last_name, role, is_active, last_login_at, created_at, updated_at FROM users WHERE id = $1"
                )
                .bind(user_id.to_string())
                .fetch_optional(&self.db_pool)
                .await?;

                if let Some((
                    id,
                    email,
                    first_name,
                    last_name,
                    role,
                    is_active,
                    last_login_at,
                    created_at,
                    updated_at,
                )) = user
                {
                    return Ok(Some(PlatformUser {
                        id: EntityId::from_string(&id)?,
                        email,
                        password_hash: "".to_string(), // Don't return password hash
                        first_name: Some(first_name),
                        last_name: Some(last_name),
                        role: UserRole::from(role),
                        is_active,
                        last_login_at,
                        created_at,
                        updated_at,
                    }));
                }
            }
        }

        Ok(None)
    }
}

// Additional service methods for HttpAuthService
impl HttpAuthService {
    pub async fn revoke_session(&self, session_id: &EntityId) -> Result<(), anyhow::Error> {
        sqlx::query("UPDATE sessions SET is_revoked = true WHERE id = $1")
            .bind(session_id.to_string())
            .execute(&self.db_pool)
            .await?;

        Ok(())
    }

    /// Hash a password using argon2id
    pub fn hash_password(&self, password: &str) -> Result<String, anyhow::Error> {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        let hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| anyhow::anyhow!("Failed to hash password: {}", e))?;
        Ok(hash.to_string())
    }

    /// Verify a password against its hash (supports argon2 and bcrypt fallback)
    pub fn verify_password(&self, password: &str, hash: &str) -> Result<bool, anyhow::Error> {
        if hash.starts_with("$argon2") {
            let parsed = argon2::password_hash::PasswordHash::new(hash)
                .map_err(|e| anyhow::anyhow!("Invalid hash: {}", e))?;
            Ok(Argon2::default()
                .verify_password(password.as_bytes(), &parsed)
                .is_ok())
        } else {
            Ok(bcrypt::verify(password, hash).unwrap_or(false))
        }
    }
}

// Implement AuthorizationService trait for HttpAuthService
#[async_trait]
impl AuthorizationService<PlatformUser> for HttpAuthService {
    type Error = anyhow::Error;

    async fn get_user(&self, user_id: &EntityId) -> Result<Option<PlatformUser>, Self::Error> {
        let user = sqlx::query_as::<_, (String, String, String, String, String, bool, Option<chrono::DateTime<chrono::Utc>>, chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>)>(
            "SELECT id, email, first_name, last_name, role, is_active, last_login_at, created_at, updated_at FROM users WHERE id = $1"
        )
        .bind(user_id.to_string())
        .fetch_optional(&self.db_pool)
        .await?;

        if let Some((
            id,
            email,
            first_name,
            last_name,
            role,
            is_active,
            last_login_at,
            created_at,
            updated_at,
        )) = user
        {
            Ok(Some(PlatformUser {
                id: EntityId::from_string(&id)?,
                email,
                password_hash: "".to_string(), // Don't return password hash in get_user
                first_name: Some(first_name),
                last_name: Some(last_name),
                role: UserRole::from(role),
                is_active,
                last_login_at,
                created_at,
                updated_at,
            }))
        } else {
            Ok(None)
        }
    }

    async fn get_user_by_email(&self, email: &str) -> Result<Option<PlatformUser>, Self::Error> {
        let user = sqlx::query_as::<_, (String, String, String, String, String, bool, Option<chrono::DateTime<chrono::Utc>>, chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>)>(
            "SELECT id, email, first_name, last_name, role, is_active, last_login_at, created_at, updated_at FROM users WHERE email = $1"
        )
        .bind(email)
        .fetch_optional(&self.db_pool)
        .await?;

        if let Some((
            id,
            email,
            first_name,
            last_name,
            role,
            is_active,
            last_login_at,
            created_at,
            updated_at,
        )) = user
        {
            Ok(Some(PlatformUser {
                id: EntityId::from_string(&id)?,
                email,
                password_hash: "".to_string(), // Don't return password hash
                first_name: Some(first_name),
                last_name: Some(last_name),
                role: UserRole::from(role),
                is_active,
                last_login_at,
                created_at,
                updated_at,
            }))
        } else {
            Ok(None)
        }
    }

    async fn create_user(
        &self,
        email: &str,
        password: &str,
        first_name: &str,
        last_name: &str,
        _role: impl Send,
    ) -> Result<PlatformUser, Self::Error> {
        // Enforce password policy
        self.validate_password_policy(password)?;
        let user_id = EntityId::new();
        let now = Utc::now();
        let password_hash = self.hash_password(password)?;

        // Role handling: default to viewer if unspecified/invalid
        let role_str = "viewer";

        sqlx::query(
            "INSERT INTO users (id, email, password_hash, first_name, last_name, role, is_active, created_at, updated_at) 
             VALUES ($1, $2, $3, $4, $5, $6, true, $7, $8)"
        )
        .bind(user_id.to_string())
        .bind(email)
        .bind(&password_hash)
        .bind(first_name)
        .bind(last_name)
        .bind(role_str)
        .bind(now)
        .bind(now)
        .execute(&self.db_pool)
        .await?;

        Ok(PlatformUser {
            id: user_id,
            email: email.to_string(),
            password_hash,
            first_name: Some(first_name.to_string()),
            last_name: Some(last_name.to_string()),
            role: UserRole::Viewer,
            is_active: true,
            last_login_at: None,
            created_at: now,
            updated_at: now,
        })
    }

    async fn update_user(&self, user: &PlatformUser) -> Result<PlatformUser, Self::Error> {
        let now = Utc::now();

        sqlx::query(
            "UPDATE users SET email = $2, first_name = $3, last_name = $4, role = $5, is_active = $6, updated_at = $7 
             WHERE id = $1"
        )
        .bind(user.id.to_string())
        .bind(&user.email)
        .bind(&user.first_name)
        .bind(&user.last_name)
        .bind(user.role.to_string())
        .bind(user.is_active)
        .bind(now)
        .execute(&self.db_pool)
        .await?;

        let mut updated_user = user.clone();
        updated_user.updated_at = now;
        Ok(updated_user)
    }
}

/// Authentication middleware
pub async fn auth_middleware(
    State(auth_service): State<HttpAuthService>,
    mut request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Extract Authorization header
    let auth_header = request
        .headers()
        .get("Authorization")
        .and_then(|header| header.to_str().ok());

    if let Some(auth_header) = auth_header {
        if let Some(token) = auth_header.strip_prefix("Bearer ") {
            match auth_service.verify_token(token).await {
                Ok(user) => {
                    // Add user to request extensions
                    request.extensions_mut().insert(user);
                    Ok(next.run(request).await)
                }
                Err(_) => Err(StatusCode::UNAUTHORIZED),
            }
        } else {
            Err(StatusCode::UNAUTHORIZED)
        }
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

/// Optional authentication - doesn't fail if no token provided
pub async fn optional_auth_middleware(
    State(auth_service): State<HttpAuthService>,
    mut request: Request,
    next: Next,
) -> Response {
    // Extract Authorization header
    let auth_header = request
        .headers()
        .get("Authorization")
        .and_then(|header| header.to_str().ok());

    if let Some(auth_header) = auth_header {
        if let Some(token) = auth_header.strip_prefix("Bearer ") {
            if let Ok(user) = auth_service.verify_token(token).await {
                request.extensions_mut().insert(user);
            }
        }
    }

    next.run(request).await
}

// Note: AuthUser can be extracted from request extensions manually
// in handlers using: req.extensions().get::<AuthUser>()

/// Login request structure
#[derive(Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

/// Login response structure
#[derive(Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub refresh_token: Option<String>,
    pub user: UserInfo,
}

/// User information response
#[derive(Serialize)]
pub struct UserInfo {
    pub id: String,
    pub email: String,
    pub role: String,
    pub first_name: String,
    pub last_name: String,
    /// Tenant binding (for the client to send as X-Tenant-ID). None = unscoped.
    pub tenant_id: Option<String>,
}

/// How a self-registered user is assigned a tenant. The platform deliberately does **not** force a
/// one-user-one-tenant convention; the operator chooses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TenantProvisioning {
    /// No tenant binding (`tenant_id = NULL`). The neutral default.
    None,
    /// Each new user becomes its own tenant (`tenant_id = user id`).
    PerUser,
    /// All new users share one fixed tenant id.
    Fixed(String),
}

impl TenantProvisioning {
    /// Parse `ATOMO_SELF_REGISTRATION_TENANT`: empty/`none` (default) | `per-user`/`self` | any
    /// other non-empty value = a fixed tenant id shared by all registrants.
    pub fn from_env() -> Self {
        match std::env::var("ATOMO_SELF_REGISTRATION_TENANT")
            .unwrap_or_default()
            .trim()
        {
            "" | "none" => TenantProvisioning::None,
            "per-user" | "self" => TenantProvisioning::PerUser,
            other => TenantProvisioning::Fixed(other.to_string()),
        }
    }

    /// The `tenant_id` to store for a new user, or `None` for an unscoped user.
    pub fn tenant_id_for(&self, user_id: &str) -> Option<String> {
        match self {
            TenantProvisioning::None => None,
            TenantProvisioning::PerUser => Some(user_id.to_string()),
            TenantProvisioning::Fixed(tenant) => Some(tenant.clone()),
        }
    }
}

/// Self-registration policy. Default-off; the granted role and tenant provisioning are configurable
/// so the platform imposes no product-specific convention. The `enabled` flag is owned by
/// `ServerConfig` (`ATOMO_ENABLE_SELF_REGISTRATION`); the provisioning policy comes from
/// `ATOMO_SELF_REGISTRATION_ROLE` (default `viewer`) and `ATOMO_SELF_REGISTRATION_TENANT`.
#[derive(Debug, Clone)]
pub struct RegistrationConfig {
    pub enabled: bool,
    pub role: String,
    pub tenant: TenantProvisioning,
}

impl RegistrationConfig {
    /// Build from the enable flag plus the env-configured provisioning policy.
    pub fn new(enabled: bool) -> Self {
        let role = std::env::var("ATOMO_SELF_REGISTRATION_ROLE")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "viewer".to_string());
        Self {
            enabled,
            role,
            tenant: TenantProvisioning::from_env(),
        }
    }

    /// A disabled policy — the default for callers/tests that do not enable self-registration.
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            role: "viewer".to_string(),
            tenant: TenantProvisioning::None,
        }
    }
}

/// Auth handlers
pub mod handlers {
    use super::*;
    use atomo_core::AuthCredentials;
    use axum::{extract::State, Json};

    /// Login handler (issues access and refresh tokens)
    pub async fn login(
        State(auth_service): State<HttpAuthService>,
        Json(request): Json<LoginRequest>,
    ) -> Result<Json<LoginResponse>, StatusCode> {
        let credentials = AuthCredentials {
            email: request.email,
            password: request.password,
        };

        match auth_service.authenticate(&credentials).await {
            Ok(Some(user)) => {
                // Issue token pair
                let (token, refresh_token) = auth_service
                    .issue_tokens(&user.id.to_string(), &user.email, &user.role.to_string())
                    .await
                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

                // Fetch user details (incl. tenant binding for X-Tenant-ID).
                let user_details = sqlx::query_as::<_, (String, String, Option<String>)>(
                    "SELECT first_name, last_name, tenant_id FROM users WHERE id = $1",
                )
                .bind(user.id.to_string())
                .fetch_one(auth_service.db_pool())
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

                Ok(Json(LoginResponse {
                    token,
                    refresh_token: Some(refresh_token),
                    user: UserInfo {
                        id: user.id.to_string(),
                        email: user.email,
                        role: user.role.to_string(),
                        first_name: user_details.0,
                        last_name: user_details.1,
                        tenant_id: user_details.2,
                    },
                }))
            }
            Ok(None) => Err(StatusCode::UNAUTHORIZED),
            Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
        }
    }

    /// Self-serve registration request.
    #[derive(Deserialize)]
    #[allow(non_snake_case)]
    pub struct RegisterRequest {
        pub email: String,
        pub password: String,
        pub firstName: Option<String>,
        pub lastName: Option<String>,
    }

    /// Register handler — creates a login-capable user (argon2id-hashed password) and immediately
    /// issues tokens. The granted role and tenant binding follow the configured
    /// [`RegistrationConfig`] provisioning policy (no hardcoded role/tenant). Only mounted when
    /// self-registration is enabled (see `create_router`).
    pub async fn register(
        State(auth_service): State<HttpAuthService>,
        axum::Extension(registration): axum::Extension<super::RegistrationConfig>,
        Json(req): Json<RegisterRequest>,
    ) -> Result<Json<LoginResponse>, StatusCode> {
        let email = req.email.trim().to_lowercase();
        if email.is_empty() || !email.contains('@') || email.len() > 254 {
            return Err(StatusCode::BAD_REQUEST);
        }
        // Enforce the configured password policy (length/complexity).
        auth_service
            .validate_password_policy(&req.password)
            .map_err(|_| StatusCode::BAD_REQUEST)?;

        let pool = auth_service.db_pool();
        let id = atomo_core::types::EntityId::new().to_string();
        let hash = auth_service
            .hash_password(&req.password)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let first = req.firstName.unwrap_or_default();
        let last = req.lastName.unwrap_or_default();
        let role = registration.role.as_str();
        let tenant_id = registration.tenant.tenant_id_for(&id);

        // Race-safe: rely on the `users.email` UNIQUE constraint instead of a check-then-insert, so
        // two concurrent registrations for the same email cannot both succeed — a duplicate maps to
        // 409 rather than a second user (or a 500).
        let inserted = sqlx::query(
            "INSERT INTO users (id, email, password_hash, first_name, last_name, role, is_active, tenant_id)
             VALUES ($1, $2, $3, $4, $5, $6, true, $7)",
        )
        .bind(&id)
        .bind(&email)
        .bind(&hash)
        .bind(&first)
        .bind(&last)
        .bind(role)
        .bind(tenant_id.as_deref())
        .execute(pool)
        .await;
        if let Err(err) = inserted {
            if matches!(&err, sqlx::Error::Database(db) if db.is_unique_violation()) {
                return Err(StatusCode::CONFLICT);
            }
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }

        let (token, refresh_token) = auth_service
            .issue_tokens(&id, &email, role)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        Ok(Json(LoginResponse {
            token,
            refresh_token: Some(refresh_token),
            user: UserInfo {
                tenant_id,
                id,
                email,
                role: role.to_string(),
                first_name: first,
                last_name: last,
            },
        }))
    }

    /// Logout handler
    pub async fn logout(
        State(auth_service): State<HttpAuthService>,
        req: Request,
    ) -> Result<Json<serde_json::Value>, StatusCode> {
        let user = req
            .extensions()
            .get::<AuthUser>()
            .cloned()
            .ok_or(StatusCode::UNAUTHORIZED)?;

        let session_id = EntityId::from_string(&user.session_id)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        auth_service
            .revoke_session(&session_id)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        Ok(Json(
            serde_json::json!({ "message": "Logged out successfully" }),
        ))
    }

    /// Get current user handler
    pub async fn me(req: Request) -> Result<Json<UserInfo>, StatusCode> {
        let user = req
            .extensions()
            .get::<AuthUser>()
            .cloned()
            .ok_or(StatusCode::UNAUTHORIZED)?;

        Ok(Json(UserInfo {
            id: user.id.clone(),
            email: user.email.clone(),
            role: user.role.to_string(),
            first_name: "".to_string(), // TODO: Fetch from database
            last_name: "".to_string(),  // TODO: Fetch from database
            tenant_id: user.tenant_id.clone(),
        }))
    }

    /// Refresh handler
    #[derive(Deserialize)]
    #[allow(non_snake_case)]
    pub struct RefreshRequest {
        pub refreshToken: String,
    }

    pub async fn refresh(
        State(auth_service): State<HttpAuthService>,
        Json(req): Json<RefreshRequest>,
    ) -> Result<Json<LoginResponse>, StatusCode> {
        match auth_service.refresh_access_token(&req.refreshToken).await {
            Ok((token, new_refresh, user)) => {
                let tenant_id: Option<String> = sqlx::query_scalar(
                    "SELECT tenant_id FROM users WHERE id = $1",
                )
                .bind(user.id.to_string())
                .fetch_optional(auth_service.db_pool())
                .await
                .ok()
                .flatten();
                Ok(Json(LoginResponse {
                    token,
                    refresh_token: Some(new_refresh),
                    user: UserInfo {
                        id: user.id.to_string(),
                        email: user.email,
                        role: user.role.to_string(),
                        first_name: user.first_name.unwrap_or_default(),
                        last_name: user.last_name.unwrap_or_default(),
                        tenant_id,
                    },
                }))
            }
            Err(_) => Err(StatusCode::UNAUTHORIZED),
        }
    }
}

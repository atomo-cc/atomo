//! Platform-level GraphQL resolvers implementation for atomo_server
//! 
//! This module provides the concrete implementations of platform functionality 
//! (users, sessions, audit logs) with actual database queries.

use async_graphql::{Context, Object, FieldResult, Error as GraphQLError, InputObject};
use async_trait::async_trait;
use sqlx::{PgPool, Row};
use chrono::{DateTime, Utc};
use std::str::FromStr;

use atomo_core::types::EntityId;
use atomo_core::audit::{AuditLogEntry, AuditOperation};
use crate::platform_models::{PlatformUser, UserSession, UserRole};

/// Platform query provider trait
#[async_trait]
pub trait PlatformQueryProvider: Send + Sync {
    type Error: Send + Sync + 'static;

    async fn get_users(
        &self,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<Vec<PlatformUser>, Self::Error>;

    async fn get_user_sessions(
        &self,
        user_id: Option<String>,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<Vec<UserSession>, Self::Error>;

    async fn get_audit_logs(
        &self,
        entity_type: Option<String>,
        entity_id: Option<String>,
        user_id: Option<String>,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<Vec<AuditLogEntry>, Self::Error>;
}

/// Platform mutation provider trait
#[async_trait]
pub trait PlatformMutationProvider: Send + Sync {
    type Error: Send + Sync + 'static;

    async fn create_user(
        &self,
        email: String,
        first_name: Option<String>,
        last_name: Option<String>,
        role: UserRole,
    ) -> Result<PlatformUser, Self::Error>;

    async fn update_user(
        &self,
        user_id: String,
        email: Option<String>,
        first_name: Option<String>,
        last_name: Option<String>,
        role: Option<UserRole>,
        is_active: Option<bool>,
    ) -> Result<PlatformUser, Self::Error>;

    async fn deactivate_user(
        &self,
        user_id: String,
    ) -> Result<bool, Self::Error>;
}

/// HTTP-based implementation of platform GraphQL queries
/// 
/// This implementation uses PostgreSQL as the backing store and provides
/// the actual database access for platform-level functionality.
pub struct HttpPlatformQueryProvider {
    db_pool: PgPool,
}

impl HttpPlatformQueryProvider {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }

    async fn get_users(
        &self,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<Vec<PlatformUser>, sqlx::Error> {
        let limit = limit.unwrap_or(20);
        let offset = offset.unwrap_or(0);

        let rows = sqlx::query(
            "SELECT id, email, password_hash, first_name, last_name, role, is_active, last_login_at, created_at, updated_at
             FROM platform_users 
             ORDER BY created_at DESC 
             LIMIT $1 OFFSET $2"
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.db_pool)
        .await?;

        let mut users = Vec::new();
        for row in rows {
            let role_str: String = row.get("role");
            let role = match role_str.as_str() {
                "admin" => UserRole::Admin,
                "manager" => UserRole::Manager,
                "sales" => UserRole::Sales,
                "support" => UserRole::Support,
                _ => UserRole::Viewer,
            };

            users.push(PlatformUser {
                id: EntityId::from_str(&row.get::<String, _>("id")).unwrap(),
                email: row.get("email"),
                password_hash: row.get("password_hash"),
                first_name: row.get("first_name"),
                last_name: row.get("last_name"),
                role,
                is_active: row.get("is_active"),
                last_login_at: row.get("last_login_at"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            });
        }

        Ok(users)
    }

    async fn get_user_sessions(
        &self,
        user_id: Option<String>,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<Vec<UserSession>, sqlx::Error> {
        let limit = limit.unwrap_or(20);
        let offset = offset.unwrap_or(0);

        let rows = if let Some(user_id) = user_id {
            sqlx::query(
                "SELECT id, user_id, token, expires_at, ip_address, user_agent, created_at
                 FROM sessions 
                 WHERE user_id = $1
                 ORDER BY created_at DESC 
                 LIMIT $2 OFFSET $3"
            )
            .bind(user_id)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.db_pool)
            .await?
        } else {
            sqlx::query(
                "SELECT id, user_id, token, expires_at, ip_address, user_agent, created_at
                 FROM sessions 
                 ORDER BY created_at DESC 
                 LIMIT $1 OFFSET $2"
            )
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.db_pool)
            .await?
        };

        let mut sessions = Vec::new();
        for row in rows {
            sessions.push(UserSession {
                id: EntityId::from_str(&row.get::<String, _>("id")).unwrap(),
                user_id: EntityId::from_str(&row.get::<String, _>("user_id")).unwrap(),
                token: row.get("token"),
                expires_at: row.get("expires_at"),
                ip_address: row.get("ip_address"),
                user_agent: row.get("user_agent"),
                created_at: row.get("created_at"),
            });
        }

        Ok(sessions)
    }

    async fn get_audit_logs(
        &self,
        entity_type: Option<String>,
        entity_id: Option<String>,
        user_id: Option<String>,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<Vec<AuditLogEntry>, sqlx::Error> {
        let limit = limit.unwrap_or(20);
        let offset = offset.unwrap_or(0);

        let mut query_str = String::from(
            "SELECT id, user_id, action, resource_type, resource_id, details, ip_address, user_agent, timestamp
             FROM audit_logs WHERE 1=1"
        );
        let mut param_count = 0;

        if entity_type.is_some() {
            param_count += 1;
            query_str.push_str(&format!(" AND resource_type = ${}", param_count));
        }
        if entity_id.is_some() {
            param_count += 1;
            query_str.push_str(&format!(" AND resource_id = ${}", param_count));
        }
        if user_id.is_some() {
            param_count += 1;
            query_str.push_str(&format!(" AND user_id = ${}", param_count));
        }

        query_str.push_str(&format!(" ORDER BY timestamp DESC LIMIT ${} OFFSET ${}", param_count + 1, param_count + 2));

        let mut query = sqlx::query(&query_str);
        
        if let Some(et) = entity_type {
            query = query.bind(et);
        }
        if let Some(ei) = entity_id {
            query = query.bind(ei);
        }
        if let Some(ui) = user_id {
            query = query.bind(ui);
        }
        
        query = query.bind(limit).bind(offset);

        let rows = query.fetch_all(&self.db_pool).await?;

        let mut entries = Vec::new();
        for row in rows {
            entries.push(AuditLogEntry {
                id: row.get::<String, _>("id"),
                entity_type: row.get("entity_type"),
                entity_id: EntityId::from_str(&row.get::<String, _>("entity_id")).unwrap(),
                operation: match row.get::<String, _>("operation").as_str() {
                    "create" => AuditOperation::Create,
                    "update" => AuditOperation::Update,
                    "delete" => AuditOperation::Delete,
                    "read" => AuditOperation::Read,
                    _ => AuditOperation::Read, // Default fallback
                },
                operation_details: row.get("operation_details"),
                user_id: row.get::<Option<String>, _>("user_id"),
                ip_address: row.get("ip_address"),
                user_agent: row.get("user_agent"),
                created_at: row.get("created_at"),
            });
        }

        Ok(entries)
    }

    async fn create_user(
        &self,
        email: String,
        password: String,
        first_name: Option<String>,
        last_name: Option<String>,
        role: UserRole,
    ) -> Result<PlatformUser, sqlx::Error> {
        let id = EntityId::new();
        let password_hash = bcrypt::hash(password, bcrypt::DEFAULT_COST).unwrap();
        let role_str = match role {
            UserRole::Admin => "admin",
            UserRole::Manager => "manager", 
            UserRole::Sales => "sales",
            UserRole::Support => "support",
            UserRole::Viewer => "viewer",
        };
        
        let now = Utc::now();

        sqlx::query(
            "INSERT INTO platform_users (id, email, password_hash, first_name, last_name, role, is_active, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)"
        )
        .bind(id.to_string())
        .bind(&email)
        .bind(&password_hash)
        .bind(&first_name)
        .bind(&last_name)
        .bind(role_str)
        .bind(true)
        .bind(now)
        .bind(now)
        .execute(&self.db_pool)
        .await?;

        let user = PlatformUser {
            id,
            email,
            password_hash,
            first_name,
            last_name,
            role,
            is_active: true,
            last_login_at: None,
            created_at: now,
            updated_at: now,
        };

        Ok(user)
    }

    async fn update_user(
        &self,
        id: String,
        email: Option<String>,
        first_name: Option<String>,
        last_name: Option<String>,
        role: Option<UserRole>,
        is_active: Option<bool>,
    ) -> Result<PlatformUser, sqlx::Error> {
        let entity_id = EntityId::from_str(&id).map_err(|_| sqlx::Error::RowNotFound)?;
        let now = Utc::now();

        // Build dynamic update query
        let mut set_clauses = vec!["updated_at = $1".to_string()];
        let mut param_count = 1;
        
        if email.is_some() {
            param_count += 1;
            set_clauses.push(format!("email = ${}", param_count));
        }
        if first_name.is_some() {
            param_count += 1;
            set_clauses.push(format!("first_name = ${}", param_count));
        }
        if last_name.is_some() {
            param_count += 1;
            set_clauses.push(format!("last_name = ${}", param_count));
        }
        if role.is_some() {
            param_count += 1;
            set_clauses.push(format!("role = ${}", param_count));
        }
        if is_active.is_some() {
            param_count += 1;
            set_clauses.push(format!("is_active = ${}", param_count));
        }

        let query_str = format!(
            "UPDATE platform_users SET {} WHERE id = ${}",
            set_clauses.join(", "),
            param_count + 1
        );

        let mut query = sqlx::query(&query_str).bind(now);
        
        if let Some(e) = email.as_ref() {
            query = query.bind(e);
        }
        if let Some(fn_) = first_name.as_ref() {
            query = query.bind(fn_);
        }
        if let Some(ln) = last_name.as_ref() {
            query = query.bind(ln);
        }
        if let Some(r) = role {
            let role_str = match r {
                UserRole::Admin => "admin",
                UserRole::Manager => "manager",
                UserRole::Sales => "sales", 
                UserRole::Support => "support",
                UserRole::Viewer => "viewer",
            };
            query = query.bind(role_str);
        }
        if let Some(ia) = is_active {
            query = query.bind(ia);
        }
        
        query = query.bind(id.clone());
        query.execute(&self.db_pool).await?;

        // Fetch updated user
        let row = sqlx::query(
            "SELECT id, email, password_hash, first_name, last_name, role, is_active, last_login_at, created_at, updated_at
             FROM platform_users WHERE id = $1"
        )
        .bind(id)
        .fetch_one(&self.db_pool)
        .await?;

        let role_str: String = row.get("role");
        let user_role = match role_str.as_str() {
            "admin" => UserRole::Admin,
            "manager" => UserRole::Manager,
            "sales" => UserRole::Sales,
            "support" => UserRole::Support,
            _ => UserRole::Viewer,
        };

        let user = PlatformUser {
            id: entity_id,
            email: row.get("email"),
            password_hash: row.get("password_hash"),
            first_name: row.get("first_name"),
            last_name: row.get("last_name"),
            role: user_role,
            is_active: row.get("is_active"),
            last_login_at: row.get("last_login_at"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        };

        Ok(user)
    }
}

/// GraphQL Query resolver for platform queries
pub struct PlatformQuery {
    provider: HttpPlatformQueryProvider,
}

impl PlatformQuery {
    pub fn new(db_pool: PgPool) -> Self {
        Self {
            provider: HttpPlatformQueryProvider::new(db_pool),
        }
    }
}

#[Object]
impl PlatformQuery {
    /// Get all platform users
    async fn users(
        &self,
        _ctx: &Context<'_>,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> FieldResult<Vec<PlatformUser>> {
        self.provider
            .get_users(limit, offset)
            .await
            .map_err(|e| GraphQLError::new(e.to_string()))
    }

    /// Get user sessions
    async fn user_sessions(
        &self,
        _ctx: &Context<'_>,
        user_id: Option<String>,
        limit: Option<i32>, 
        offset: Option<i32>,
    ) -> FieldResult<Vec<UserSession>> {
        self.provider
            .get_user_sessions(user_id, limit, offset)
            .await
            .map_err(|e| GraphQLError::new(e.to_string()))
    }

    /// Get audit logs
    async fn audit_logs(
        &self,
        _ctx: &Context<'_>,
        entity_type: Option<String>,
        entity_id: Option<String>,
        user_id: Option<String>,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> FieldResult<Vec<AuditLogEntry>> {
        self.provider
            .get_audit_logs(entity_type, entity_id, user_id, limit, offset)
            .await
            .map_err(|e| GraphQLError::new(e.to_string()))
    }
}

/// GraphQL Mutation resolver for platform mutations
pub struct PlatformMutation {
    provider: HttpPlatformMutationProvider,
}

impl PlatformMutation {
    pub fn new(db_pool: PgPool) -> Self {
        Self { provider: HttpPlatformMutationProvider::new(db_pool) }
    }
}

#[Object]
impl PlatformMutation {
    /// Create a new user
    async fn create_user(
        &self,
        _ctx: &Context<'_>,
        email: String,
        first_name: Option<String>,
        last_name: Option<String>,
        role: UserRole,
    ) -> FieldResult<PlatformUser> {
        self.provider
            .create_user(email, first_name, last_name, role)
            .await
            .map_err(|e| GraphQLError::new(e.to_string()))
    }

    /// Update an existing user
    async fn update_user(
        &self,
        _ctx: &Context<'_>,
        id: String,
        email: Option<String>,
        first_name: Option<String>,
        last_name: Option<String>,
        role: Option<UserRole>,
        is_active: Option<bool>,
    ) -> FieldResult<PlatformUser> {
        self.provider
            .update_user(id, email, first_name, last_name, role, is_active)
            .await
            .map_err(|e| GraphQLError::new(e.to_string()))
    }
}

/// HTTP-based implementation of platform GraphQL mutations
/// 
/// This implementation uses PostgreSQL as the backing store and provides
/// the actual database access for platform-level user management.
pub struct HttpPlatformMutationProvider {
    db_pool: PgPool,
}

impl HttpPlatformMutationProvider {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }

    pub fn pool(&self) -> &PgPool { &self.db_pool }
}

#[async_trait]
impl PlatformMutationProvider for HttpPlatformMutationProvider {
    type Error = sqlx::Error;

    async fn create_user(
        &self,
        email: String,
        first_name: Option<String>,
        last_name: Option<String>,
        role: UserRole,
    ) -> Result<PlatformUser, Self::Error> {
        // Generate a temporary password hash (should be replaced with proper implementation)
        let password_hash = "temp_hash".to_string(); // TODO: Implement proper password hashing
        let id = EntityId::new();
        
        let row = sqlx::query(
            r#"
            INSERT INTO users (id, email, password_hash, first_name, last_name, role, is_active, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, true, NOW(), NOW())
            RETURNING id, email, password_hash, first_name, last_name, role, is_active, last_login_at, created_at, updated_at
            "#
        )
        .bind(id.to_string())
        .bind(&email)
        .bind(&password_hash)
        .bind(&first_name)
        .bind(&last_name)
        .bind(role.to_string())
        .fetch_one(&self.db_pool)
        .await?;
        
        let user = PlatformUser {
            id: EntityId::from_string(&row.get::<String, _>("id")).unwrap(),
            email: row.get("email"),
            password_hash: row.get("password_hash"),
            first_name: row.get::<Option<String>, _>("first_name"),
            last_name: row.get::<Option<String>, _>("last_name"),
            role: UserRole::from_str(&row.get::<String, _>("role")).unwrap(),
            is_active: row.get("is_active"),
            last_login_at: row.get("last_login_at"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        };
        
        Ok(user)
    }

    async fn update_user(
        &self,
        user_id: String,
        email: Option<String>,
        first_name: Option<String>,
        last_name: Option<String>,
        role: Option<UserRole>,
        is_active: Option<bool>,
    ) -> Result<PlatformUser, Self::Error> {
        let entity_id: EntityId = user_id.parse().map_err(|_| {
            sqlx::Error::ColumnDecode { 
                index: "user_id".to_string(), 
                source: Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid user ID format"))
            }
        })?;

        // Build dynamic query based on provided fields
        let mut query_parts = Vec::new();
        let mut bind_count = 1;
        
        if email.is_some() {
            query_parts.push(format!("email = ${}", bind_count));
            bind_count += 1;
        }
        if first_name.is_some() {
            query_parts.push(format!("first_name = ${}", bind_count));
            bind_count += 1;
        }
        if last_name.is_some() {
            query_parts.push(format!("last_name = ${}", bind_count));
            bind_count += 1;
        }
        if role.is_some() {
            query_parts.push(format!("role = ${}", bind_count));
            bind_count += 1;
        }
        if is_active.is_some() {
            query_parts.push(format!("is_active = ${}", bind_count));
            bind_count += 1;
        }
        
        query_parts.push("updated_at = NOW()".to_string());
        
        let query_str = format!(
            r#"
            UPDATE users 
            SET {}
            WHERE id = ${}
            RETURNING id, email, password_hash, first_name, last_name, role, is_active, last_login_at, created_at, updated_at
            "#,
            query_parts.join(", "),
            bind_count
        );
        
        let mut query = sqlx::query(&query_str);
        
        if let Some(e) = email {
            query = query.bind(e);
        }
        if let Some(fn_) = first_name {
            query = query.bind(fn_);
        }
        if let Some(ln) = last_name {
            query = query.bind(ln);
        }
        if let Some(r) = role {
            query = query.bind(r.to_string());
        }
        if let Some(active) = is_active {
            query = query.bind(active);
        }
        
        query = query.bind(entity_id.to_string());
        
        let row = query.fetch_one(&self.db_pool).await?;
        
        let user = PlatformUser {
            id: EntityId::from_string(&row.get::<String, _>("id")).unwrap(),
            email: row.get("email"),
            password_hash: row.get("password_hash"),
            first_name: row.get::<Option<String>, _>("first_name"),
            last_name: row.get::<Option<String>, _>("last_name"),
            role: UserRole::from_str(&row.get::<String, _>("role")).unwrap(),
            is_active: row.get("is_active"),
            last_login_at: row.get("last_login_at"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        };
        
        Ok(user)
    }

    async fn deactivate_user(
        &self,
        user_id: String,
    ) -> Result<bool, Self::Error> {
        let entity_id: EntityId = user_id.parse().map_err(|_| {
            sqlx::Error::ColumnDecode { 
                index: "user_id".to_string(), 
                source: Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid user ID format"))
            }
        })?;

        let result = sqlx::query(
            r#"
            UPDATE users 
            SET is_active = false, updated_at = NOW()
            WHERE id = $1
            "#
        )
        .bind(entity_id.to_string())
        .execute(&self.db_pool)
        .await?;
        
        Ok(result.rows_affected() > 0)
    }
}

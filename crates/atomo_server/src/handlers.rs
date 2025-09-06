//! HTTP handlers for the Atomo server

use async_graphql::{Schema as GraphQLSchema, MergedObject};
use async_graphql_axum::{GraphQLRequest, GraphQLResponse};
use axum::{
    extract::{Extension, State},
    response::{Html, Json},
    routing::{get, post},
    Router,
};
use atomo::prelude::*;
use atomo_core::types::EntityId;
use atomo::graphql::{Query as ServiceQuery, Mutation as ServiceMutation, Subscription};
use crate::platform_graphql::{PlatformQuery, PlatformMutation};
use serde_json::{json, Value};

/// Combined query that merges service-level and platform-level queries
#[derive(MergedObject)]
pub struct Query(ServiceQuery, PlatformQuery);

/// Combined mutation that merges service-level and platform-level mutations  
#[derive(MergedObject)]
pub struct Mutation(ServiceMutation, PlatformMutation);

pub type AtomoGraphQLSchema = GraphQLSchema<Query, Mutation, Subscription>;

/// Build extended GraphQL schema that includes both service and platform queries
pub fn build_extended_schema(atomo: &Atomo) -> AtomoGraphQLSchema {
    use std::sync::Arc;
    
    // Get service-level components
    let client = Arc::new(atomo.client().clone());
    let schema = atomo.schema().clone();
    let pool = atomo.db_pool().clone();
    
    // Create service-level components
    let service_query = ServiceQuery::new(client.clone(), schema.clone());
    let service_mutation = ServiceMutation::new(client.clone(), schema.clone());
    let subscription = Subscription::new(client.clone());
    
    // Create platform-level components
    let platform_query = PlatformQuery::new(pool.clone());
    let platform_mutation = PlatformMutation::new(pool.clone());
    
    // Merge queries and mutations
    let query = Query(service_query, platform_query);
    let mutation = Mutation(service_mutation, platform_mutation);
    
    GraphQLSchema::build(query, mutation, subscription)
        .data(client)
        .data(pool)
        .finish()
}

pub async fn graphql_handler(
    Extension(schema): Extension<AtomoGraphQLSchema>,
    req: GraphQLRequest,
) -> GraphQLResponse {
    schema.execute(req.into_inner()).await.into()
}

pub async fn graphql_playground() -> Html<String> {
    let source = async_graphql::http::playground_source(
        async_graphql::http::GraphQLPlaygroundConfig::new("/graphql"),
    );
    Html(source)
}

pub async fn health_check() -> &'static str {
    "OK"
}

pub async fn atomo_info(Extension(_atomo): Extension<Atomo>) -> String {
    // You could add schema introspection here
    format!("🚀 Atomo Content Core Server\n📊 Schema loaded successfully")
}

/// Schema metadata endpoint for Admin UI
/// Returns JSON metadata describing the entire data model for dynamic rendering
pub async fn schema_metadata(Extension(atomo): Extension<Atomo>) -> Json<Value> {
    // Extract actual schema metadata from Atomo instance including platform models
    let metadata = crate::schema_metadata::extract_schema_metadata(&atomo);
    
    // Fallback: also include hardcoded CRM models for backward compatibility
    let mut extended_metadata = metadata;
    if let Some(models) = extended_metadata.get_mut("models") {
        // Add hardcoded CRM models if they don't exist yet
        if models.get("Contact").is_none() {
            models["Contact"] = json!({
                "tableName": "contact",
                "primaryKey": "id",
                "searchable": ["firstName", "lastName", "email"],
                "relationships": {
                    "company": {
                        "type": "belongsTo",
                        "model": "Company",
                        "foreignKey": "companyId"
                    },
                    "deals": {
                        "type": "hasMany",
                        "model": "Deal",
                        "foreignKey": "contactId"
                    }
                },
                "validation": {
                    "email": "email",
                    "firstName": "required|min:1|max:100",
                    "lastName": "max:100"
                },
                "ui": {
                    "displayField": ["firstName", "lastName"],
                    "listView": ["firstName", "lastName", "email", "companyId", "createdAt"],
                    "editForm": ["firstName", "lastName", "email", "phone", "companyId", "tags", "notes"]
                },
                "fields": {
                    "id": {
                        "name": "id",
                        "type": "string",
                        "optional": false,
                        "attributes": ["primary"],
                        "ui": {
                            "label": "ID",
                            "component": "readonly"
                        }
                    },
                    "firstName": {
                        "name": "firstName",
                        "type": "string",
                        "optional": false,
                        "attributes": ["required"],
                        "ui": {
                            "label": "名字",
                            "placeholder": "请输入名字"
                        }
                    },
                    "lastName": {
                        "name": "lastName",
                        "type": "string",
                        "optional": false,
                        "attributes": ["required"],
                        "ui": {
                            "label": "姓氏",
                            "placeholder": "请输入姓氏"
                        }
                    },
                    "email": {
                        "name": "email",
                        "type": "string",
                        "optional": false,
                        "attributes": ["required"],
                        "ui": {
                            "label": "邮箱",
                            "placeholder": "请输入邮箱地址"
                        }
                    },
                    "phone": {
                        "name": "phone",
                        "type": "string",
                        "optional": true,
                        "attributes": [],
                        "ui": {
                            "label": "电话",
                            "placeholder": "请输入电话号码"
                        }
                    },
                    "companyId": {
                        "name": "companyId",
                        "type": "reference",
                        "optional": true,
                        "attributes": [],
                        "ui": {
                            "label": "公司",
                            "component": "reference-select"
                        }
                    },
                    "tags": {
                        "name": "tags",
                        "type": "array",
                        "optional": false,
                        "attributes": [],
                        "ui": {
                            "label": "标签",
                            "component": "tag-input"
                        }
                    },
                    "notes": {
                        "name": "notes",
                        "type": "blocks",
                        "optional": false,
                        "attributes": [],
                        "ui": {
                            "label": "备注",
                            "component": "blocks-editor"
                        }
                    },
                    "createdAt": {
                        "name": "createdAt",
                        "type": "datetime",
                        "optional": false,
                        "attributes": ["readonly"],
                        "ui": {
                            "label": "创建时间",
                            "component": "readonly"
                        }
                    },
                    "updatedAt": {
                        "name": "updatedAt",
                        "type": "datetime",
                        "optional": false,
                        "attributes": ["readonly"],
                        "ui": {
                            "label": "更新时间",
                            "component": "readonly"
                        }
                    }
                }
            });
            
            models["Company"] = json!({
                "tableName": "company",
                "primaryKey": "id",
                "searchable": ["name", "website", "industry"],
                "relationships": {
                    "contacts": {
                        "type": "hasMany",
                        "model": "Contact",
                        "foreignKey": "companyId"
                    },
                    "deals": {
                        "type": "hasMany",
                        "model": "Deal",
                        "foreignKey": "companyId"
                    }
                },
                "validation": {
                    "name": "required|min:1|max:255",
                    "website": "url",
                    "email": "email"
                },
                "ui": {
                    "displayField": "name",
                    "listView": ["name", "website", "industry", "size", "createdAt"],
                    "editForm": ["name", "website", "address", "industry", "size", "notes"]
                },
                "fields": {
                    "id": {
                        "name": "id",
                        "type": "string",
                        "optional": false,
                        "attributes": ["primary"],
                        "ui": {
                            "label": "ID",
                            "component": "readonly"
                        }
                    },
                    "name": {
                        "name": "name",
                        "type": "string",
                        "optional": false,
                        "attributes": ["required"],
                        "ui": {
                            "label": "公司名称",
                            "placeholder": "请输入公司名称"
                        }
                    },
                    "website": {
                        "name": "website",
                        "type": "string",
                        "optional": true,
                        "attributes": [],
                        "ui": {
                            "label": "网站",
                            "placeholder": "https://example.com"
                        }
                    },
                    "address": {
                        "name": "address",
                        "type": "text",
                        "optional": true,
                        "attributes": [],
                        "ui": {
                            "label": "地址",
                            "placeholder": "请输入公司地址"
                        }
                    },
                    "industry": {
                        "name": "industry",
                        "type": "string",
                        "optional": true,
                        "attributes": [],
                        "ui": {
                            "label": "行业",
                            "placeholder": "请输入行业"
                        }
                    },
                    "size": {
                        "name": "size",
                        "type": "string",
                        "optional": true,
                        "attributes": [],
                        "ui": {
                            "label": "公司规模",
                            "component": "select",
                            "options": [
                                {"value": "startup", "label": "初创公司 (1-10人)"},
                                {"value": "small", "label": "小公司 (11-50人)"},
                                {"value": "medium", "label": "中型公司 (51-200人)"},
                                {"value": "large", "label": "大公司 (201-1000人)"},
                                {"value": "enterprise", "label": "企业 (1000+人)"}
                            ]
                        }
                    },
                    "notes": {
                        "name": "notes",
                        "type": "blocks",
                        "optional": false,
                        "attributes": [],
                        "ui": {
                            "label": "备注",
                            "component": "blocks-editor"
                        }
                    },
                    "createdAt": {
                        "name": "createdAt",
                        "type": "datetime",
                        "optional": false,
                        "attributes": ["readonly"],
                        "ui": {
                            "label": "创建时间",
                            "component": "readonly"
                        }
                    },
                    "updatedAt": {
                        "name": "updatedAt",
                        "type": "datetime",
                        "optional": false,
                        "attributes": ["readonly"],
                        "ui": {
                            "label": "更新时间",
                            "component": "readonly"
                        }
                    }
                }
            });
            
            models["Deal"] = json!({
                "tableName": "deal",
                "primaryKey": "id",
                "searchable": ["title"],
                "relationships": {
                    "contact": {
                        "type": "belongsTo",
                        "model": "Contact",
                        "foreignKey": "contactId"
                    },
                    "company": {
                        "type": "belongsTo",
                        "model": "Company",
                        "foreignKey": "companyId"
                    }
                },
                "validation": {
                    "title": "required|min:1|max:255",
                    "value": "numeric|min:0",
                    "contactId": "required|exists:contacts,id"
                },
                "ui": {
                    "displayField": "title",
                    "listView": ["title", "value", "stage", "contactId", "companyId", "expectedCloseDate"],
                    "editForm": ["title", "value", "stage", "contactId", "companyId", "description", "expectedCloseDate"]
                },
                "fields": {
                    "id": {
                        "name": "id",
                        "type": "string",
                        "optional": false,
                        "attributes": ["primary"],
                        "ui": {
                            "label": "ID",
                            "component": "readonly"
                        }
                    },
                    "title": {
                        "name": "title",
                        "type": "string",
                        "optional": false,
                        "attributes": ["required"],
                        "ui": {
                            "label": "交易标题",
                            "placeholder": "请输入交易标题"
                        }
                    },
                    "value": {
                        "name": "value",
                        "type": "number",
                        "optional": false,
                        "attributes": ["required"],
                        "ui": {
                            "label": "交易金额",
                            "placeholder": "0.00"
                        }
                    },
                    "stage": {
                        "name": "stage",
                        "type": "string",
                        "optional": false,
                        "attributes": ["required"],
                        "ui": {
                            "label": "阶段",
                            "component": "select",
                            "options": [
                                {"value": "lead", "label": "潜在客户"},
                                {"value": "qualified", "label": "合格线索"},
                                {"value": "proposal", "label": "提案阶段"},
                                {"value": "negotiation", "label": "谈判阶段"},
                                {"value": "closed_won", "label": "成交"},
                                {"value": "closed_lost", "label": "失败"}
                            ]
                        }
                    },
                    "contactId": {
                        "name": "contactId",
                        "type": "reference",
                        "optional": false,
                        "attributes": ["required"],
                        "ui": {
                            "label": "联系人",
                            "component": "reference-select"
                        }
                    },
                    "companyId": {
                        "name": "companyId",
                        "type": "reference",
                        "optional": true,
                        "attributes": [],
                        "ui": {
                            "label": "公司",
                            "component": "reference-select"
                        }
                    },
                    "description": {
                        "name": "description",
                        "type": "blocks",
                        "optional": false,
                        "attributes": [],
                        "ui": {
                            "label": "描述",
                            "component": "blocks-editor"
                        }
                    },
                    "expectedCloseDate": {
                        "name": "expectedCloseDate",
                        "type": "date",
                        "optional": true,
                        "attributes": [],
                        "ui": {
                            "label": "预期成交日期",
                            "component": "date-picker"
                        }
                    },
                    "actualCloseDate": {
                        "name": "actualCloseDate",
                        "type": "date",
                        "optional": true,
                        "attributes": [],
                        "ui": {
                            "label": "实际成交日期",
                            "component": "date-picker"
                        }
                    },
                    "createdAt": {
                        "name": "createdAt",
                        "type": "datetime",
                        "optional": false,
                        "attributes": ["readonly"],
                        "ui": {
                            "label": "创建时间",
                            "component": "readonly"
                        }
                    },
                    "updatedAt": {
                        "name": "updatedAt",
                        "type": "datetime",
                        "optional": false,
                        "attributes": ["readonly"],
                        "ui": {
                            "label": "更新时间",
                            "component": "readonly"
                        }
                    }
                }
            });
        }
    }
    
    Json(extended_metadata)
}

pub fn create_router(
    schema: AtomoGraphQLSchema, 
    atomo: Atomo, 
    auth_service: crate::auth::HttpAuthService,
    audit_service: crate::audit::HttpAuditService,
) -> Router {
    use crate::auth::{auth_middleware, optional_auth_middleware, handlers};
    use axum::middleware;

    let protected_routes = Router::new()
        .route("/graphql", post(graphql_handler))
        .route_layer(middleware::from_fn_with_state(auth_service.clone(), auth_middleware));

    let semi_protected_routes = Router::new()
        .route("/meta/schema", get(schema_metadata))
        .route("/graphql", get(graphql_playground))
        .route_layer(middleware::from_fn_with_state(auth_service.clone(), optional_auth_middleware));

    Router::new()
        // Public routes (no authentication required)
        .route("/", get(|| async { "🚀 Atomo Content Core Server" }))
        .route("/health", get(health_check))
        .route("/info", get(atomo_info))
        
        // Authentication routes  
        .nest("/auth", Router::new()
            .route("/login", post(handlers::login))
            .route("/logout", post(handlers::logout))
            .route("/me", get(handlers::me))
            .with_state(auth_service.clone())
        )
        
        // Audit log routes
        .nest("/audit", Router::new()
            .route("/logs", get(get_audit_logs))
            .route("/user/:user_id/activity", get(get_user_activity))
            .route("/entity/:entity_type/:entity_id/audit", get(get_entity_audit))
            .route("/statistics", get(get_audit_statistics))
            .with_state(audit_service.clone())
        )
        
        // Merge protected and semi-protected routes
        .merge(protected_routes)
        .merge(semi_protected_routes)
        
        // Add state and extensions
        .with_state(auth_service)
        .layer(Extension(schema))
        .layer(Extension(atomo))
}

// ============================================================================
// Audit Log Handlers
// ============================================================================

/// Get audit logs with optional filters
pub async fn get_audit_logs(
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
    State(audit_service): State<crate::audit::HttpAuditService>,
    req: axum::extract::Request,
) -> Result<Json<Vec<atomo_core::audit::AuditLogEntry>>, axum::http::StatusCode> {
    use atomo_core::audit::{AuditService, AuditSearchFilters, AuditOperation};
    use atomo_core::types::{EntityId, StreamId};
    
    // Check authentication
    let _user = req.extensions()
        .get::<crate::auth::AuthUser>()
        .cloned()
        .ok_or(axum::http::StatusCode::UNAUTHORIZED)?;

    // TODO: Check if user has permission to view audit logs (admin/manager only)
    
    // Parse query parameters into filter
    let filters = AuditSearchFilters {
        entity_type: params.get("entity_type").cloned(),
        entity_id: params.get("entity_id")
            .and_then(|s| EntityId::from_string(s).ok()),
        stream_id: params.get("stream_id")
            .and_then(|s| StreamId::from_string(s).ok()),
        operation: params.get("operation")
            .and_then(|s| match s.as_str() {
                "create" => Some(AuditOperation::Create),
                "update" => Some(AuditOperation::Update),
                "delete" => Some(AuditOperation::Delete),
                "read" => Some(AuditOperation::Read),
                _ => None,
            }),
        user_id: params.get("user_id").cloned(),
        start_time: params.get("start_date")
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&chrono::Utc)),
        end_time: params.get("end_date")
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&chrono::Utc)),
        limit: params.get("limit").and_then(|s| s.parse().ok()),
        offset: params.get("offset").and_then(|s| s.parse().ok()),
    };

    match audit_service.search_audit_logs(&filters).await {
        Ok(logs) => Ok(Json(logs)),
        Err(_) => Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// Get user activity summary
pub async fn get_user_activity(
    axum::extract::Path(user_id): axum::extract::Path<String>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
    State(audit_service): State<crate::audit::HttpAuditService>,
    req: axum::extract::Request,
) -> Result<Json<Vec<atomo_core::audit::AuditLogEntry>>, axum::http::StatusCode> {
    use atomo_core::audit::AuditService;
    
    // Check authentication
    let current_user = req.extensions()
        .get::<crate::auth::AuthUser>()
        .cloned()
        .ok_or(axum::http::StatusCode::UNAUTHORIZED)?;

    // Users can only view their own activity unless they're admin/manager
    if current_user.id != user_id && !matches!(current_user.role, crate::platform_models::UserRole::Admin | crate::platform_models::UserRole::Manager) {
        return Err(axum::http::StatusCode::FORBIDDEN);
    }

    let limit = params.get("limit")
        .and_then(|s| s.parse().ok())
        .unwrap_or(100);

    let user_entity_id = match EntityId::from_string(&user_id) {
        Ok(id) => id,
        Err(_) => return Err(axum::http::StatusCode::BAD_REQUEST),
    };

    match audit_service.get_audit_logs_for_user(&user_entity_id).await {
        Ok(entries) => Ok(Json(entries)),
        Err(_) => Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// Get entity audit summary
pub async fn get_entity_audit(
    axum::extract::Path((entity_type, entity_id)): axum::extract::Path<(String, String)>,
    State(audit_service): State<crate::audit::HttpAuditService>,
    req: axum::extract::Request,
) -> Result<Json<Vec<atomo_core::audit::AuditLogEntry>>, axum::http::StatusCode> {
    use atomo_core::audit::AuditService;
    use atomo_core::types::EntityId;
    
    // Check authentication
    let _user = req.extensions()
        .get::<crate::auth::AuthUser>()
        .cloned()
        .ok_or(axum::http::StatusCode::UNAUTHORIZED)?;

    let entity_id = EntityId::from_string(&entity_id)
        .map_err(|_| axum::http::StatusCode::BAD_REQUEST)?;

    match audit_service.get_audit_logs_for_entity(&entity_type, &entity_id).await {
        Ok(entries) => Ok(Json(entries)),
        Err(_) => Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// Get audit statistics
pub async fn get_audit_statistics(
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
    State(audit_service): State<crate::audit::HttpAuditService>,
    req: axum::extract::Request,
) -> Result<Json<atomo_core::audit::AuditStats>, axum::http::StatusCode> {
    use atomo_core::audit::{AuditService, AuditSearchFilters};
    
    // Check authentication - only admin/manager can view statistics
    let user = req.extensions()
        .get::<crate::auth::AuthUser>()
        .cloned()
        .ok_or(axum::http::StatusCode::UNAUTHORIZED)?;

    if !matches!(user.role, crate::platform_models::UserRole::Admin | crate::platform_models::UserRole::Manager) {
        return Err(axum::http::StatusCode::FORBIDDEN);
    }

    // Parse date range for statistics
    let filters = AuditSearchFilters {
        start_time: params.get("start_date")
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&chrono::Utc)),
        end_time: params.get("end_date")
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&chrono::Utc)),
        ..Default::default()
    };

    match audit_service.get_audit_stats(&filters).await {
        Ok(stats) => Ok(Json(stats)),
        Err(_) => Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR),
    }
}

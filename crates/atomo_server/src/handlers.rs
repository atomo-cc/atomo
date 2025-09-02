//! HTTP handlers for the Atomo server

use axum::{
    extract::Extension,
    response::{Html, Json},
    routing::{get, post},
    Router,
};
use async_graphql_axum::{GraphQLRequest, GraphQLResponse};
use async_graphql::{Schema as GraphQLSchema};
use atomo::prelude::*;
use atomo::graphql::{Query, Mutation, Subscription};
use serde_json::{json, Value};

pub type AtomoGraphQLSchema = GraphQLSchema<Query, Mutation, Subscription>;

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
    // For now, return a sample schema based on the CRM service
    // TODO: Extract actual schema metadata from Atomo instance
    let metadata = json!({
        "models": {
            "Contact": {
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
            },
            "Company": {
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
            },
            "Deal": {
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
            }
        },
        "config": {
            "auditLog": true,
            "softDeletes": true,
            "defaultPageSize": 20,
            "subscriptions": true
        }
    });

    Json(metadata)
}

pub fn create_router(schema: AtomoGraphQLSchema, atomo: Atomo) -> Router {
    Router::new()
        .route("/", get(|| async { "🚀 Atomo Content Core Server" }))
        .route("/health", get(health_check))
        .route("/info", get(atomo_info))
        .route("/meta/schema", get(schema_metadata))
        .route("/graphql", post(graphql_handler))
        .route("/graphql", get(graphql_playground))
        .layer(Extension(schema))
        .layer(Extension(atomo))
}

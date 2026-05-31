use async_graphql::{Error as GqlError, ErrorExtensions};

#[derive(Debug, Clone)]
pub enum AtomoError {
    NotFound { model: String, id: String },
    Unauthorized { message: String },
    Forbidden { message: String },
    ValidationFailed { errors: Vec<FieldError> },
    Internal { message: String },
}

#[derive(Debug, Clone)]
pub struct FieldError {
    pub field: String,
    pub message: String,
    pub code: String,
}

impl From<AtomoError> for GqlError {
    fn from(err: AtomoError) -> Self {
        match err {
            AtomoError::NotFound { model, id } => {
                GqlError::new(format!("{} with id '{}' not found", model, id)).extend_with(
                    |_, e| {
                        e.set("code", "NOT_FOUND");
                        e.set("model", model.as_str());
                    },
                )
            }
            AtomoError::Unauthorized { message } => GqlError::new(message).extend_with(|_, e| {
                e.set("code", "UNAUTHORIZED");
            }),
            AtomoError::Forbidden { message } => GqlError::new(message).extend_with(|_, e| {
                e.set("code", "FORBIDDEN");
            }),
            AtomoError::ValidationFailed { errors } => {
                let msg = errors
                    .iter()
                    .map(|e| e.message.clone())
                    .collect::<Vec<_>>()
                    .join("; ");
                let field_errors: Vec<serde_json::Value> = errors.iter().map(|e| {
                    serde_json::json!({ "field": e.field, "message": e.message, "code": e.code })
                }).collect();
                GqlError::new(msg).extend_with(|_, e| {
                    e.set("code", "VALIDATION_ERROR");
                    e.set(
                        "fields",
                        serde_json::to_string(&field_errors).unwrap_or_default(),
                    );
                })
            }
            AtomoError::Internal { message } => GqlError::new(message).extend_with(|_, e| {
                e.set("code", "INTERNAL_ERROR");
            }),
        }
    }
}

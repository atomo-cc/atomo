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

#[cfg(test)]
mod tests {
    use super::*;

    // Map each AtomoError variant to a GqlError and assert the message + code extension.
    fn code_of(err: AtomoError) -> (String, String) {
        let g: GqlError = err.into();
        // Extensions are serialized into the error; render to a string and check the code.
        let msg = g.message.clone();
        let ext = format!("{:?}", g.extensions);
        (msg, ext)
    }

    #[test]
    fn not_found_maps_message_and_code() {
        let (msg, ext) = code_of(AtomoError::NotFound {
            model: "Contact".into(),
            id: "7".into(),
        });
        assert!(msg.contains("Contact") && msg.contains("'7'") && msg.contains("not found"));
        assert!(ext.contains("NOT_FOUND"));
    }

    #[test]
    fn auth_errors_carry_codes() {
        assert!(code_of(AtomoError::Unauthorized {
            message: "x".into()
        })
        .1
        .contains("UNAUTHORIZED"));
        assert!(code_of(AtomoError::Forbidden {
            message: "x".into()
        })
        .1
        .contains("FORBIDDEN"));
        assert!(code_of(AtomoError::Internal {
            message: "x".into()
        })
        .1
        .contains("INTERNAL_ERROR"));
    }

    #[test]
    fn validation_failed_joins_messages_and_sets_code() {
        let (msg, ext) = code_of(AtomoError::ValidationFailed {
            errors: vec![
                FieldError {
                    field: "email".into(),
                    message: "bad email".into(),
                    code: "email".into(),
                },
                FieldError {
                    field: "name".into(),
                    message: "required".into(),
                    code: "required".into(),
                },
            ],
        });
        assert!(msg.contains("bad email") && msg.contains("required"));
        assert!(ext.contains("VALIDATION_ERROR"));
    }
}

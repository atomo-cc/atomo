pub use atomo_schema::{
    ActionCondition, ActionDef, ActionInputDef, ActionInputField, ActionReturn, EventActionBinding,
    ModelEvents,
};

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::HashMap;

    #[test]
    fn changed_any_matches_when_field_differs() {
        let cond = ActionCondition::ChangedAny(vec!["title".into()]);
        let mut current = HashMap::new();
        current.insert("title".into(), json!("new"));
        let mut previous = HashMap::new();
        previous.insert("title".into(), json!("old"));

        assert!(cond.matches(&current, Some(&previous)));
    }

    #[test]
    fn changed_any_skips_when_field_unchanged() {
        let cond = ActionCondition::ChangedAny(vec!["title".into()]);
        let mut current = HashMap::new();
        current.insert("title".into(), json!("same"));
        let mut previous = HashMap::new();
        previous.insert("title".into(), json!("same"));

        assert!(!cond.matches(&current, Some(&previous)));
    }

    #[test]
    fn changed_any_fires_on_create_no_previous() {
        let cond = ActionCondition::ChangedAny(vec!["title".into()]);
        let mut current = HashMap::new();
        current.insert("title".into(), json!("hello"));

        assert!(cond.matches(&current, None));
    }

    #[test]
    fn field_equals_matches() {
        let cond = ActionCondition::FieldEquals {
            field: "status".into(),
            value: json!("published"),
        };
        let mut current = HashMap::new();
        current.insert("status".into(), json!("published"));

        assert!(cond.matches(&current, None));
    }

    #[test]
    fn build_input_picks_fields() {
        let action = ActionDef {
            name: "processPost".into(),
            input: ActionInputDef::PickFields {
                model: "posts".into(),
                fields: vec!["id".into(), "title".into()],
            },
            returns: None,
            source_model: Some("posts".into()),
        };
        let mut doc = HashMap::new();
        doc.insert("id".into(), json!("123"));
        doc.insert("title".into(), json!("Hello"));
        doc.insert("content".into(), json!("body"));
        doc.insert("authorId".into(), json!("u1"));

        let input = action.build_input(&doc);
        assert_eq!(input.len(), 2);
        assert_eq!(input["id"], json!("123"));
        assert_eq!(input["title"], json!("Hello"));
    }

    #[test]
    fn lifecycle_action_is_not_callable() {
        let action = ActionDef {
            name: "processPost".into(),
            input: ActionInputDef::PickFields {
                model: "posts".into(),
                fields: vec!["id".into()],
            },
            returns: None,
            source_model: Some("posts".into()),
        };
        assert!(action.is_lifecycle_only());
        assert!(!action.is_callable());
    }

    #[test]
    fn callable_action_returns_model() {
        let action = ActionDef {
            name: "createPostWithScore".into(),
            input: ActionInputDef::Object {
                fields: vec![ActionInputField {
                    name: "title".into(),
                    field_type: "string".into(),
                    required: true,
                }],
            },
            returns: Some(ActionReturn::Model("posts".into())),
            source_model: None,
        };
        assert!(action.is_callable());
        assert!(!action.is_lifecycle_only());
    }

    #[test]
    fn serde_round_trip() {
        let action = ActionDef {
            name: "processPost".into(),
            input: ActionInputDef::PickFields {
                model: "posts".into(),
                fields: vec!["id".into(), "title".into()],
            },
            returns: None,
            source_model: Some("posts".into()),
        };
        let json = serde_json::to_string(&action).unwrap();
        let back: ActionDef = serde_json::from_str(&json).unwrap();
        assert_eq!(back.name, "processPost");
    }
}

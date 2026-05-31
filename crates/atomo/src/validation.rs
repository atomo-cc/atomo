use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct ValidationError {
    pub field: String,
    pub message: String,
    pub code: String,
}

/// Validate data against a set of field rules.
/// Rules format: "required|email|min:1|max:100|numeric"
pub fn validate(
    data: &HashMap<String, Value>,
    rules: &HashMap<String, String>,
) -> Vec<ValidationError> {
    let mut errors = Vec::new();
    for (field, rule_str) in rules {
        let value = data.get(field);
        for rule in rule_str.split('|') {
            if let Some(err) = check_rule(field, value, rule) {
                errors.push(err);
            }
        }
    }
    errors
}

/// Update-aware validation: only validate rules for fields actually present in the patch.
/// A partial update that omits a `required` field must NOT fail (the field keeps its stored
/// value); but a field that IS being set must still satisfy its rules (e.g. non-empty title).
pub fn validate_partial(
    data: &HashMap<String, Value>,
    rules: &HashMap<String, String>,
) -> Vec<ValidationError> {
    let mut errors = Vec::new();
    for (field, rule_str) in rules {
        if !data.contains_key(field) {
            continue; // field not in this patch — leave its stored value untouched
        }
        let value = data.get(field);
        for rule in rule_str.split('|') {
            if let Some(err) = check_rule(field, value, rule) {
                errors.push(err);
            }
        }
    }
    errors
}

fn check_rule(field: &str, value: Option<&Value>, rule: &str) -> Option<ValidationError> {
    let (rule_name, param) = if let Some(idx) = rule.find(':') {
        (&rule[..idx], Some(&rule[idx + 1..]))
    } else {
        (rule, None)
    };

    match rule_name {
        "required" => {
            let missing = match value {
                None | Some(Value::Null) => true,
                Some(Value::String(s)) if s.is_empty() => true,
                _ => false,
            };
            if missing {
                return Some(ValidationError {
                    field: field.to_string(),
                    message: format!("{} is required", field),
                    code: "required".to_string(),
                });
            }
        }
        "email" => {
            if let Some(Value::String(s)) = value {
                if !s.is_empty() && (!s.contains('@') || !s.contains('.')) {
                    return Some(ValidationError {
                        field: field.to_string(),
                        message: format!("{} must be a valid email", field),
                        code: "email".to_string(),
                    });
                }
            }
        }
        "min" => {
            let min: usize = param?.parse().ok()?;
            match value {
                Some(Value::String(s)) if s.len() < min => {
                    return Some(ValidationError {
                        field: field.to_string(),
                        message: format!("{} must be at least {} characters", field, min),
                        code: "min_length".to_string(),
                    });
                }
                Some(Value::Number(n)) => {
                    if let Some(v) = n.as_f64() {
                        if v < min as f64 {
                            return Some(ValidationError {
                                field: field.to_string(),
                                message: format!("{} must be at least {}", field, min),
                                code: "min_value".to_string(),
                            });
                        }
                    }
                }
                _ => {}
            }
        }
        "max" => {
            let max: usize = param?.parse().ok()?;
            match value {
                Some(Value::String(s)) if s.len() > max => {
                    return Some(ValidationError {
                        field: field.to_string(),
                        message: format!("{} must be at most {} characters", field, max),
                        code: "max_length".to_string(),
                    });
                }
                Some(Value::Number(n)) => {
                    if let Some(v) = n.as_f64() {
                        if v > max as f64 {
                            return Some(ValidationError {
                                field: field.to_string(),
                                message: format!("{} must be at most {}", field, max),
                                code: "max_value".to_string(),
                            });
                        }
                    }
                }
                _ => {}
            }
        }
        "numeric" => {
            if let Some(val) = value {
                match val {
                    Value::Number(_) | Value::Null => {}
                    Value::String(s) if s.parse::<f64>().is_ok() => {}
                    _ => {
                        return Some(ValidationError {
                            field: field.to_string(),
                            message: format!("{} must be numeric", field),
                            code: "numeric".to_string(),
                        })
                    }
                }
            }
        }
        "url" => {
            if let Some(Value::String(s)) = value {
                if !(s.is_empty() || s.starts_with("http://") || s.starts_with("https://")) {
                    return Some(ValidationError {
                        field: field.to_string(),
                        message: format!("{} must be a valid URL", field),
                        code: "url".to_string(),
                    });
                }
            }
        }
        "in" => {
            // `in:a,b,c` — value must be one of the listed options.
            if let (Some(Value::String(s)), Some(opts)) = (value, param) {
                if !opts.split(',').any(|o| o == s) {
                    return Some(ValidationError {
                        field: field.to_string(),
                        message: format!("{} must be one of: {}", field, opts),
                        code: "in".to_string(),
                    });
                }
            }
        }
        // `exists:<table>,<col>` needs a DB lookup; the sync validator can't do it (no pool),
        // so it is intentionally a no-op here. Referential integrity is the DB's job (FKs).
        _ => {}
    }
    None
}


#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn rules() -> HashMap<String, String> {
        // Mirrors CRM Deal: title required + bounded, value numeric.
        [
            ("title".to_string(), "required|min:1|max:255".to_string()),
            ("value".to_string(), "numeric|min:0".to_string()),
        ]
        .into_iter()
        .collect()
    }

    #[test]
    fn partial_update_omitting_required_passes() {
        // Patch only sets `value` — `title` (required) is absent and must NOT trip.
        let patch: HashMap<String, Value> = [("value".to_string(), json!(100))].into_iter().collect();
        assert!(validate_partial(&patch, &rules()).is_empty());
    }

    #[test]
    fn partial_update_with_invalid_present_field_fails() {
        // Patch sets `title` to empty — present, so `required|min:1` must fire.
        let patch: HashMap<String, Value> = [("title".to_string(), json!(""))].into_iter().collect();
        assert!(!validate_partial(&patch, &rules()).is_empty(), "empty title in patch must fail");
    }

    #[test]
    fn full_validate_still_requires_absent_field() {
        // Create-time `validate` (not partial) must still flag the missing required title.
        let data: HashMap<String, Value> = [("value".to_string(), json!(100))].into_iter().collect();
        assert!(!validate(&data, &rules()).is_empty(), "create must still require title");
    }
}

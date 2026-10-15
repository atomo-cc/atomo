//! Request security, shared by HTTP entry points and the data layer.
use crate::query::{WhereClause, WhereOperator};
use crate::schema::{AccessRule, FieldAttribute, FieldType, Model};
use anyhow::{bail, Result};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Clone, Debug, Default)]
pub struct Principal {
    pub user_id: Option<String>,
    pub role: Option<String>,
    pub tenant_id: Option<String>,
}

tokio::task_local! { static PRINCIPAL: Principal; }

pub async fn with_principal<F: std::future::Future>(principal: Principal, future: F) -> F::Output {
    crate::client::with_tenant_scope(
        principal.tenant_id.clone(),
        PRINCIPAL.scope(principal, future),
    )
    .await
}

pub fn current() -> Option<Principal> {
    PRINCIPAL.try_with(Clone::clone).ok()
}

/// A target header is never proof of membership. Until relational membership is
/// supported, only an explicitly privileged, unbound administrator can select one.
pub fn resolve_tenant(
    bound: Option<&str>,
    target: Option<&str>,
    admin: bool,
) -> Result<Option<String>> {
    match (bound, target) {
        (Some(own), Some(selected)) if own != selected => bail!("Access denied: tenant mismatch"),
        (Some(own), _) => Ok(Some(own.to_owned())),
        (None, Some(selected)) if admin && !selected.trim().is_empty() => {
            Ok(Some(selected.to_owned()))
        }
        (None, Some(_)) => bail!("Access denied: no authorized tenant binding"),
        (None, None) => Ok(None),
    }
}

fn constant(allow: bool) -> WhereClause {
    WhereClause {
        field: String::new(),
        operator: WhereOperator::Or(if allow {
            vec![WhereClause {
                field: String::new(),
                operator: WhereOperator::And(vec![]),
                value: Value::Null,
            }]
        } else {
            vec![]
        }),
        value: Value::Null,
    }
}

fn equals(field: &str, value: Option<&str>) -> WhereClause {
    match value {
        Some(value) => WhereClause {
            field: field.into(),
            operator: WhereOperator::Equals,
            value: Value::String(value.into()),
        },
        None => constant(false),
    }
}

fn rule_filter(rule: &AccessRule, p: &Principal) -> WhereClause {
    let operator = match rule {
        AccessRule::Owner(field) => return equals(field, p.user_id.as_deref()),
        AccessRule::SameTenant(field) => return equals(field, p.tenant_id.as_deref()),
        AccessRule::Boolean(_) => {
            return constant(rule.decide(p.role.as_deref()) == atomo_schema::AccessDecision::Allow)
        }
        AccessRule::And(rules) => {
            WhereOperator::And(rules.iter().map(|r| rule_filter(r, p)).collect())
        }
        AccessRule::Or(rules) => {
            WhereOperator::Or(rules.iter().map(|r| rule_filter(r, p)).collect())
        }
        // Legacy arbitrary query expressions must never silently grant access.
        AccessRule::Query(_) => return constant(false),
    };
    WhereClause {
        field: String::new(),
        operator,
        value: Value::Null,
    }
}

pub fn filter(model: &Model, operation: &str, clauses: &[WhereClause]) -> Result<Vec<WhereClause>> {
    let Some(p) = current() else {
        return Ok(clauses.to_vec());
    };
    let mut out = clauses.to_vec();
    if model.tenant_policy == atomo_schema::TenantPolicy::Global {
        out.push(WhereClause {
            field: "tenant_id".into(),
            operator: WhereOperator::IsNull,
            value: Value::Null,
        });
    } else {
        let tenant = p
            .tenant_id
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("Access denied: model requires tenant context"))?;
        out.push(equals("tenant_id", Some(tenant)));
    }
    if let Some(rule) = model.access.as_ref().and_then(|a| a.rule_for(operation)) {
        out.push(rule_filter(rule, &p));
    }
    Ok(out)
}

fn row_value<'a>(row: &'a HashMap<String, Value>, field: &str) -> Option<&'a Value> {
    row.get(field)
        .or_else(|| row.get(&crate::query::sql_builder::to_snake_case(field)))
}

fn matches(rule: &AccessRule, p: &Principal, row: &HashMap<String, Value>) -> bool {
    match rule {
        AccessRule::Owner(f) => p
            .user_id
            .as_deref()
            .is_some_and(|id| row_value(row, f).and_then(Value::as_str) == Some(id)),
        AccessRule::SameTenant(f) => p
            .tenant_id
            .as_deref()
            .is_some_and(|id| row_value(row, f).and_then(Value::as_str) == Some(id)),
        AccessRule::And(rs) => rs.iter().all(|r| matches(r, p, row)),
        AccessRule::Or(rs) => rs.iter().any(|r| matches(r, p, row)),
        AccessRule::Boolean(_) => {
            rule.decide(p.role.as_deref()) == atomo_schema::AccessDecision::Allow
        }
        AccessRule::Query(_) => false,
    }
}

pub fn row_allowed(model: &Model, p: &Principal, row: &HashMap<String, Value>) -> bool {
    let tenant = row_value(row, "tenantId").and_then(Value::as_str);
    let tenant_allowed = if model.tenant_policy == atomo_schema::TenantPolicy::Global {
        tenant.is_none()
    } else {
        p.tenant_id.is_some() && tenant == p.tenant_id.as_deref()
    };
    tenant_allowed
        && model
            .access
            .as_ref()
            .and_then(|a| a.rule_for("read"))
            .is_none_or(|r| matches(r, p, row))
}

pub fn check_row(model: &Model, operation: &str, row: &HashMap<String, Value>) -> Result<()> {
    if let Some(p) = current() {
        if let Some(rule) = model.access.as_ref().and_then(|a| a.rule_for(operation)) {
            if !matches(rule, &p, row) {
                bail!("Access denied: row policy for {operation}");
            }
        }
    }
    Ok(())
}

/// Runs after hooks and before SQL conversion. Source fields are immutable and
/// conflicting client input is rejected, including snake_case aliases.
pub fn prepare(
    model: &Model,
    data: &HashMap<String, Value>,
    update: bool,
) -> Result<HashMap<String, Value>> {
    let mut out = HashMap::new();
    for (key, value) in data {
        let canonical = model
            .fields
            .keys()
            .find(|f| {
                crate::query::sql_builder::to_snake_case(f)
                    == crate::query::sql_builder::to_snake_case(key)
            })
            .unwrap_or(key);
        if out.insert(canonical.clone(), value.clone()).is_some() {
            bail!("validation failed: duplicate field alias");
        }
    }
    for field in model.fields.values() {
        if let Some(value) = out.get(&field.name).filter(|v| !v.is_null()) {
            if field.field_type == FieldType::Number
                && !value
                    .as_i64()
                    .is_some_and(|n| (-9_007_199_254_740_991..=9_007_199_254_740_991).contains(&n))
            {
                bail!(
                    "validation failed: {} requires a safe JSON integer",
                    field.name
                );
            }
        }
        for attr in &field.attributes {
            if let FieldAttribute::FromAuth(source) = attr {
                // Raw system APIs outside request scope are explicitly privileged.
                if current().is_none() {
                    continue;
                }
                if update && out.contains_key(&field.name) {
                    bail!("Access denied: source field is immutable");
                }
                if !update {
                    let p = current().ok_or_else(|| {
                        anyhow::anyhow!("Authentication required for source field")
                    })?;
                    let value = match source.as_str() {
                        "userId" => p.user_id,
                        "tenantId" => p.tenant_id,
                        _ => bail!("Access denied: unsupported authentication source"),
                    }
                    .ok_or_else(|| {
                        anyhow::anyhow!("Access denied: missing authentication source")
                    })?;
                    if out
                        .get(&field.name)
                        .is_some_and(|v| v.as_str() != Some(&value))
                    {
                        bail!("Access denied: forged source field");
                    }
                    out.insert(field.name.clone(), Value::String(value));
                }
            }
        }
    }
    if let Some(p) = current() {
        let expected =
            if model.tenant_policy == atomo_schema::TenantPolicy::Global {
                Value::Null
            } else {
                Value::String(p.tenant_id.ok_or_else(|| {
                    anyhow::anyhow!("Access denied: model requires tenant context")
                })?)
            };
        let tenant_key = model
            .fields
            .keys()
            .find(|f| crate::query::sql_builder::to_snake_case(f) == "tenant_id")
            .cloned()
            .unwrap_or_else(|| "tenant_id".into());
        if let Some(value) = out.get(&tenant_key) {
            if update || value != &expected {
                bail!("Access denied: tenant field is immutable");
            }
        }
        if !update {
            out.insert(tenant_key, expected);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tenant_resolution_is_fail_closed() {
        assert_eq!(
            resolve_tenant(Some("a"), None, false).unwrap().as_deref(),
            Some("a")
        );
        assert!(resolve_tenant(Some("a"), Some("b"), true).is_err());
        assert!(resolve_tenant(None, Some("b"), false).is_err());
        assert_eq!(
            resolve_tenant(None, Some("b"), true).unwrap().as_deref(),
            Some("b")
        );
    }

    #[test]
    fn integers_are_validated_before_conversion_in_trusted_paths_too() {
        let schema = crate::schema::parse_typescript_schema("import {} from '@atomo-cc/schema'; export const Record = model('records', { fields: { id: text().id(), quota: number() } });").unwrap();
        let model = &schema.models["Record"];
        for value in [
            serde_json::json!(99.5),
            serde_json::json!(9_007_199_254_740_992_i64),
            serde_json::json!(u64::MAX),
            serde_json::json!("1"),
            serde_json::json!("NaN"),
            serde_json::json!("Infinity"),
        ] {
            assert!(prepare(model, &HashMap::from([("quota".into(), value)]), false).is_err());
        }
        for value in [
            serde_json::json!(-9_007_199_254_740_991_i64),
            serde_json::json!(9_007_199_254_740_991_i64),
            serde_json::json!(0),
            Value::Null,
        ] {
            assert!(prepare(model, &HashMap::from([("quota".into(), value)]), false).is_ok());
        }
    }

    #[test]
    fn owner_tenant_and_combinations_require_each_declared_condition() {
        let principal = Principal {
            user_id: Some("subject-a".into()),
            role: Some("viewer".into()),
            tenant_id: Some("tenant-a".into()),
        };
        let row = HashMap::from([
            ("owner_id".into(), serde_json::json!("subject-a")),
            ("scope_key".into(), serde_json::json!("tenant-a")),
        ]);
        let owner = AccessRule::Owner("ownerId".into());
        let tenant = AccessRule::SameTenant("scopeKey".into());
        let admin = AccessRule::Boolean("admin".into());
        assert!(matches(
            &AccessRule::And(vec![owner.clone(), tenant.clone()]),
            &principal,
            &row
        ));
        assert!(!matches(
            &AccessRule::And(vec![owner.clone(), admin.clone()]),
            &principal,
            &row
        ));
        assert!(matches(
            &AccessRule::Or(vec![owner.clone(), admin.clone()]),
            &principal,
            &row
        ));
        let foreign = Principal {
            user_id: Some("subject-b".into()),
            tenant_id: Some("tenant-b".into()),
            ..principal.clone()
        };
        assert!(!matches(&owner, &foreign, &row));
        assert!(!matches(&tenant, &foreign, &row));
        assert!(!matches(
            &AccessRule::Or(vec![owner, admin]),
            &foreign,
            &row
        ));
        assert!(!matches(&tenant, &Principal::default(), &row));
    }
}

use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;
use sqlx::PgPool;
use std::collections::HashMap;

/// A projection that materializes a read model from events
#[async_trait]
pub trait Projection: Send + Sync {
    fn name(&self) -> &str;
    fn source_model(&self) -> &str;
    async fn handle_event(&self, event_type: &str, data: &HashMap<String, Value>, pool: &PgPool) -> Result<()>;
    async fn rebuild(&self, pool: &PgPool) -> Result<()>;
}

/// Auto-generated table projection: maintains a denormalized read table
pub struct TableProjection {
    pub name: String,
    pub source_model: String,
    pub table_name: String,
    pub columns: Vec<String>,
}

impl TableProjection {
    pub fn new(source_model: &str, table_name: &str, columns: Vec<String>) -> Self {
        Self {
            name: format!("{}_projection", table_name),
            source_model: source_model.to_string(),
            table_name: table_name.to_string(),
            columns,
        }
    }
}

#[async_trait]
impl Projection for TableProjection {
    fn name(&self) -> &str { &self.name }
    fn source_model(&self) -> &str { &self.source_model }

    async fn handle_event(&self, event_type: &str, data: &HashMap<String, Value>, pool: &PgPool) -> Result<()> {
        match event_type {
            "Created" => {
                let cols: Vec<&str> = self.columns.iter().filter(|c| data.contains_key(*c)).map(|s| s.as_str()).collect();
                if cols.is_empty() { return Ok(()); }
                let placeholders: Vec<String> = (1..=cols.len()).map(|i| format!("${}", i)).collect();
                let sql = format!("INSERT INTO {} ({}) VALUES ({}) ON CONFLICT DO NOTHING", self.table_name, cols.join(", "), placeholders.join(", "));
                let mut query = sqlx::query(&sql);
                for col in &cols {
                    let val = data.get(*col).unwrap_or(&Value::Null);
                    query = query.bind(val.as_str().unwrap_or_default());
                }
                query.execute(pool).await?;
            }
            "Updated" => {
                if let Some(Value::String(id)) = data.get("id") {
                    let sets: Vec<String> = self.columns.iter()
                        .filter(|c| *c != "id" && data.contains_key(*c))
                        .enumerate()
                        .map(|(i, c)| format!("{} = ${}", c, i + 1))
                        .collect();
                    if sets.is_empty() { return Ok(()); }
                    let sql = format!("UPDATE {} SET {} WHERE id = ${}", self.table_name, sets.join(", "), sets.len() + 1);
                    let mut query = sqlx::query(&sql);
                    for col in self.columns.iter().filter(|c| *c != "id" && data.contains_key(*c)) {
                        let val = data.get(col).unwrap_or(&Value::Null);
                        query = query.bind(val.as_str().unwrap_or_default());
                    }
                    query = query.bind(id);
                    query.execute(pool).await?;
                }
            }
            "Deleted" => {
                if let Some(Value::String(id)) = data.get("id") {
                    sqlx::query(&format!("DELETE FROM {} WHERE id = $1", self.table_name))
                        .bind(id).execute(pool).await?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    async fn rebuild(&self, pool: &PgPool) -> Result<()> {
        sqlx::query(&format!("TRUNCATE TABLE {} CASCADE", self.table_name))
            .execute(pool).await?;
        Ok(())
    }
}

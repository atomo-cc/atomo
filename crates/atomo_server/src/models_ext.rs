//! Database-specific extensions for atomo_core models
//!
//! This module provides SQLx implementations for core models,
//! maintaining the separation between pure domain models and database specifics.

use atomo_core::ContentBlock;
use serde_json;
use sqlx;

/// Wrapper type for ContentBlock to implement database traits
/// This avoids the orphan rule while maintaining clean separation
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ContentBlockDb(pub ContentBlock);

impl From<ContentBlock> for ContentBlockDb {
    fn from(cb: ContentBlock) -> Self {
        ContentBlockDb(cb)
    }
}

impl From<ContentBlockDb> for ContentBlock {
    fn from(cbd: ContentBlockDb) -> Self {
        cbd.0
    }
}

// SQLx support for ContentBlockDb - store as JSON
impl sqlx::Type<sqlx::Postgres> for ContentBlockDb {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        <serde_json::Value as sqlx::Type<sqlx::Postgres>>::type_info()
    }
}

impl sqlx::postgres::PgHasArrayType for ContentBlockDb {
    fn array_type_info() -> sqlx::postgres::PgTypeInfo {
        <serde_json::Value as sqlx::postgres::PgHasArrayType>::array_type_info()
    }
}

impl sqlx::Encode<'_, sqlx::Postgres> for ContentBlockDb {
    fn encode_by_ref(&self, buf: &mut sqlx::postgres::PgArgumentBuffer) -> sqlx::encode::IsNull {
        match serde_json::to_value(&self.0) {
            Ok(json_value) => json_value.encode_by_ref(buf),
            Err(_) => sqlx::encode::IsNull::Yes,
        }
    }
}

impl sqlx::Decode<'_, sqlx::Postgres> for ContentBlockDb {
    fn decode(value: sqlx::postgres::PgValueRef<'_>) -> Result<Self, sqlx::error::BoxDynError> {
        let json_value = <serde_json::Value as sqlx::Decode<sqlx::Postgres>>::decode(value)?;
        let content_block: ContentBlock = serde_json::from_value(json_value)?;
        Ok(ContentBlockDb(content_block))
    }
}

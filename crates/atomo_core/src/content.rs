//! Content Management Core Interfaces
//!
//! This module provides the core content management types and interfaces
//! for the Atomo platform, including rich text content blocks.

use async_graphql::{Enum, InputObject, SimpleObject};
use serde::{Deserialize, Serialize};

/// Content block type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Enum)]
pub enum ContentBlockType {
    Text,
    RichText,
    Image,
    Video,
    File,
    Link,
    Code,
    Quote,
    List,
    Table,
}

// SQLx implementations for ContentBlockType
#[cfg(feature = "sqlx")]
impl sqlx::Type<sqlx::Postgres> for ContentBlockType {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        sqlx::postgres::PgTypeInfo::with_name("text")
    }
}

#[cfg(feature = "sqlx")]
impl<'r> sqlx::Decode<'r, sqlx::Postgres> for ContentBlockType {
    fn decode(value: sqlx::postgres::PgValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
        let s = <&str as sqlx::Decode<sqlx::Postgres>>::decode(value)?;
        match s {
            "Text" => Ok(Self::Text),
            "RichText" => Ok(Self::RichText),
            "Image" => Ok(Self::Image),
            "Video" => Ok(Self::Video),
            "File" => Ok(Self::File),
            "Link" => Ok(Self::Link),
            "Code" => Ok(Self::Code),
            "Quote" => Ok(Self::Quote),
            "List" => Ok(Self::List),
            "Table" => Ok(Self::Table),
            _ => Err(format!("Invalid ContentBlockType: {}", s).into()),
        }
    }
}

#[cfg(feature = "sqlx")]
impl<'q> sqlx::Encode<'q, sqlx::Postgres> for ContentBlockType {
    fn encode_by_ref(
        &self,
        buf: &mut sqlx::postgres::PgArgumentBuffer,
    ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
        let s = match self {
            Self::Text => "Text",
            Self::RichText => "RichText",
            Self::Image => "Image",
            Self::Video => "Video",
            Self::File => "File",
            Self::Link => "Link",
            Self::Code => "Code",
            Self::Quote => "Quote",
            Self::List => "List",
            Self::Table => "Table",
        };
        <&str as sqlx::Encode<'q, sqlx::Postgres>>::encode_by_ref(&s, buf)
    }
}

/// Core content block structure for rich text and media content
///
/// This represents a single content block that can contain text, media,
/// or structured data. Content blocks are the building blocks for
/// rich content throughout the platform.
#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject, InputObject)]
#[graphql(input_name = "ContentBlockInput")]
pub struct ContentBlock {
    /// Unique identifier for this content block
    pub id: String,

    /// Type of content block
    pub block_type: ContentBlockType,

    /// Raw content data (text, JSON, etc.)
    pub content: String,

    /// Optional metadata for the content block
    pub metadata: Option<String>,

    /// Display order within parent content
    pub order: i32,

    /// Whether this block is visible
    pub is_visible: bool,
}

// SQLx implementations for ContentBlock to work with PostgreSQL JSON fields
#[cfg(feature = "sqlx")]
impl sqlx::Type<sqlx::Postgres> for ContentBlock {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        sqlx::postgres::PgTypeInfo::with_name("jsonb")
    }
}

#[cfg(feature = "sqlx")]
impl<'r> sqlx::Decode<'r, sqlx::Postgres> for ContentBlock {
    fn decode(value: sqlx::postgres::PgValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
        let json = <serde_json::Value as sqlx::Decode<sqlx::Postgres>>::decode(value)?;
        Ok(serde_json::from_value(json)?)
    }
}

#[cfg(feature = "sqlx")]
impl<'q> sqlx::Encode<'q, sqlx::Postgres> for ContentBlock {
    fn encode_by_ref(
        &self,
        buf: &mut sqlx::postgres::PgArgumentBuffer,
    ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
        let json = serde_json::to_value(self)?;
        <serde_json::Value as sqlx::Encode<'q, sqlx::Postgres>>::encode_by_ref(&json, buf)
    }
}

// Array support for Vec<ContentBlock>
#[cfg(feature = "sqlx")]
impl sqlx::postgres::PgHasArrayType for ContentBlock {
    fn array_type_info() -> sqlx::postgres::PgTypeInfo {
        sqlx::postgres::PgTypeInfo::with_name("_jsonb")
    }
}

impl ContentBlock {
    /// Create a new text content block
    pub fn new_text(content: impl Into<String>) -> Self {
        Self {
            id: ulid::Ulid::new().to_string(),
            block_type: ContentBlockType::Text,
            content: content.into(),
            metadata: None,
            order: 0,
            is_visible: true,
        }
    }

    /// Create a new rich text content block
    pub fn new_rich_text(content: impl Into<String>) -> Self {
        Self {
            id: ulid::Ulid::new().to_string(),
            block_type: ContentBlockType::RichText,
            content: content.into(),
            metadata: None,
            order: 0,
            is_visible: true,
        }
    }

    /// Create a new image content block
    pub fn new_image(url: impl Into<String>, alt_text: Option<String>) -> Self {
        let metadata = alt_text.map(|alt| serde_json::json!({"alt": alt}).to_string());
        Self {
            id: ulid::Ulid::new().to_string(),
            block_type: ContentBlockType::Image,
            content: url.into(),
            metadata,
            order: 0,
            is_visible: true,
        }
    }
}

impl Default for ContentBlock {
    fn default() -> Self {
        Self::new_text("")
    }
}

// Specialized content block types for CRM and business applications

/// Paragraph content block for formatted text
#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject, InputObject)]
#[graphql(input_name = "ParagraphBlockInput")]
#[cfg_attr(feature = "sqlx", derive(sqlx::FromRow))]
pub struct ParagraphBlock {
    /// Unique identifier
    pub id: String,
    /// Text content
    pub text: String,
    /// Text formatting (bold, italic, etc.)
    pub formatting: Option<serde_json::Value>,
    /// Display order
    pub order: i32,
}

/// Call log content block for recording phone calls
#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject, InputObject)]
#[graphql(input_name = "CallLogBlockInput")]
#[cfg_attr(feature = "sqlx", derive(sqlx::FromRow))]
pub struct CallLogBlock {
    /// Unique identifier
    pub id: String,
    /// Call duration in seconds
    pub duration: Option<i32>,
    /// Call start time
    pub call_time: chrono::DateTime<chrono::Utc>,
    /// Call notes/summary
    pub notes: String,
    /// Call outcome or result
    pub outcome: Option<String>,
    /// Display order
    pub order: i32,
}

/// Meeting notes content block
#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject, InputObject)]
#[graphql(input_name = "MeetingNoteBlockInput")]
#[cfg_attr(feature = "sqlx", derive(sqlx::FromRow))]
pub struct MeetingNoteBlock {
    /// Unique identifier
    pub id: String,
    /// Meeting title
    pub title: String,
    /// Meeting date and time
    pub meeting_time: chrono::DateTime<chrono::Utc>,
    /// Meeting attendees
    pub attendees: Vec<String>,
    /// Meeting notes content
    pub notes: String,
    /// Action items from the meeting
    pub action_items: Vec<String>,
    /// Display order
    pub order: i32,
}

/// Task content block for action items and todos
#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject, InputObject)]
#[graphql(input_name = "TaskBlockInput")]
#[cfg_attr(feature = "sqlx", derive(sqlx::FromRow))]
pub struct TaskBlock {
    /// Unique identifier
    pub id: String,
    /// Task title
    pub title: String,
    /// Task description
    pub description: Option<String>,
    /// Task due date
    pub due_date: Option<chrono::DateTime<chrono::Utc>>,
    /// Task priority (1-5, where 1 is highest)
    pub priority: Option<i32>,
    /// Whether the task is completed
    pub is_completed: bool,
    /// Assigned to (user ID or name)
    pub assigned_to: Option<String>,
    /// Display order
    pub order: i32,
}

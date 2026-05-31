//! AI integration: pgvector embeddings storage and similarity search

use anyhow::Result;
use sqlx::PgPool;

pub struct EmbeddingStore {
    pool: PgPool,
}

impl EmbeddingStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Initialize pgvector extension and embeddings table
    pub async fn init(&self) -> Result<()> {
        sqlx::query("CREATE EXTENSION IF NOT EXISTS vector")
            .execute(&self.pool)
            .await?;
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS embeddings (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                model_name TEXT NOT NULL,
                entity_id TEXT NOT NULL,
                field_name TEXT NOT NULL,
                content TEXT NOT NULL,
                embedding vector(1536),
                metadata JSONB DEFAULT '{}',
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                UNIQUE(model_name, entity_id, field_name)
            )",
        )
        .execute(&self.pool)
        .await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_embeddings_model ON embeddings (model_name)")
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Store an embedding vector for a model field
    pub async fn store(
        &self,
        model_name: &str,
        entity_id: &str,
        field_name: &str,
        content: &str,
        embedding: &[f32],
    ) -> Result<()> {
        let embedding_str = format!(
            "[{}]",
            embedding
                .iter()
                .map(|f| f.to_string())
                .collect::<Vec<_>>()
                .join(",")
        );
        sqlx::query(
            "INSERT INTO embeddings (model_name, entity_id, field_name, content, embedding)
             VALUES ($1, $2, $3, $4, $5::vector)
             ON CONFLICT (model_name, entity_id, field_name)
             DO UPDATE SET content = $4, embedding = $5::vector, created_at = NOW()",
        )
        .bind(model_name)
        .bind(entity_id)
        .bind(field_name)
        .bind(content)
        .bind(&embedding_str)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Find similar records by vector similarity (cosine distance)
    pub async fn search_similar(
        &self,
        model_name: &str,
        query_embedding: &[f32],
        limit: i32,
    ) -> Result<Vec<SimilarityResult>> {
        let embedding_str = format!(
            "[{}]",
            query_embedding
                .iter()
                .map(|f| f.to_string())
                .collect::<Vec<_>>()
                .join(",")
        );
        let rows = sqlx::query_as::<_, SimilarityRow>(
            "SELECT entity_id, field_name, content, 1 - (embedding <=> $1::vector) as similarity
             FROM embeddings
             WHERE model_name = $2
             ORDER BY embedding <=> $1::vector
             LIMIT $3",
        )
        .bind(&embedding_str)
        .bind(model_name)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| SimilarityResult {
                entity_id: r.entity_id,
                field_name: r.field_name,
                content: r.content,
                similarity: r.similarity,
            })
            .collect())
    }

    /// Delete embeddings for an entity
    pub async fn delete(&self, model_name: &str, entity_id: &str) -> Result<()> {
        sqlx::query("DELETE FROM embeddings WHERE model_name = $1 AND entity_id = $2")
            .bind(model_name)
            .bind(entity_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

pub struct SimilarityResult {
    pub entity_id: String,
    pub field_name: String,
    pub content: String,
    pub similarity: f64,
}

#[derive(sqlx::FromRow)]
struct SimilarityRow {
    entity_id: String,
    field_name: String,
    content: String,
    similarity: f64,
}

//! Generic public-read redirect store — supports slug-based 301 redirects for
//! archived, renamed, or tombstoned public projections. The redirect table is
//! self-initializing at boot (idempotent CREATE TABLE) like the other operational
//! stores. Operators manage redirects through admin-only endpoints; the public
//! read handler checks them when a slug query returns no results.

use anyhow::Result;
use sqlx::PgPool;

#[derive(Clone)]
pub struct RedirectStore {
    pool: PgPool,
}

#[derive(Debug, serde::Serialize)]
pub struct Redirect {
    pub from_model: String,
    pub from_slug: String,
    pub to_slug: String,
    pub permanent: bool,
}

impl RedirectStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn init(&self) -> Result<()> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS public_read_redirects (
                id TEXT PRIMARY KEY DEFAULT gen_random_uuid(),
                model TEXT NOT NULL,
                from_slug TEXT NOT NULL,
                to_slug TEXT NOT NULL,
                permanent BOOLEAN NOT NULL DEFAULT true,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                UNIQUE (model, from_slug)
            )",
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Look up a redirect for a model+slug pair. Returns the target slug and
    /// whether it's permanent (301) or temporary (302).
    pub async fn lookup(&self, model: &str, from_slug: &str) -> Result<Option<Redirect>> {
        let row: Option<(String, String, String, bool)> = sqlx::query_as(
            "SELECT model, from_slug, to_slug, permanent FROM public_read_redirects
             WHERE model = $1 AND from_slug = $2",
        )
        .bind(model)
        .bind(from_slug)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|(from_model, from_slug, to_slug, permanent)| Redirect {
            from_model,
            from_slug,
            to_slug,
            permanent,
        }))
    }

    /// Create or update a redirect. Upserts on (model, from_slug).
    pub async fn upsert(
        &self,
        model: &str,
        from_slug: &str,
        to_slug: &str,
        permanent: bool,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO public_read_redirects (id, model, from_slug, to_slug, permanent)
             VALUES (gen_random_uuid(), $1, $2, $3, $4)
             ON CONFLICT (model, from_slug) DO UPDATE SET to_slug = $3, permanent = $4",
        )
        .bind(model)
        .bind(from_slug)
        .bind(to_slug)
        .bind(permanent)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Remove a redirect.
    pub async fn remove(&self, model: &str, from_slug: &str) -> Result<bool> {
        let result = sqlx::query(
            "DELETE FROM public_read_redirects WHERE model = $1 AND from_slug = $2",
        )
        .bind(model)
        .bind(from_slug)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// List all redirects for a model.
    pub async fn list(&self, model: &str) -> Result<Vec<Redirect>> {
        let rows: Vec<(String, String, String, bool)> = sqlx::query_as(
            "SELECT model, from_slug, to_slug, permanent FROM public_read_redirects
             WHERE model = $1 ORDER BY from_slug",
        )
        .bind(model)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|(from_model, from_slug, to_slug, permanent)| Redirect {
                from_model,
                from_slug,
                to_slug,
                permanent,
            })
            .collect())
    }
}

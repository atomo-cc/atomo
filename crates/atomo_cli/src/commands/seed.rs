use anyhow::Result;
use console::style;
use colored::Colorize;
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::path::{Path, PathBuf};

fn current_service_dir() -> Result<PathBuf> {
    let cwd = std::env::current_dir()?;
    if cwd.join("schema.ts").exists() {
        Ok(cwd)
    } else {
        anyhow::bail!("❌ Run this in a service directory (schema.ts not found)")
    }
}

async fn connect_db_from_env() -> Result<PgPool> {
    // Load .env in current working directory (service root)
    let _ = dotenv::dotenv();
    let url = std::env::var("DATABASE_URL")
        .map_err(|_| anyhow::anyhow!("DATABASE_URL not set; ensure .env exists in the service root"))?;
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&url)
        .await?;
    Ok(pool)
}

fn split_sql_statements(sql: &str) -> Vec<String> {
    // Simple splitter: split by ';' that end statements. Keeps things robust enough for our seed.sql format.
    sql.split(';')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| format!("{};", s))
        .collect()
}

pub async fn seed_command(seed_path: Option<String>) -> Result<()> {
    println!("{}", "🌱 Seeding database".bright_green().bold());

    // 1) Ensure we're in a service dir
    let service_dir = current_service_dir()?;
    println!("   📁 Service: {}", service_dir.display());

    // 2) Connect to DB using .env in service dir
    println!("   🔑 Loading .env and connecting to DB...");
    let pool = connect_db_from_env().await?;
    println!("   ✅ Connected to database");

    // 3) Locate seed.sql
    let path = seed_path
        .map(PathBuf::from)
        .unwrap_or(service_dir.join("seed.sql"));
    if !path.exists() {
        anyhow::bail!(
            "❌ Seed file not found: {} (provide --file <path> or create seed.sql)",
            path.display()
        );
    }
    println!("   📄 Using seed file: {}", path.display());
    let content = std::fs::read_to_string(&path)?;
    let statements = split_sql_statements(&content);
    if statements.is_empty() {
        println!("   ⚠️  Seed file is empty; nothing to run");
        return Ok(());
    }

    // 4) Execute in a transaction
    println!("   ▶️  Executing {} statements...", statements.len());
    let mut tx = pool.begin().await?;
    for (i, stmt) in statements.iter().enumerate() {
        if let Err(e) = sqlx::query(stmt).execute(&mut *tx).await {
            let _ = tx.rollback().await; // best-effort rollback
            anyhow::bail!("Seed failed on statement #{}: {}\nSQL: {}", i + 1, e, stmt);
        }
    }
    tx.commit().await?;

    println!("   {}", "✅ Seeding completed".bright_green());
    Ok(())
}

//! `atomo schema check` — parse the schema and report diagnostics.
//!
//! The parsers are deliberately permissive (regex/brace-walking, not a full TS
//! compiler): constructs they can't represent are dropped rather than erroring.
//! This command surfaces those drops — unregistered models, unrecognized
//! access/validation/relationships keys, reserved-word identifiers — so schema
//! mistakes are visible before boot instead of silently misbehaving at runtime.

use anyhow::{Context, Result};
use atomo_schema::{is_builder_dsl, parse_builder_dsl, TypeScriptParser};
use clap::Subcommand;
use colored::*;
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum SchemaCommands {
    /// Parse the schema and report diagnostics (dropped constructs, unrecognized
    /// keys, PostgreSQL reserved-word identifiers). Exits non-zero on parse errors.
    Check {
        /// Path to the schema file (defaults to ./schema.ts)
        #[arg(long)]
        schema: Option<PathBuf>,
    },
}

pub async fn schema_command(command: SchemaCommands) -> Result<()> {
    match command {
        SchemaCommands::Check { schema } => check_command(schema).await,
    }
}

async fn check_command(schema: Option<PathBuf>) -> Result<()> {
    let path = schema.unwrap_or_else(|| PathBuf::from("schema.ts"));
    if !path.exists() {
        anyhow::bail!("❌ schema file not found: {}", path.display());
    }
    let content = tokio::fs::read_to_string(&path)
        .await
        .with_context(|| format!("failed to read {}", path.display()))?;

    let parsed = if is_builder_dsl(&content) {
        parse_builder_dsl(&content)
    } else {
        TypeScriptParser::new().parse_schema(&content)
    }
    .with_context(|| format!("failed to parse {}", path.display()))?;

    println!(
        "   {} {} model(s), {} action(s)",
        "Parsed:".bright_green(),
        parsed.models.len(),
        parsed.actions.len()
    );

    if parsed.warnings.is_empty() {
        println!("   {} no schema warnings", "✓".bright_green());
    } else {
        println!(
            "   {} {} schema warning(s):",
            "⚠".bright_yellow(),
            parsed.warnings.len()
        );
        for w in &parsed.warnings {
            println!("     - {}", w.bright_yellow());
        }
    }
    Ok(())
}

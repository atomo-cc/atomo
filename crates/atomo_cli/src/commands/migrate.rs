use anyhow::{Result, Context};
use colored::*;
use console::style;
use std::fs;
use std::path::Path;
use sqlx::{PgPool, Row};
use std::collections::HashMap;
use atomo_schema::{TypeScriptParser, Field, Model};

pub async fn migrate_command(database_url: Option<String>) -> Result<()> {
    println!("🗄️  {}", style("Running database migrations...").cyan());
    
    // Load .env file if it exists
    let env_loaded = dotenv::dotenv().is_ok();
    if env_loaded {
        println!("   ✅ .env file loaded successfully");
    }
    
    let db_url = database_url.unwrap_or_else(|| {
        std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgresql://localhost/atomo_dev".to_string())
    });
    
    println!("   📊 Connecting to database...");
    
    // Connect to database
    let pool = PgPool::connect(&db_url).await
        .with_context(|| "Failed to connect to database. Make sure PostgreSQL is running and DATABASE_URL is correct")?;
    
    // Ensure migrations directory exists
    let migrations_dir = "migrations";
    if !Path::new(migrations_dir).exists() {
        fs::create_dir_all(migrations_dir)
            .with_context(|| "Failed to create migrations directory")?;
        println!("   📁 Created migrations directory");
    }
    
    // Create migration tracking table if it doesn't exist
    create_migration_table(&pool).await?;
    
    // Apply pending migrations
    apply_migrations(&pool, migrations_dir).await?;
    
    println!("   ✓ {}", "Migrations completed successfully!".bright_green());
    
    Ok(())
}

pub async fn generate_migration_command(name: Option<String>) -> Result<()> {
    println!("🔧 {}", style("Generating database migration...").cyan());
    
    // Find schema.ts file in current directory or subdirectories
    let schema_path = find_schema_file(".")
        .ok_or_else(|| anyhow::anyhow!("No schema.ts file found in current directory or subdirectories"))?;
    
    println!("   📄 Found schema: {}", schema_path.display());
    
    // Parse TypeScript schema
    let content = fs::read_to_string(&schema_path)
        .with_context(|| format!("Failed to read schema file: {}", schema_path.display()))?;
    
    let parser = TypeScriptParser::new();
    let models = parser.parse(&content)
        .with_context(|| "Failed to parse TypeScript schema")?;
    
    // Connect to database to get current schema
    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://localhost/atomo_dev".to_string());
    
    println!("   📊 Connecting to database to analyze current schema...");
    
    let pool = PgPool::connect(&db_url).await
        .with_context(|| "Failed to connect to database")?;
    
    // Get current database schema
    let current_tables = get_current_database_schema(&pool).await?;
    
    // Generate migration SQL
    let migration_sql = generate_migration_sql(&models, &current_tables)?;
    
    if migration_sql.trim().is_empty() {
        println!("   ✅ No schema changes detected - database is up to date");
        return Ok(());
    }
    
    // Create migration file
    let migrations_dir = "migrations";
    if !Path::new(migrations_dir).exists() {
        fs::create_dir_all(migrations_dir)
            .with_context(|| "Failed to create migrations directory")?;
    }
    
    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let migration_name = name.unwrap_or_else(|| "auto_generated".to_string());
    let filename = format!("{}_{}.sql", timestamp, migration_name);
    let filepath = Path::new(migrations_dir).join(&filename);
    
    fs::write(&filepath, migration_sql)
        .with_context(|| format!("Failed to write migration file: {}", filepath.display()))?;
    
    println!("   🚀 Generated migration: {}", filename.bright_green());
    println!("   📝 Review the migration file before running 'atomo migrate'");
    
    Ok(())
}

async fn create_migration_table(pool: &PgPool) -> Result<()> {
    let create_table_sql = r#"
        CREATE TABLE IF NOT EXISTS _atomo_migrations (
            id SERIAL PRIMARY KEY,
            filename VARCHAR(255) NOT NULL UNIQUE,
            checksum VARCHAR(64) NOT NULL,
            applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        );
    "#;
    
    sqlx::query(create_table_sql)
        .execute(pool)
        .await
        .with_context(|| "Failed to create migration tracking table")?;
    
    Ok(())
}

async fn apply_migrations(pool: &PgPool, migrations_dir: &str) -> Result<()> {
    // Get already applied migrations
    let applied_migrations: HashMap<String, String> = sqlx::query(
        "SELECT filename, checksum FROM _atomo_migrations ORDER BY id"
    )
    .fetch_all(pool)
    .await
    .with_context(|| "Failed to fetch applied migrations")?
    .into_iter()
    .map(|row| (row.get("filename"), row.get("checksum")))
    .collect();
    
    // Read migration files from directory
    let mut migration_files = Vec::new();
    if let Ok(entries) = fs::read_dir(migrations_dir) {
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("sql") {
                migration_files.push(path);
            }
        }
    }
    
    // Sort migration files by name (timestamp-based naming ensures chronological order)
    migration_files.sort();
    
    println!("   ⬆️  Applying migrations...");
    
    for migration_path in migration_files {
        let filename = migration_path.file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| anyhow::anyhow!("Invalid migration filename"))?;
        
        let migration_content = fs::read_to_string(&migration_path)
            .with_context(|| format!("Failed to read migration file: {}", filename))?;
        
        // Calculate checksum
        let checksum = format!("{:x}", md5::compute(&migration_content));
        
        // Check if migration already applied
        match applied_migrations.get(filename) {
            Some(existing_checksum) => {
                if existing_checksum != &checksum {
                    return Err(anyhow::anyhow!(
                        "Migration {} has been modified after being applied. Checksum mismatch.",
                        filename
                    ));
                }
                println!("   ⏭️  Skipping {} (already applied)", filename.dimmed());
                continue;
            }
            None => {
                println!("   🚀 Applying migration: {}", filename.bright_yellow());
                
                // Begin transaction
                let mut tx = pool.begin().await?;
                
                // Execute each statement separately
                let statements = parse_sql_statements(&migration_content);
                
                println!("   📋 Found {} SQL statements to execute", statements.len());
                
                for (i, statement) in statements.iter().enumerate() {
                    let statement_num = i + 1;
                    let preview = statement.lines().next().unwrap_or("").get(..50).unwrap_or(statement);
                    
                    println!("   📝 Executing statement {}: {}...", statement_num, preview);
                    
                    sqlx::query(statement)
                        .execute(&mut *tx)
                        .await
                        .with_context(|| format!("Failed to execute statement {}: {}", statement_num, statement))?;
                }
                
                // Record migration as applied
                sqlx::query(
                    "INSERT INTO _atomo_migrations (filename, checksum) VALUES ($1, $2)"
                )
                .bind(filename)
                .bind(&checksum)
                .execute(&mut *tx)
                .await
                .with_context(|| "Failed to record migration")?;
                
                // Commit transaction
                tx.commit().await
                    .with_context(|| "Failed to commit migration transaction")?;
                
                println!("   ✅ Applied: {}", filename.bright_green());
            }
        }
    }
    
    Ok(())
}

fn find_schema_file(dir: &str) -> Option<std::path::PathBuf> {
    use std::fs;
    use std::path::PathBuf;
    
    // Common schema file locations
    let candidates = [
        "schema.ts",
        "atomo/schema.ts", 
        "src/schema.ts",
        "atomo.config.ts", // backup option
    ];
    
    for candidate in &candidates {
        let path = PathBuf::from(dir).join(candidate);
        if path.exists() {
            return Some(path);
        }
    }
    
    // Search subdirectories for schema.ts
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
                let subdir_name = entry.file_name();
                if subdir_name == "node_modules" || subdir_name == "target" {
                    continue; // Skip these directories
                }
                
                if let Some(found) = find_schema_file(&entry.path().to_string_lossy()) {
                    return Some(found);
                }
            }
        }
    }
    
    None
}

#[derive(Debug)]
struct DatabaseTable {
    name: String,
    columns: HashMap<String, DatabaseColumn>,
}

#[derive(Debug)]
struct DatabaseColumn {
    name: String,
    data_type: String,
    is_nullable: bool,
    is_primary_key: bool,
}

async fn get_current_database_schema(pool: &PgPool) -> Result<HashMap<String, DatabaseTable>> {
    let mut tables = HashMap::new();
    
    // Query to get all tables and their columns
    let rows = sqlx::query(r#"
        SELECT 
            t.table_name,
            c.column_name,
            c.data_type,
            c.is_nullable,
            CASE WHEN kcu.column_name IS NOT NULL THEN true ELSE false END as is_primary_key
        FROM information_schema.tables t
        LEFT JOIN information_schema.columns c ON t.table_name = c.table_name
        LEFT JOIN information_schema.key_column_usage kcu ON c.table_name = kcu.table_name 
            AND c.column_name = kcu.column_name
        LEFT JOIN information_schema.table_constraints tc ON kcu.constraint_name = tc.constraint_name
            AND tc.constraint_type = 'PRIMARY KEY'
        WHERE t.table_schema = 'public' 
            AND t.table_type = 'BASE TABLE'
            AND t.table_name != '__atomo_migrations'
        ORDER BY t.table_name, c.ordinal_position
    "#).fetch_all(pool).await?;
    
    for row in rows {
        let table_name: String = row.get("table_name");
        let column_name: String = row.get("column_name");
        let data_type: String = row.get("data_type");
        let is_nullable: String = row.get("is_nullable");
        let is_primary_key: bool = row.get("is_primary_key");
        
        let table = tables.entry(table_name.clone()).or_insert_with(|| DatabaseTable {
            name: table_name.clone(),
            columns: HashMap::new(),
        });
        
        table.columns.insert(column_name.clone(), DatabaseColumn {
            name: column_name,
            data_type,
            is_nullable: is_nullable == "YES",
            is_primary_key,
        });
    }
    
    Ok(tables)
}

fn generate_migration_sql(
    models: &[Model], 
    current_tables: &HashMap<String, DatabaseTable>
) -> Result<String> {
    let mut sql = String::new();
    
    // Add migration header
    sql.push_str("-- Auto-generated migration\n");
    sql.push_str(&format!("-- Generated at: {}\n", chrono::Utc::now().to_rfc3339()));
    sql.push_str("\n");
    
    for model in models {
        let table_name = model.name.to_lowercase();
        
        if let Some(existing_table) = current_tables.get(&table_name) {
            // Table exists - check for column changes
            let table_changes = generate_table_alterations(model, existing_table)?;
            if !table_changes.is_empty() {
                sql.push_str(&table_changes);
                sql.push_str("\n");
            }
        } else {
            // New table - create it
            sql.push_str(&generate_create_table_sql(model)?);
            sql.push_str("\n");
        }
    }
    
    // Check for tables that need to be dropped (exist in DB but not in schema)
    for (table_name, _) in current_tables {
        let model_exists = models.iter().any(|m| m.name.to_lowercase() == *table_name);
        if !model_exists && !is_system_table(table_name) {
            sql.push_str(&format!("-- DROP TABLE IF EXISTS {}; -- Uncomment to drop unused table\n", table_name));
        }
    }
    
    Ok(sql)
}

fn generate_create_table_sql(model: &Model) -> Result<String> {
    let mut sql = String::new();
    let table_name = model.name.to_lowercase();
    
    sql.push_str(&format!("-- Create table for {}\n", model.name));
    sql.push_str(&format!("CREATE TABLE {} (\n", table_name));
    
    let mut columns = Vec::new();
    
    // Add id column if not present
    let has_id = model.fields.contains_key("id");
    if !has_id {
        columns.push("    id UUID PRIMARY KEY DEFAULT gen_random_uuid()".to_string());
    }
    
    // Add model fields (skip standard audit fields that will be added later)
    for (field_name, field) in &model.fields {
        // Skip standard Atomo fields that are added automatically
        if field_name == "createdAt" || field_name == "updatedAt" || field_name == "version" {
            continue;
        }
        let column_def = generate_column_definition(field_name, field)?;
        columns.push(format!("    {}", column_def));
    }
    
    // Add audit columns
    columns.push("    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()".to_string());
    columns.push("    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()".to_string());
    columns.push("    version INTEGER NOT NULL DEFAULT 1".to_string());
    
    sql.push_str(&columns.join(",\n"));
    sql.push_str("\n);\n");
    
    // Add indexes
    sql.push_str(&format!("CREATE INDEX idx_{}_created_at ON {} (created_at);\n", table_name, table_name));
    sql.push_str(&format!("CREATE INDEX idx_{}_updated_at ON {} (updated_at);\n", table_name, table_name));
    
    Ok(sql)
}

fn generate_table_alterations(model: &Model, existing_table: &DatabaseTable) -> Result<String> {
    let mut sql = String::new();
    let table_name = model.name.to_lowercase();
    
    // Check for new columns
    for (field_name, field) in &model.fields {
        if !existing_table.columns.contains_key(field_name) {
            sql.push_str(&format!(
                "ALTER TABLE {} ADD COLUMN {};\n", 
                table_name, 
                generate_column_definition(field_name, field)?
            ));
        }
    }
    
    // TODO: Check for modified columns (type changes, etc.)
    // TODO: Check for dropped columns
    
    Ok(sql)
}

fn generate_column_definition(field_name: &str, field: &Field) -> Result<String> {
    let pg_type = match &field.field_type {
        atomo_schema::FieldType::String => "TEXT",
        atomo_schema::FieldType::Number => "NUMERIC",
        atomo_schema::FieldType::Boolean => "BOOLEAN",
        atomo_schema::FieldType::Date => "DATE",
        atomo_schema::FieldType::DateTime => "TIMESTAMPTZ",
        atomo_schema::FieldType::EntityId => "UUID",
        atomo_schema::FieldType::Json => "JSONB",
        atomo_schema::FieldType::Reference(_) => "UUID",
        atomo_schema::FieldType::Array(_) => "JSONB",
        atomo_schema::FieldType::Blocks => "JSONB",
        atomo_schema::FieldType::Custom(_) => "TEXT", // Default fallback
    };
    
    // Convert camelCase to snake_case for database columns
    let db_field_name = camel_to_snake_case(field_name);
    let mut def = format!("{} {}", db_field_name, pg_type);
    
    if field_name == "id" {
        def.push_str(" PRIMARY KEY DEFAULT gen_random_uuid()");
    } else if !field.optional {
        def.push_str(" NOT NULL");
    }
    
    Ok(def)
}

// Convert camelCase to snake_case
fn camel_to_snake_case(input: &str) -> String {
    let mut result = String::new();
    let mut chars = input.chars().peekable();
    
    while let Some(ch) = chars.next() {
        if ch.is_uppercase() && !result.is_empty() {
            result.push('_');
        }
        result.push(ch.to_lowercase().next().unwrap());
    }
    
    result
}

fn is_system_table(table_name: &str) -> bool {
    matches!(table_name, "audit_log" | "__atomo_migrations")
}

fn parse_sql_statements(content: &str) -> Vec<String> {
    use regex::Regex;
    let mut statements = Vec::new();
    let mut current = String::new();
    let mut in_dollar: Option<String> = None; // e.g. Some("$$") or Some("$BODY$")

    // Regex to detect dollar-quote delimiters like $$ or $tag$
    let dq = Regex::new(r"\$[A-Za-z0-9_]*\$").unwrap();

    for original_line in content.lines() {
        let line = original_line;
        let trimmed = line.trim();

        // Skip pure comments, but keep comment lines inside dollar-quoted blocks to preserve bodies
        if in_dollar.is_none() && (trimmed.is_empty() || trimmed.starts_with("--")) {
            continue;
        }

        // Scan for entering/exiting dollar-quoted bodies
        // There may be multiple tokens on one line; toggle for each match
        if let Some(_) = dq.find(line) {
            // process all occurrences in order
            let mut start = 0;
            while let Some(m) = dq.find_at(line, start) {
                let token = m.as_str().to_string();
                if let Some(ref current_token) = in_dollar {
                    if &token == current_token {
                        // leaving
                        in_dollar = None;
                    }
                } else {
                    // entering a dollar-quoted block
                    in_dollar = Some(token);
                }
                start = m.end();
            }
        }

        current.push_str(line);
        current.push('\n');

        // Only split on semicolon when not inside a dollar-quoted body
        if in_dollar.is_none() && trimmed.ends_with(';') {
            let stmt = current.trim().to_string();
            if !stmt.is_empty() {
                statements.push(stmt);
            }
            current.clear();
        }
    }

    if !current.trim().is_empty() {
        statements.push(current.trim().to_string());
    }

    statements
}

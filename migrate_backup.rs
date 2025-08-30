use anyhow::{Result, Context};
use colored::*;
use console::style;
use std::fs;
use std::path::Path;
use sqlx::{PgPool, R                // Begin transaction
                let mut tx = pool.begin().await?;
                
                // Split by semicolon and execute each statement separately
                // This is a simpler approach that works for most cases
                let statements: Vec<String> = migration_content
                    .split(';')
                    .map(|stmt| stmt.trim().to_string())
                    .filter(|stmt| !stmt.is_empty() && !stmt.starts_with("--"))
                    .collect();
                
                println!("   📋 Found {} SQL statements to execute", statements.len());
                
                for (i, statement) in statements.iter().enumerate() {
                    if statement.trim().is_empty() {
                        continue;
                    }
                    
                    let statement_num = i + 1;
                    let preview = statement.lines().next().unwrap_or("").get(..60).unwrap_or(statement);
                    
                    println!("   📝 Executing statement {}: {}", statement_num, preview);
                    
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
        
        // Create initial migration for CRM models (Phase 1)
        create_initial_crm_migration(migrations_dir)?;
    }
    
    // Create migration tracking table if it doesn't exist
    create_migration_table(&pool).await?;
    
    // Apply pending migrations
    apply_migrations(&pool, migrations_dir).await?;
    
    println!("   ✓ {}", "Migrations completed successfully!".bright_green());
    
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
                
                // For now, let's use a simple approach: use sqlx::query! with raw SQL
                // Split properly by looking for statements that end with semicolon followed by newline
                
                // Begin transaction
                let mut tx = pool.begin().await?;
                
                // Execute the entire migration as a single statement
                // This handles PostgreSQL functions and complex statements better
                println!("   � Executing migration file as single transaction");
                
                sqlx::query(&migration_content)
                    .execute(&mut *tx)
                    .await
                    .with_context(|| format!("Failed to execute migration: {}", filename))?;
                
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

fn create_initial_crm_migration(migrations_dir: &str) -> Result<()> {
    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let migration_file = format!("{}/{}__initial_crm_schema.sql", migrations_dir, timestamp);
    
    let migration_sql = r#"-- Initial CRM schema for Atomo (Phase 1: CRUD + Audit Log)
-- This migration demonstrates "events-friendly CRUD" architecture

-- Core audit log table (Phase 1 approach)
CREATE TABLE audit_log (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_type VARCHAR(100) NOT NULL,
    entity_id UUID NOT NULL,
    stream_id UUID NOT NULL,
    operation VARCHAR(20) NOT NULL, -- CREATE, UPDATE, DELETE, READ
    old_data JSONB,
    new_data JSONB,
    user_id VARCHAR(255),
    ip_address INET,
    user_agent TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes for audit log
CREATE INDEX idx_audit_log_entity_type ON audit_log(entity_type);
CREATE INDEX idx_audit_log_entity_id ON audit_log(entity_id);
CREATE INDEX idx_audit_log_stream_id ON audit_log(stream_id);
CREATE INDEX idx_audit_log_created_at ON audit_log(created_at);
CREATE INDEX idx_audit_log_user_id ON audit_log(user_id);

-- Companies table
CREATE TABLE companies (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    stream_id UUID NOT NULL UNIQUE DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL,
    domain VARCHAR(255),
    website TEXT,
    address TEXT,
    phone VARCHAR(50),
    industry VARCHAR(100),
    employee_count INTEGER,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    version BIGINT NOT NULL DEFAULT 0
);

-- Indexes for companies
CREATE INDEX idx_companies_name ON companies(name);
CREATE INDEX idx_companies_domain ON companies(domain);
CREATE INDEX idx_companies_industry ON companies(industry);

-- Contacts table
CREATE TABLE contacts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    stream_id UUID NOT NULL UNIQUE DEFAULT gen_random_uuid(),
    first_name VARCHAR(100) NOT NULL,
    last_name VARCHAR(100) NOT NULL,
    email VARCHAR(255),
    phone VARCHAR(50),
    company_id UUID REFERENCES companies(id) ON DELETE SET NULL,
    title VARCHAR(100),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    version BIGINT NOT NULL DEFAULT 0
);

-- Indexes for contacts
CREATE INDEX idx_contacts_name ON contacts(first_name, last_name);
CREATE INDEX idx_contacts_email ON contacts(email);
CREATE UNIQUE INDEX idx_contacts_email_unique ON contacts(email) WHERE email IS NOT NULL;
CREATE INDEX idx_contacts_company_id ON contacts(company_id);

-- Deal stages and priorities (using enums for type safety)
CREATE TYPE deal_stage AS ENUM ('Lead', 'Qualified', 'Proposal', 'Negotiation', 'Won', 'Lost');
CREATE TYPE deal_priority AS ENUM ('Low', 'Medium', 'High', 'Critical');

-- Deals table
CREATE TABLE deals (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    stream_id UUID NOT NULL UNIQUE DEFAULT gen_random_uuid(),
    title VARCHAR(255) NOT NULL,
    description TEXT,
    amount DECIMAL(15,2),
    currency CHAR(3) NOT NULL DEFAULT 'USD',
    stage deal_stage NOT NULL DEFAULT 'Lead',
    priority deal_priority NOT NULL DEFAULT 'Medium',
    contact_id UUID REFERENCES contacts(id) ON DELETE SET NULL,
    company_id UUID REFERENCES companies(id) ON DELETE SET NULL,
    expected_close_date DATE,
    actual_close_date DATE,
    probability DECIMAL(3,2) NOT NULL DEFAULT 0.10 CHECK (probability >= 0.0 AND probability <= 1.0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    version BIGINT NOT NULL DEFAULT 0
);

-- Indexes for deals
CREATE INDEX idx_deals_title ON deals(title);
CREATE INDEX idx_deals_stage ON deals(stage);
CREATE INDEX idx_deals_priority ON deals(priority);
CREATE INDEX idx_deals_contact_id ON deals(contact_id);
CREATE INDEX idx_deals_company_id ON deals(company_id);
CREATE INDEX idx_deals_expected_close_date ON deals(expected_close_date);
CREATE INDEX idx_deals_amount ON deals(amount);

-- Triggers for automatic updated_at timestamps
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    NEW.version = OLD.version + 1;
    RETURN NEW;
END;
$$ language 'plpgsql';

CREATE TRIGGER update_companies_updated_at BEFORE UPDATE ON companies FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_contacts_updated_at BEFORE UPDATE ON contacts FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_deals_updated_at BEFORE UPDATE ON deals FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- Audit log triggers (Phase 1: CRUD with audit logging)
CREATE OR REPLACE FUNCTION audit_trigger_function()
RETURNS TRIGGER AS $$
BEGIN
    INSERT INTO audit_log (entity_type, entity_id, stream_id, operation, old_data, new_data)
    VALUES (
        TG_TABLE_NAME,
        COALESCE(NEW.id, OLD.id),
        COALESCE(NEW.stream_id, OLD.stream_id),
        TG_OP,
        CASE WHEN TG_OP = 'DELETE' THEN row_to_json(OLD) ELSE NULL END,
        CASE WHEN TG_OP IN ('INSERT', 'UPDATE') THEN row_to_json(NEW) ELSE NULL END
    );
    RETURN COALESCE(NEW, OLD);
END;
$$ language 'plpgsql';

-- Apply audit triggers to all CRM tables
CREATE TRIGGER companies_audit_trigger
    AFTER INSERT OR UPDATE OR DELETE ON companies
    FOR EACH ROW EXECUTE FUNCTION audit_trigger_function();

CREATE TRIGGER contacts_audit_trigger
    AFTER INSERT OR UPDATE OR DELETE ON contacts
    FOR EACH ROW EXECUTE FUNCTION audit_trigger_function();

CREATE TRIGGER deals_audit_trigger
    AFTER INSERT OR UPDATE OR DELETE ON deals
    FOR EACH ROW EXECUTE FUNCTION audit_trigger_function();

-- Sample data for development (optional)
INSERT INTO companies (name, domain, website, industry, employee_count) VALUES
('Acme Corporation', 'acme.com', 'https://acme.com', 'Technology', 500),
('Global Dynamics', 'globaldynamics.com', 'https://globaldynamics.com', 'Manufacturing', 1200);

INSERT INTO contacts (first_name, last_name, email, phone, company_id, title) VALUES
('John', 'Smith', 'john.smith@acme.com', '+1-555-0101', (SELECT id FROM companies WHERE name = 'Acme Corporation'), 'CTO'),
('Jane', 'Doe', 'jane.doe@globaldynamics.com', '+1-555-0102', (SELECT id FROM companies WHERE name = 'Global Dynamics'), 'VP Engineering');

INSERT INTO deals (title, description, amount, contact_id, company_id, stage, priority, expected_close_date, probability) VALUES
('Enterprise License Deal', 'Annual enterprise license for Atomo platform', 50000.00, 
 (SELECT id FROM contacts WHERE email = 'john.smith@acme.com'),
 (SELECT id FROM companies WHERE name = 'Acme Corporation'),
 'Proposal', 'High', '2025-12-31', 0.7),
('Consulting Services', 'Implementation consulting for Q1 2026', 25000.00,
 (SELECT id FROM contacts WHERE email = 'jane.doe@globaldynamics.com'), 
 (SELECT id FROM companies WHERE name = 'Global Dynamics'),
 'Negotiation', 'Medium', '2026-03-31', 0.8);
"#;

    fs::write(&migration_file, migration_sql)
        .with_context(|| format!("Failed to create migration file: {}", migration_file))?;
    
    println!("   ✅ Created initial CRM migration: {}", migration_file.bright_cyan());
    
    Ok(())
}

/// Parse SQL statements from migration content, handling PostgreSQL dollar-quoted strings
fn parse_sql_statements(content: &str) -> Result<Vec<String>> {
    let mut statements = Vec::new();
    let mut current_statement = String::new();
    let mut in_dollar_quote = false;
    let mut dollar_tag = String::new();
    let mut chars = content.chars().peekable();
    
    while let Some(ch) = chars.next() {
        current_statement.push(ch);
        
        if !in_dollar_quote {
            // Check for start of dollar quote
            if ch == '$' {
                let mut tag = String::new();
                let mut temp_chars = Vec::new();
                
                // Look for the dollar quote tag
                while let Some(&next_ch) = chars.peek() {
                    if next_ch == '$' {
                        chars.next(); // consume the closing $
                        current_statement.push(next_ch);
                        in_dollar_quote = true;
                        dollar_tag = tag;
                        break;
                    } else if next_ch.is_alphanumeric() || next_ch == '_' {
                        tag.push(next_ch);
                        temp_chars.push(next_ch);
                        chars.next();
                        current_statement.push(next_ch);
                    } else {
                        // Not a dollar quote, put chars back conceptually
                        break;
                    }
                }
            }
            // Check for statement end (semicolon followed by newline or end)
            else if ch == ';' {
                if let Some(&next_ch) = chars.peek() {
                    if next_ch == '\n' || next_ch == '\r' {
                        // Found end of statement
                        let stmt = current_statement.trim().to_string();
                        if !stmt.is_empty() && !stmt.starts_with("--") {
                            statements.push(clean_sql_statement(&stmt));
                        }
                        current_statement.clear();
                    }
                } else {
                    // End of file
                    let stmt = current_statement.trim().to_string();
                    if !stmt.is_empty() && !stmt.starts_with("--") {
                        statements.push(clean_sql_statement(&stmt));
                    }
                    break;
                }
            }
        } else {
            // We're inside a dollar quote, look for the end
            if ch == '$' && chars.peek() == Some(&'$') {
                // Check if this matches our opening tag
                let mut tag = String::new();
                let pos = chars.clone().position(|c| c == '$').unwrap_or(0);
                
                for _ in 0..pos {
                    if let Some(tag_ch) = chars.peek() {
                        if tag_ch.is_alphanumeric() || *tag_ch == '_' {
                            tag.push(chars.next().unwrap());
                            current_statement.push(tag.chars().last().unwrap());
                        } else {
                            break;
                        }
                    }
                }
                
                if chars.peek() == Some(&'$') && tag == dollar_tag {
                    chars.next(); // consume the closing $
                    current_statement.push('$');
                    in_dollar_quote = false;
                    dollar_tag.clear();
                }
            }
        }
    }
    
    // Handle the last statement if it doesn't end with semicolon
    let final_stmt = current_statement.trim().to_string();
    if !final_stmt.is_empty() && !final_stmt.starts_with("--") {
        statements.push(clean_sql_statement(&final_stmt));
    }
    
    Ok(statements)
}

/// Clean up a SQL statement by removing comments and extra whitespace
fn clean_sql_statement(stmt: &str) -> String {
    stmt.lines()
        .map(|line| {
            let line = line.trim();
            // Remove inline comments but preserve content before them
            if let Some(comment_pos) = line.find("--") {
                line[..comment_pos].trim()
            } else {
                line
            }
        })
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

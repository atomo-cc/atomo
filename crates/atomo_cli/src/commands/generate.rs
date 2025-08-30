use anyhow::{Result, Context};
use colored::*;
use console::style;
use std::path::{Path, PathBuf};
use std::fs;
use std::collections::HashMap;
use atomo_schema::{TypeScriptParser, CodeGenerator, Schema, GraphQLGenerator};

/// Smart schema file discovery
/// Tries to find schema.ts in multiple locations
fn find_schema_file(schema_path: &str) -> Result<PathBuf> {
    let current_dir = std::env::current_dir()?;
    
    // Strategy 1: Use provided path directly if it exists
    let direct_path = Path::new(schema_path);
    if direct_path.exists() {
        return Ok(direct_path.to_path_buf());
    }
    
    // Strategy 2: Look in current directory
    let current_schema = current_dir.join(schema_path);
    if current_schema.exists() {
        return Ok(current_schema);
    }
    
    // Strategy 3: Look for atomo/schema.ts in current directory
    let atomo_schema = current_dir.join("atomo").join("schema.ts");
    if atomo_schema.exists() {
        return Ok(atomo_schema);
    }
    
    // Strategy 4: Look for schema.ts in current directory
    let current_dir_schema = current_dir.join("schema.ts");
    if current_dir_schema.exists() {
        return Ok(current_dir_schema);
    }
    
    // Strategy 5: Look in services/*/schema.ts (for service-based projects)
    let services_dir = current_dir.join("services");
    if services_dir.exists() {
        for entry in fs::read_dir(services_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                let service_schema = entry.path().join("schema.ts");
                if service_schema.exists() {
                    return Ok(service_schema);
                }
            }
        }
    }
    
    // Strategy 6: Go up directories looking for atomo/schema.ts
    let mut parent = current_dir.parent();
    while let Some(dir) = parent {
        let candidate = dir.join("atomo").join("schema.ts");
        if candidate.exists() {
            return Ok(candidate);
        }
        parent = dir.parent();
    }
    
    anyhow::bail!(
        "❌ Schema file not found. Tried:\n   • {}\n   • {}\n   • {}\n   • ./schema.ts\n   • services/*/schema.ts\n   💡 Make sure you're in the project root or run 'atomo init' to create a new project", 
        schema_path,
        current_schema.display(),
        atomo_schema.display()
    );
}

pub async fn generate_command(schema_path: String) -> Result<()> {
    println!("⚙️  {}", style("Generating Rust code from schema...").cyan());
    
    // Smart schema file discovery
    let schema_file = find_schema_file(&schema_path)?;
    
    println!("   📄 Reading schema from: {}", schema_file.display().to_string().bright_green());
    
    println!("   📄 Reading schema from: {}", schema_path.bright_green());
    
    // Read and parse TypeScript schema
    let schema_content = std::fs::read_to_string(&schema_file)
        .with_context(|| format!("Failed to read schema file: {}", schema_file.display()))?;
    
    println!("   📊 Parsing TypeScript interfaces...");
    let parser = TypeScriptParser::new();
    let models = parser.parse(&schema_content)
        .with_context(|| "Failed to parse TypeScript schema")?;
    
    // Convert Vec<Model> to Schema
    let mut schema_models = HashMap::new();
    for model in models {
        schema_models.insert(model.name.clone(), model);
    }
    let schema = Schema { models: schema_models };
    
    println!("   🦀 Generating {} Rust models...", schema.models.len());
    for (_name, model) in &schema.models {
        // Skip block types in output
        if model.name.ends_with("Block") {
            continue;
        }
        
        println!("      ├─ {}", model.name.bright_yellow());
        
        // Show parsed fields for debugging
        for (field_name, field) in &model.fields {
            let optional_marker = if field.optional { "?" } else { "" };
            println!("      │  └─ {}{}: {:?}", field_name, optional_marker, field.field_type);
        }
    }
    
    // Generate Rust code
    println!("   🔧 Generating Rust code...");
    let rust_code = CodeGenerator::generate_rust_models(&schema)
        .with_context(|| "Failed to generate Rust code")?;
    
    // Determine output path - should be in the CRM crate
    let output_path = "generated/models.rs";
    
    // Ensure output directory exists
    if let Some(parent) = Path::new(output_path).parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create output directory: {:?}", parent))?;
    }
    
    // Write generated code
    fs::write(output_path, rust_code)
        .with_context(|| format!("Failed to write generated code to: {}", output_path))?;
    
    println!("   📝 Generated Rust code written to: {}", output_path.bright_cyan());
    
    // Generate TypeScript types for SDK
    println!("   🔧 Generating TypeScript types for SDK...");
    let typescript_code = CodeGenerator::generate_typescript_types(&schema)
        .with_context(|| "Failed to generate TypeScript types")?;
    
    // SDK output path
    let sdk_output_path = "packages/atomo-client-sdk/types.ts";
    
    // Ensure SDK output directory exists
    if let Some(parent) = Path::new(sdk_output_path).parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create SDK output directory: {:?}", parent))?;
    }
    
    // Write SDK types
    fs::write(sdk_output_path, typescript_code)
        .with_context(|| format!("Failed to write SDK types to: {}", sdk_output_path))?;
    
    println!("   📝 Generated TypeScript types written to: {}", sdk_output_path.bright_cyan());
    
    // Generate GraphQL schema
    println!("   🔧 Generating GraphQL schema...");
    let models = parser.parse(&schema_content)
        .with_context(|| "Failed to re-parse TypeScript schema for GraphQL")?;
    
    let graphql_generator = GraphQLGenerator::new();
    let graphql_schema = graphql_generator.generate_schema(&models)
        .with_context(|| "Failed to generate GraphQL schema")?;
    
    // GraphQL schema output path
    let graphql_output_path = "generated/schema.graphql";
    
    // Ensure output directory exists
    if let Some(parent) = Path::new(graphql_output_path).parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create GraphQL output directory: {:?}", parent))?;
    }
    
    // Write GraphQL schema
    fs::write(graphql_output_path, graphql_schema)
        .with_context(|| format!("Failed to write GraphQL schema to: {}", graphql_output_path))?;
    
    println!("   📝 Generated GraphQL schema written to: {}", graphql_output_path.bright_cyan());
    
    // Generate GraphQL resolvers
    println!("   🔧 Generating GraphQL resolvers...");
    let resolver_generator = atomo_schema::ResolverGenerator::new();
    let resolver_code = resolver_generator.generate_resolvers(&models)
        .with_context(|| "Failed to generate GraphQL resolvers")?;
    
    // Resolver output path  
    let resolver_output_path = "generated/resolvers.rs";
    
    // Ensure output directory exists
    if let Some(parent) = Path::new(resolver_output_path).parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create resolver output directory: {:?}", parent))?;
    }
    
    // Write resolver code
    fs::write(resolver_output_path, resolver_code)
        .with_context(|| format!("Failed to write resolvers to: {}", resolver_output_path))?;
    
    println!("   📝 Generated GraphQL resolvers written to: {}", resolver_output_path.bright_cyan());
    
    // Generate mod.rs to organize the generated modules
    println!("   🔧 Generating module organization...");
    let mod_content = r#"//! Auto-generated modules for service
//! DO NOT EDIT - This file is automatically generated

pub mod models;
pub mod resolvers;

// Re-export for convenience
pub use models::*;
pub use resolvers::*;
"#;
    
    let mod_output_path = "generated/mod.rs";
    fs::write(mod_output_path, mod_content)
        .with_context(|| format!("Failed to write mod.rs to: {}", mod_output_path))?;
    
    println!("   📝 Generated module organization written to: {}", mod_output_path.bright_cyan());
    
    println!("   ✓ {}", "Code generation completed successfully!".bright_green());
    println!("   💡 {}", "Next steps:".bright_blue());
    println!("      • Run `atomo migrate` to apply database changes");
    println!("      • Run `cargo build` to compile Rust models");
    println!("      • GraphQL schema is ready for API development");
    println!("      • SDK types are ready for frontend development");
    println!("      • Include generated_models.rs in your lib.rs file");
    
    Ok(())
}

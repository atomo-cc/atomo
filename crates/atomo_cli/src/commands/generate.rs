use anyhow::{Result, Context};
use colored::*;
use console::style;
use std::path::{Path, PathBuf};
use std::fs;
use std::collections::HashMap;
use atomo_schema::{TypeScriptParser, hasura_v2_type_generator::HasuraV2TypeGenerator, hasura_v2_resolver_generator::HasuraV2ResolverGenerator};

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
    
    println!("   🦀 Generating {} models for Hasura v2...", models.len());
    for model in &models {
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

    // Generate Hasura v2 GraphQL types
    println!("   🔧 Generating Hasura v2 GraphQL types...");
    let type_generator = HasuraV2TypeGenerator::new();
    let hasura_types_code = type_generator.generate_types(&models)
        .with_context(|| "Failed to generate Hasura v2 GraphQL types")?;

    // Generate Hasura v2 GraphQL resolvers
    println!("   🔧 Generating Hasura v2 GraphQL resolvers...");
    let hasura_resolver_generator = HasuraV2ResolverGenerator::new();
    let resolver_code = hasura_resolver_generator.generate_resolvers(&models)
        .with_context(|| "Failed to generate Hasura v2 GraphQL resolvers")?;
    
    // Determine output paths
    let types_output_path = "generated/models.rs";
    let resolver_output_path = "generated/resolvers.rs";
    
    // Ensure output directory exists
    if let Some(parent) = Path::new(types_output_path).parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create output directory: {:?}", parent))?;
    }

    // Write Hasura v2 types
    fs::write(types_output_path, &hasura_types_code)
        .with_context(|| format!("Failed to write Hasura v2 types to: {}", types_output_path))?;

    println!("   � Generated Hasura v2 types written to: {}", types_output_path.bright_cyan());

    // Write Hasura v2 resolvers
    fs::write(resolver_output_path, &resolver_code)
        .with_context(|| format!("Failed to write Hasura v2 resolvers to: {}", resolver_output_path))?;

    println!("   📝 Generated Hasura v2 resolvers written to: {}", resolver_output_path.bright_cyan());
    
    println!("   ✓ {}", "Hasura v2 code generation completed successfully!".bright_green());
    println!("   💡 {}", "Next steps:".bright_blue());
    println!("      • Run `atomo dev` to start development server");
    println!("      • Run `cargo build` to compile Rust models");
    println!("      • Hasura v2 GraphQL API is ready for development");
    println!("      • Access GraphQL Playground at http://localhost:3000/playground");
    
    Ok(())
}

//! Atomo CLI application
//!
//! This provides a command-line interface for Atomo operations.

use clap::{Parser, Subcommand};
use anyhow::Result;

#[derive(Parser)]
#[command(name = "atomo")]
#[command(about = "Atomo - The Content Core")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate Rust code from TypeScript schema
    Generate {
        /// Path to the schema file
        #[arg(long)]
        schema: String,
    },
    /// Start Atomo server
    Serve {
        /// Port to listen on
        #[arg(long, default_value = "3000")]
        port: u16,
        /// Schema file path
        #[arg(long)]
        schema: String,
    },
    /// Run database migrations
    Migrate {
        /// Schema file path
        #[arg(long)]
        schema: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    
    match cli.command {
        Commands::Generate { schema } => {
            // Use Hasura v2 generators for code generation
            println!("🔧 Generating Hasura v2 GraphQL code from schema: {}", schema);
            
            // Read and parse schema
            let schema_content = tokio::fs::read_to_string(&schema).await?;
            let parser = atomo_schema::TypeScriptParser::new();
            let models = parser.parse(&schema_content)?;
            
            // Generate Hasura v2 types
            let type_generator = atomo_schema::hasura_v2_type_generator::HasuraV2TypeGenerator::new();
            let types_code = type_generator.generate_types(&models)?;
            
            // Generate Hasura v2 resolvers
            let resolver_generator = atomo_schema::hasura_v2_resolver_generator::HasuraV2ResolverGenerator::new();
            let resolvers_code = resolver_generator.generate_resolvers(&models)?;
            
            // Write to output files
            let types_output_path = "generated/models.rs";
            let resolvers_output_path = "generated/resolvers.rs";
            
            // Ensure output directory exists
            tokio::fs::create_dir_all("generated").await?;
            
            tokio::fs::write(types_output_path, types_code).await?;
            tokio::fs::write(resolvers_output_path, resolvers_code).await?;
            
            println!("✅ Hasura v2 types generated at: {}", types_output_path);
            println!("✅ Hasura v2 resolvers generated at: {}", resolvers_output_path);
            Ok(())
        }
        Commands::Serve { port, schema } => {
            serve_command(port, schema).await
        }
        Commands::Migrate { schema } => {
            migrate_command(schema).await
        }
    }
}

async fn serve_command(port: u16, schema_path: String) -> Result<()> {
    use atomo::prelude::*;
    use axum::{Router, routing::post};
    use async_graphql_axum::{GraphQLRequest, GraphQLResponse};
    
    println!("🚀 Starting Atomo server on port {}", port);
    
    // Initialize Atomo from schema
    let atomo = Atomo::from_schema(&schema_path).await?;
    let schema = atomo.graphql_schema();
    
    // Create GraphQL handler
    let graphql_handler = |req: GraphQLRequest| async move {
        let resp = schema.execute(req.into_inner()).await;
        GraphQLResponse::from(resp)
    };
    
    // Build router
    let app = Router::new()
        .route("/graphql", post(graphql_handler));
    
    // Start server
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    println!("✅ GraphQL endpoint available at http://localhost:{}/graphql", port);
    
    axum::serve(listener, app).await?;
    
    Ok(())
}

async fn migrate_command(schema_path: String) -> Result<()> {
    use atomo::schema;
    
    println!("📊 Running migrations for schema: {}", schema_path);
    
    let schema_content = tokio::fs::read_to_string(&schema_path).await?;
    let schema = schema::parse_typescript_schema(&schema_content)?;
    let migrations = schema::generate_migrations(&schema)?;
    
    for migration in migrations {
        println!("Executing: {}", migration);
        // TODO: Execute against database
    }
    
    println!("✅ Migrations completed");
    Ok(())
}

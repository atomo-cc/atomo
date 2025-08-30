use clap::{Parser, Subcommand};
use colored::*;

mod commands;

use commands::*;

// Load environment variables from .env file
fn load_env() {
    if let Err(_) = dotenv::dotenv() {
        // .env file might not exist, which is fine
    }
}

#[derive(Parser)]
#[command(name = "atomo")]
#[command(about = "Atomo CLI - The command line interface for Atomo Content Core")]
#[command(version = "0.1.0")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new Atomo project
    Init {
        /// Project name
        name: String,
        /// Template to use (optional)
        #[arg(short, long)]
        template: Option<String>,
    },
    /// Generate Rust code from schema definitions
    Generate {
        /// Path to schema file
        #[arg(short, long, default_value = "atomo/schema.ts")]
        schema: String,
    },
    /// Run database migrations
    Migrate {
        /// Database URL
        #[arg(long)]
        database_url: Option<String>,
        /// Generate a new migration from schema changes
        #[arg(long)]
        generate: bool,
        /// Name for the generated migration
        #[arg(long)]
        name: Option<String>,
    },
    /// Generate client code for frontend
    Codegen {
        /// Output directory
        #[arg(short, long, default_value = "generated")]
        output: String,
    },
    /// Start development server
    Dev {
        /// Port to run on
        #[arg(short, long, default_value = "3000")]
        port: u16,
    },
    /// Build project for production
    Build,
    /// Deploy project to Atomo Cloud
    Deploy {
        /// Environment to deploy to
        #[arg(short, long, default_value = "production")]
        env: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load environment variables from .env file
    load_env();
    
    let cli = Cli::parse();

    println!("{}", "🚀 Atomo CLI".bright_blue().bold());
    println!("   {}", "The Next-Generation Content Core".dimmed());
    println!();

    match cli.command {
        Commands::Init { name, template } => {
            init_command(name, template).await?;
        }
        Commands::Generate { schema } => {
            generate_command(schema).await?;
        }
        Commands::Migrate { database_url, generate, name } => {
            if generate {
                generate_migration_command(name).await?;
            } else {
                migrate_command(database_url).await?;
            }
        }
        Commands::Codegen { output } => {
            codegen_command(output).await?;
        }
        Commands::Dev { port } => {
            dev_command(port).await?;
        }
        Commands::Build => {
            build_command().await?;
        }
        Commands::Deploy { env } => {
            deploy_command(env).await?;
        }
    }

    Ok(())
}

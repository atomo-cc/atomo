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
    /// Run database migrations (Note: run in service directory with .env file for DATABASE_URL)
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
    /// Start development server with auto code generation
    Dev {
        /// Port to run on
        #[arg(short, long, default_value = "3000")]
        port: u16,
    },
    /// Start development server with workspace context (faster for core development)
    WorkspaceDev {
        /// Port to run on
        #[arg(short, long, default_value = "3000")]
        port: u16,
        /// Optional path to service directory
        #[arg(long)]
        service_path: Option<std::path::PathBuf>,
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
        Commands::WorkspaceDev { port, service_path } => {
            workspace_dev_command(port, service_path).await?;
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

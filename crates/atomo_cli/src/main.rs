#![allow(dead_code, unused_assignments)]
// Pre-existing lints in dev/migrate tooling; revisit during a focused CLI cleanup.
#![allow(
    clippy::if_same_then_else,
    clippy::await_holding_lock,
    clippy::collapsible_match,
    clippy::collapsible_if
)]
use clap::{Parser, Subcommand};
use colored::*;

mod commands;

use commands::*;

// Load environment variables from .env file
fn load_env() {
    if dotenv::dotenv().is_err() {
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
    /// Seed database using SQL file (run in service dir; reads .env for DATABASE_URL)
    Seed {
        /// Optional path to seed SQL file (defaults to ./seed.sql)
        #[arg(long)]
        file: Option<String>,
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
        /// Use workspace context (fast rebuilds, Admin UI, platform integration)
        #[arg(long, default_value_t = false)]
        workspace: bool,
        /// Force isolated mode (no workspace), useful to test service-only flow inside the monorepo
        #[arg(long, default_value_t = false)]
        isolated: bool,
        /// Treat schema validation as errors (otherwise warnings in workspace)
        #[arg(long, default_value_t = false)]
        strict_schema: bool,
        /// Run schema validations (enabled by default; flag explicitly enables and can be used for clarity)
        #[arg(long, default_value_t = false)]
        verify_schema: bool,
        /// Optional path to service directory (workspace mode)
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
    /// Run service tests
    Test {
        /// Path to service directory
        #[arg(long)]
        service_path: Option<String>,
        /// Filter test names
        #[arg(short, long)]
        filter: Option<String>,
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
        Commands::Migrate {
            database_url,
            generate,
            name,
        } => {
            if generate {
                generate_migration_command(name).await?;
            } else {
                migrate_command(database_url).await?;
            }
        }
        Commands::Codegen { output } => {
            codegen_command(output).await?;
        }
        Commands::Dev {
            port,
            workspace,
            isolated,
            strict_schema,
            verify_schema,
            service_path,
        } => {
            dev_command(
                port,
                workspace,
                isolated,
                service_path,
                strict_schema,
                verify_schema,
            )
            .await?;
        }
        Commands::Build => {
            build_command().await?;
        }
        Commands::Deploy { env } => {
            deploy_command(env).await?;
        }
        Commands::Seed { file } => {
            seed_command(file).await?;
        }
        Commands::Test {
            service_path,
            filter,
        } => {
            test_command(service_path, filter).await?;
        }
    }

    Ok(())
}

//! Atomo Server binary
//!
//! A standalone server that uses the Atomo library to provide
//! a GraphQL API for your schema.

use anyhow::Result;
use atomo_server::{AtomoServer, ServerConfig};
use clap::{Arg, Command};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command line arguments
    let matches = Command::new("atomo-server")
        .version("1.0.0")
        .about("Atomo Platform Server - Serves configured business services")
        .arg(
            Arg::new("config-dir")
                .long("config-dir")
                .value_name("DIR")
                .help("Path to service configuration directory")
                .value_parser(clap::value_parser!(PathBuf)),
        )
        .arg(
            Arg::new("port")
                .long("port")
                .short('p')
                .value_name("PORT")
                .help("Port to run the server on")
                .value_parser(clap::value_parser!(u16)),
        )
        .get_matches();

    // If config dir is provided, try to load .env from that directory first
    if let Some(config_dir) = matches.get_one::<PathBuf>("config-dir") {
        let env_path = config_dir.join(".env");
        if env_path.exists() {
            if let Err(e) = dotenvy::from_path(&env_path) {
                eprintln!(
                    "Warning: Failed to load .env from {}: {}",
                    env_path.display(),
                    e
                );
            } else {
                println!("Loaded environment from: {}", env_path.display());
            }
        }
    }

    // Also try to load .env from current directory as fallback
    if dotenvy::dotenv().is_err() {
        // It's okay if there's no .env file in current directory
    }

    // Load configuration from environment or use defaults
    let mut config = ServerConfig::from_env();

    // Override with command line arguments
    if let Some(config_dir) = matches.get_one::<PathBuf>("config-dir") {
        config.service_config_dir = Some(config_dir.clone());

        // If config dir is provided, look for schema.ts in that directory
        let schema_path = config_dir.join("schema.ts");
        if schema_path.exists() {
            config.schema_path = schema_path.to_string_lossy().to_string();
        }
    }

    if let Some(port) = matches.get_one::<u16>("port") {
        config.port = *port;
    }

    // Create and run the server
    let server = AtomoServer::new(config).await?;
    server.run().await?;

    Ok(())
}

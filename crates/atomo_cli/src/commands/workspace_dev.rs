use anyhow::Result;
use colored::*;
use console::style;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::Command as TokioCommand;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::sync::mpsc;
use std::time::Duration;

/// Workspace-level development mode
/// 
/// Instead of creating isolated runtime, this mode runs the service directly
/// in the workspace context, allowing for fast incremental compilation
/// when atomo core changes.
pub async fn workspace_dev_command(port: u16, service_path: Option<PathBuf>) -> Result<()> {
    println!("🚀 {}", style("Starting Atomo workspace development server...").cyan());
    
    // Step 1: Detect or use provided service directory
    let service_dir = match service_path {
        Some(path) => path,
        None => detect_service_in_workspace().await?,
    };
    
    println!("   📋 Using service: {}", service_dir.display().to_string().bright_yellow());
    
    // Step 2: Setup workspace development environment
    setup_workspace_dev_environment(&service_dir).await?;
    
    // Step 3: Run service with workspace context and hot reload
    run_service_with_workspace_context(&service_dir, port).await?;
    
    Ok(())
}

/// Detect service directory in current workspace
async fn detect_service_in_workspace() -> Result<PathBuf> {
    let current_dir = std::env::current_dir()?;
    
    // Check if we're in a service directory
    if current_dir.join("schema.ts").exists() {
        return Ok(current_dir);
    }
    
    // Check if we're in workspace root and look for services
    let services_dir = current_dir.join("services");
    if services_dir.exists() {
        let mut entries = tokio::fs::read_dir(&services_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            if entry.file_type().await?.is_dir() {
                let service_path = entry.path();
                if service_path.join("schema.ts").exists() {
                    println!("   🔍 Found service: {}", service_path.display());
                    return Ok(service_path);
                }
            }
        }
    }
    
    anyhow::bail!("❌ No service found. Run from a service directory or workspace root with services/");
}

/// Setup workspace development environment
async fn setup_workspace_dev_environment(service_dir: &Path) -> Result<()> {
    println!("   ⚙️  Setting up workspace development environment...");
    
    // Create development configuration in the service directory
    let dev_config_path = service_dir.join(".atomo").join("workspace-dev.toml");
    tokio::fs::create_dir_all(service_dir.join(".atomo")).await?;
    
    let dev_config = format!(r#"# Workspace Development Configuration
[workspace]
# Use workspace context for faster compilation
use_workspace_context = true
# Hot reload configuration
hot_reload = true
# Development server configuration
dev_server_port = 3000

[compilation]
# Use workspace target directory for incremental compilation
use_workspace_target = true
# Development profile
profile = "dev"
# Feature flags for development
features = ["dev-mode", "hot-reload"]

[paths]
# Atomo core libraries (relative to workspace root)
atomo_core = "../../crates/atomo_core"
atomo_schema = "../../crates/atomo_schema"
atomo_server = "../../crates/atomo_server"
"#);
    
    tokio::fs::write(&dev_config_path, dev_config).await?;
    println!("   📝 Created workspace development configuration");
    
    Ok(())
}

/// Run service with workspace context for fast incremental compilation
async fn run_service_with_workspace_context(service_dir: &Path, port: u16) -> Result<()> {
    println!("   🔧 Starting service with workspace context...");
    
    // Step 1: Setup file watching for core libraries AND service schema
    let (tx, rx) = mpsc::channel();
    let mut watcher = RecommendedWatcher::new(tx, notify::Config::default())?;
    
    // Watch atomo core libraries
    let workspace_root = find_workspace_root(service_dir)?;
    
    // Check if the directories exist before watching
    let core_src = workspace_root.join("crates/atomo_core/src");
    let schema_src = workspace_root.join("crates/atomo_schema/src");
    let server_src = workspace_root.join("crates/atomo_server/src");
    let cli_src = workspace_root.join("crates/atomo_cli/src");
    let atomo_src = workspace_root.join("crates/atomo/src");
    let service_schema = service_dir.join("schema.ts");
    
    if !core_src.exists() {
        anyhow::bail!("❌ Atomo core source directory not found: {}", core_src.display());
    }
    if !schema_src.exists() {
        anyhow::bail!("❌ Atomo schema source directory not found: {}", schema_src.display());
    }
    if !server_src.exists() {
        anyhow::bail!("❌ Atomo server source directory not found: {}", server_src.display());
    }
    if !service_schema.exists() {
        anyhow::bail!("❌ Service schema.ts not found: {}", service_schema.display());
    }
    
    println!("   📂 Watching directories:");
    println!("      ├─ Core: {}", core_src.display().to_string().dimmed());
    println!("      ├─ Schema: {}", schema_src.display().to_string().dimmed());
    println!("      ├─ Server: {}", server_src.display().to_string().dimmed());
    if cli_src.exists() {
        println!("      ├─ CLI: {}", cli_src.display().to_string().dimmed());
    }
    if atomo_src.exists() {
        println!("      ├─ Atomo: {}", atomo_src.display().to_string().dimmed());
    }
    println!("      └─ Service Schema: {}", service_schema.display().to_string().dimmed());
    
    watcher.watch(&core_src, RecursiveMode::Recursive)?;
    watcher.watch(&schema_src, RecursiveMode::Recursive)?;
    watcher.watch(&server_src, RecursiveMode::Recursive)?;
    if cli_src.exists() {
        watcher.watch(&cli_src, RecursiveMode::Recursive)?;
    }
    if atomo_src.exists() {
        watcher.watch(&atomo_src, RecursiveMode::Recursive)?;
    }
    watcher.watch(&service_schema, RecursiveMode::NonRecursive)?;
    
    println!("   👀 File watching setup complete");
    
    // Step 2: Build and run service with workspace dependencies
    build_and_run_workspace_service(service_dir, port, rx).await?;
    
    Ok(())
}

/// Find workspace root directory
fn find_workspace_root(service_dir: &Path) -> Result<PathBuf> {
    let mut current = service_dir;
    for _ in 0..10 {  // Max 10 levels up
        if current.join("Cargo.toml").exists() {
            // Check if it's a workspace Cargo.toml
            let cargo_content = std::fs::read_to_string(current.join("Cargo.toml"))?;
            if cargo_content.contains("[workspace]") && cargo_content.contains("members") {
                return Ok(current.to_path_buf());
            }
        }
        if let Some(parent) = current.parent() {
            current = parent;
        } else {
            break;
        }
    }
    anyhow::bail!("Could not find workspace root from {}", service_dir.display());
}

/// Rebuild CLI when CLI files change
async fn rebuild_cli(workspace_root: &Path) -> Result<()> {
    println!("   📦 Building atomo-cli...");
    
    let output = TokioCommand::new("cargo")
        .arg("build")
        .arg("--bin")
        .arg("atomo-cli")
        .current_dir(workspace_root)
        .output()
        .await?;
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("CLI build failed:\n{}", stderr);
    }
    
    println!("   ✅ CLI rebuilt successfully");
    Ok(())
}

/// Build and run service with workspace incremental compilation
async fn build_and_run_workspace_service(
    service_dir: &Path,
    port: u16,
    rx: mpsc::Receiver<notify::Result<notify::Event>>
) -> Result<()> {
    let workspace_root = find_workspace_root(service_dir)?;
    
    // Step 1: Generate service-specific runtime code
    generate_service_runtime_code(service_dir, port).await?;
    
    // Step 2: Create workspace-aware Cargo.toml for the service
    create_workspace_service_cargo_toml(service_dir, &workspace_root).await?;
    
    // Step 3: Initial compilation with workspace context
    println!("   🔧 Initial compilation with workspace context...");
    compile_workspace_service(&workspace_root, service_dir).await?;
    
    // Step 4: Start service with hot reload
    start_workspace_service_with_hot_reload(&workspace_root, service_dir, port, rx).await?;
    
    Ok(())
}

/// Generate minimal runtime code for the service
async fn generate_service_runtime_code(service_dir: &Path, port: u16) -> Result<()> {
    use atomo_schema::{TypeScriptParser, HasuraV2TypeGenerator, HasuraV2ResolverGenerator};
    
    let schema_path = service_dir.join("schema.ts");
    
    // Create hidden temporary runtime directory (following Atomo architecture principles)
    let runtime_dir = service_dir.join(".atomo/runtime");
    let src_dir = runtime_dir.join("src");
    tokio::fs::create_dir_all(&src_dir).await?;
    
    // Parse schema
    let schema_content = tokio::fs::read_to_string(&schema_path).await?;
    let parser = TypeScriptParser::new();
    let models = parser.parse(&schema_content)?;
    
    // Generate models and resolvers in the temporary directory
    let type_generator = HasuraV2TypeGenerator::new();
    let resolver_generator = HasuraV2ResolverGenerator::new();
    
    let models_code = type_generator.generate_types(&models)?;
    let resolvers_code = resolver_generator.generate_resolvers(&models)?;
    
    tokio::fs::write(src_dir.join("models.rs"), models_code).await?;
    tokio::fs::write(src_dir.join("resolvers.rs"), resolvers_code).await?;

    // Copy .env file if it exists
    let env_file = service_dir.join(".env");
    if env_file.exists() {
        let env_content = tokio::fs::read_to_string(&env_file).await?;
        tokio::fs::write(runtime_dir.join(".env"), env_content).await?;
        println!("   📋 Copied .env configuration to runtime");
    }
    
    // Generate main.rs using the same template as dev command for consistency
    let main_rs = format!(r#"//! {} Service Runtime - Workspace Development Mode
//! 
//! 这是一个由 Atomo CLI 自动生成的服务运行时。
//! 使用工作区上下文进行快速增量编译开发。

use anyhow::Result;
use axum::{{
    extract::State,
    response::{{Html, IntoResponse}},
    routing::{{get, post}},
    Router,
}};
use async_graphql::{{EmptySubscription, Schema}};
use async_graphql_axum::{{GraphQLRequest, GraphQLResponse}};
use sqlx::postgres::PgPoolOptions;
use tower_http::cors::CorsLayer;
use tracing::info;

mod models;
mod resolvers;

use resolvers::*;

// GraphQL Schema 类型别名
type ServiceSchema = Schema<Query, Mutation, EmptySubscription>;

#[tokio::main]
async fn main() -> Result<()> {{
    // 初始化日志
    tracing_subscriber::fmt::init();
    
    // 加载环境变量
    dotenvy::dotenv().ok();
    
    info!("🚀 Starting {} Service Runtime (Workspace Dev Mode)");
    
    // 连接数据库
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://localhost/atomo_dev".to_string());
    
    info!("📊 Connecting to database...");
    let db_pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await?;
    
    // 构建 GraphQL Schema
    info!("🔧 Building GraphQL schema with generated resolvers...");
    let schema = Schema::build(Query, Mutation, EmptySubscription)
        .data(db_pool.clone())
        .finish();
    
    // 创建应用路由
    let app = Router::new()
        .route("/", get(health_check))
        .route("/health", get(health_check))
        .route("/graphql", post(graphql_handler))
        .route("/playground", get(graphql_playground))
        .layer(CorsLayer::permissive())
        .with_state(AppState {{ schema }});
    
    // 启动服务器
    let port = {};
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{{}}", port)).await?;
    
    info!("✅ {} Service running on http://localhost:{{}}", port);
    info!("📊 GraphQL endpoint: http://localhost:{{}}/graphql", port);
    info!("🎮 GraphQL playground: http://localhost:{{}}/playground", port);
    info!("🔥 Hot reload enabled - workspace development mode");
    
    axum::serve(listener, app).await?;
    
    Ok(())
}}

#[derive(Clone)]
struct AppState {{
    schema: ServiceSchema,
}}

async fn health_check() -> impl IntoResponse {{
    "{} Service is healthy! 🚀 (Workspace Dev)"
}}

async fn graphql_handler(
    State(state): State<AppState>,
    req: GraphQLRequest,
) -> GraphQLResponse {{
    state.schema.execute(req.into_inner()).await.into()
}}

async fn graphql_playground() -> impl IntoResponse {{
    Html(async_graphql::http::GraphiQLSource::build()
        .endpoint("/graphql")
        .finish())
}}
"#, 
        service_dir.file_name().and_then(|n| n.to_str()).unwrap_or("workspace-service"),
        service_dir.file_name().and_then(|n| n.to_str()).unwrap_or("workspace-service"),
        port,
        service_dir.file_name().and_then(|n| n.to_str()).unwrap_or("workspace-service"),
        service_dir.file_name().and_then(|n| n.to_str()).unwrap_or("workspace-service")
    );
    
    tokio::fs::write(src_dir.join("main.rs"), main_rs).await?;
    
    println!("   📝 Generated service runtime code");
    Ok(())
}

/// Create Cargo.toml that uses workspace dependencies
async fn create_workspace_service_cargo_toml(service_dir: &Path, workspace_root: &Path) -> Result<()> {
    let service_name = service_dir.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("service");
    
    // Create Cargo.toml in the hidden temporary runtime directory
    let runtime_dir = service_dir.join(".atomo/runtime");
    let cargo_toml_path = runtime_dir.join("Cargo.toml");
    
    let cargo_toml = format!(r#"[package]
name = "{service_name}-workspace-dev"
version = "0.1.0"
edition = "2021"

# Ensure this package doesn't participate in the workspace
[workspace]

[[bin]]
name = "service"
path = "src/main.rs"

[dependencies]
# Local Atomo dependencies - these will use incremental compilation
atomo_core = {{ path = "{workspace_path}/crates/atomo_core" }}
atomo_schema = {{ path = "{workspace_path}/crates/atomo_schema" }}
atomo_server = {{ path = "{workspace_path}/crates/atomo_server" }}

# External dependencies - use explicit versions instead of workspace
tokio = {{ version = "1.0", features = ["full"] }}
anyhow = "1.0"
axum = {{ version = "0.8", features = ["macros"] }}
async-graphql = {{ version = "7.0", features = ["uuid", "chrono", "dataloader"] }}
async-graphql-axum = "7.0"
tower-http = {{ version = "0.6", features = ["cors", "trace"] }}
tracing = "0.1"
tracing-subscriber = {{ version = "0.3", features = ["env-filter"] }}
serde = {{ version = "1.0", features = ["derive"] }}
serde_json = "1.0"
chrono = {{ version = "0.4", features = ["serde"] }}
uuid = {{ version = "1.0", features = ["v4", "serde"] }}
bigdecimal = "0.4"
dotenvy = "0.15"
sqlx = {{ version = "0.7", features = ["runtime-tokio-rustls", "postgres", "chrono", "uuid", "bigdecimal"] }}

[profile.dev]
# Optimized for fast incremental compilation
incremental = true
debug = 1
opt-level = 0
overflow-checks = false
lto = false
codegen-units = 256
"#, 
    service_name = service_name,
    workspace_path = workspace_root.canonicalize()?.display()
);
    
    tokio::fs::write(cargo_toml_path, cargo_toml).await?;
    println!("   📝 Created workspace-aware Cargo.toml");
    
    Ok(())
}

/// Compile service with workspace context for incremental compilation
async fn compile_workspace_service(workspace_root: &Path, service_dir: &Path) -> Result<()> {
    println!("   🔧 Compiling with workspace incremental compilation...");
    
    let mut compile_cmd = TokioCommand::new("cargo")
        .arg("build")
        .arg("--manifest-path")
        .arg(service_dir.join(".atomo/runtime/Cargo.toml"))
        .current_dir(workspace_root)  // Use workspace root for shared target
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()?;
    
    let status = compile_cmd.wait().await?;
    if !status.success() {
        anyhow::bail!("❌ Compilation failed");
    }
    
    println!("   ✅ Compilation completed successfully");
    Ok(())
}

/// Start service with hot reload for both core changes and schema changes
async fn start_workspace_service_with_hot_reload(
    workspace_root: &Path,
    service_dir: &Path,
    port: u16,
    rx: mpsc::Receiver<notify::Result<notify::Event>>
) -> Result<()> {
    use std::sync::{Arc, Mutex};
    use tokio::process::Child;
    
    println!("🎉 {} {} {}", 
        "".repeat(5),
        "Workspace Development Server Started!".bright_green().bold(),
        "".repeat(5)
    );
    println!("   🌐 Service: http://localhost:{}", port.to_string().bright_blue());
    println!("   🔍 GraphQL Playground: http://localhost:{}/playground", port.to_string().bright_blue());
    println!("   🔥 Hot Reload: Watching core libraries, schema, and CLI files...");
    println!();
    
    let current_process: Arc<Mutex<Option<Child>>> = Arc::new(Mutex::new(None));
    
    // Start initial service process
    let binary_path = service_dir.join(".atomo/runtime/target/debug/service");
    
    {
        let mut process_guard = current_process.lock().unwrap();
        let service_process = TokioCommand::new(&binary_path)
            .current_dir(service_dir)
            .env("PORT", port.to_string())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()?;
        *process_guard = Some(service_process);
    }
    
    // Hot reload loop
    let mut last_reload_time = std::time::Instant::now();
    
    loop {
        if let Ok(event) = rx.recv_timeout(Duration::from_millis(100)) {
            match event {
                Ok(notify::Event { kind: notify::EventKind::Modify(_), paths, .. }) => {
                    let now = std::time::Instant::now();
                    if now.duration_since(last_reload_time) < Duration::from_millis(1000) {
                        continue; // Debounce events
                    }
                    last_reload_time = now;
                    
                    let changed_file = paths.first().map(|p| p.display().to_string())
                        .unwrap_or_else(|| "unknown".to_string());
                    
                    println!("🔄 {} File changed: {}", 
                        "Detected change!".bright_yellow(),
                        changed_file.dimmed()
                    );
                    
                    // Check what type of change occurred
                    let cli_changed = paths.iter()
                        .any(|p| p.to_string_lossy().contains("atomo_cli/src"));
                    let schema_changed = paths.iter()
                        .any(|p| p.file_name().and_then(|n| n.to_str()) == Some("schema.ts"));
                    
                    // If CLI files changed, rebuild CLI first
                    if cli_changed {
                        println!("   🔧 CLI files changed - rebuilding CLI tool...");
                        if let Err(e) = rebuild_cli(workspace_root).await {
                            eprintln!("   ❌ CLI rebuild failed: {}", e);
                            continue;
                        }
                    }
                    
                    // Stop current process
                    {
                        let mut process_guard = current_process.lock().unwrap();
                        if let Some(mut process) = process_guard.take() {
                            let _ = process.kill().await;
                            let _ = process.wait().await;
                        }
                    }
                    
                    // Check if schema changed (need regeneration) or just core libs (just recompile)
                    if schema_changed {
                        println!("   📝 Schema changed - regenerating code...");
                        if let Err(e) = generate_service_runtime_code(service_dir, 3000).await {
                            eprintln!("   ❌ Code generation failed: {}", e);
                            continue;
                        }
                    }
                    
                    // Recompile (incremental)
                    println!("   🔧 Recompiling (incremental)...");
                    if let Err(e) = compile_workspace_service(workspace_root, service_dir).await {
                        eprintln!("   ❌ Compilation failed: {}", e);
                        continue;
                    }
                    
                    // Restart service
                    println!("   🚀 Restarting service...");
                    let new_process = TokioCommand::new(&binary_path)
                        .current_dir(service_dir)
                        .env("PORT", port.to_string())
                        .stdout(Stdio::inherit())
                        .stderr(Stdio::inherit())
                        .spawn()?;
                    
                    {
                        let mut process_guard = current_process.lock().unwrap();
                        *process_guard = Some(new_process);
                    }
                    
                    println!("   ✅ {} Service reloaded successfully! 🚀", "Hot reload completed!".bright_green());
                },
                _ => {}
            }
        }
    }
}

use anyhow::{Result, Context};
use colored::*;
use console::style;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::Command as TokioCommand;
use atomo_schema::{TypeScriptParser, CodeGenerator, ResolverGenerator};

/// 即时编译的服务运行时
/// 
/// 核心机制: Atomo 的开发环境采用"即时编译的服务运行时"模式，
/// 实现平台通用性与服务特异性的完美结合。
pub async fn dev_command(port: u16) -> Result<()> {
    println!("🚀 {}", style("Starting Atomo development server...").cyan());
    
    // 步骤1: 检测当前是否在service目录中
    let current_dir = std::env::current_dir()?;
    let service_name = detect_service_context(&current_dir)?;
    
    println!("   📋 Detected service: {}", service_name.bright_yellow());
    
    // 步骤2: 创建临时工作区
    let runtime_dir = create_runtime_workspace(&current_dir, &service_name).await?;
    println!("   📁 Created runtime workspace: {}", runtime_dir.display().to_string().dimmed());
    
    // 步骤3: 生成服务配置文件
    generate_service_config(&runtime_dir, &service_name, port).await?;
    println!("   ⚙️  Generated service configuration");
    
    // 步骤4: 解析schema.ts并生成业务代码
    let schema_path = current_dir.join("schema.ts");
    if !schema_path.exists() {
        anyhow::bail!("❌ schema.ts not found in current directory");
    }
    
    generate_business_code(&runtime_dir, &schema_path).await?;
    println!("   🦀 Generated business code from schema.ts");
    
    // 步骤5: 编译并运行服务
    compile_and_run_service(&runtime_dir, &service_name, port).await?;
    
    Ok(())
}

/// 检测当前是否在service目录中
fn detect_service_context(current_dir: &Path) -> Result<String> {
    // 检查当前目录是否包含schema.ts和atomo.config.ts
    let has_schema = current_dir.join("schema.ts").exists();
    
    if !has_schema {
        anyhow::bail!("❌ Not in a service directory. schema.ts not found.");
    }
    
    // 从目录名推断服务名
    let service_name = current_dir
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("unknown-service")
        .to_string();
    
    Ok(service_name)
}

/// 创建临时工作区
async fn create_runtime_workspace(service_dir: &Path, service_name: &str) -> Result<PathBuf> {
    let runtime_dir = service_dir.join(".atomo").join("runtime");
    
    // 检查是否需要重新生成（增量编译优化）
    let schema_path = service_dir.join("schema.ts");
    let cargo_toml_path = runtime_dir.join("Cargo.toml");
    let models_path = runtime_dir.join("src").join("models.rs");
    
    let should_regenerate = if !runtime_dir.exists() {
        true
    } else {
        // 检查schema.ts是否比生成的文件更新
        let schema_modified = schema_path.metadata()
            .and_then(|m| m.modified())
            .unwrap_or(std::time::UNIX_EPOCH);
        
        let models_modified = models_path.metadata()
            .and_then(|m| m.modified())
            .unwrap_or(std::time::UNIX_EPOCH);
            
        schema_modified > models_modified
    };
    
    if should_regenerate {
        // 清理并重新创建runtime目录
        if runtime_dir.exists() {
            tokio::fs::remove_dir_all(&runtime_dir).await?;
        }
        
        tokio::fs::create_dir_all(&runtime_dir).await?;
        tokio::fs::create_dir_all(runtime_dir.join("src")).await?;
        
        println!("   🔄 Schema changed, regenerating runtime workspace...");
    } else {
        println!("   ♻️  Using cached runtime workspace (no schema changes)...");
    }
    
    Ok(runtime_dir)
}

async fn generate_service_config(runtime_dir: &Path, service_name: &str, port: u16) -> Result<()> {
    let cargo_toml_path = runtime_dir.join("Cargo.toml");
    let main_rs_path = runtime_dir.join("src").join("main.rs");
    
    // 只在文件不存在时才生成
    if !cargo_toml_path.exists() {
        // 首先复制.env文件到运行时目录
        let current_dir = std::env::current_dir()?;
        let source_env = current_dir.join(".env");
        if source_env.exists() {
            let target_env = runtime_dir.join(".env");
            tokio::fs::copy(source_env, target_env).await?;
            println!("   📄 Copied .env file to runtime workspace");
        }
        
        // 生成 Cargo.toml
        let cargo_toml = format!(r#"[package]
name = "{}-runtime"
version = "0.1.0"
edition = "2021"

# 明确表示这不是工作区的一部分
[workspace]

[[bin]]
name = "service"
path = "src/main.rs"

[dependencies]
# 基础运行时
tokio = {{ version = "1.0", features = ["full"] }}
anyhow = "1.0"

# Web 框架 - 锁定兼容版本
axum = "0.8"

# GraphQL - 使用兼容的版本
async-graphql = {{ version = "7.0", features = ["chrono", "uuid"] }}
async-graphql-axum = "7.0"

# 数据库
sqlx = {{ version = "0.7", features = ["runtime-tokio-rustls", "postgres", "chrono", "uuid", "json", "bigdecimal"] }}

# 序列化
serde = {{ version = "1.0", features = ["derive"] }}
serde_json = "1.0"

# 工具库
chrono = {{ version = "0.4", features = ["serde"] }}
uuid = {{ version = "1.0", features = ["v4", "serde"] }}

# HTTP
tower-http = {{ version = "0.5", features = ["cors"] }}

# 日志
tracing = "0.1"
tracing-subscriber = {{ version = "0.3", features = ["env-filter"] }}

# 环境变量
dotenvy = "0.15"

# 共享缓存配置以加速编译
[profile.dev]
incremental = true

[profile.release]
incremental = true
lto = "thin"  # 链接时优化，但不要太慢
"#, service_name);

        tokio::fs::write(&cargo_toml_path, cargo_toml).await?;
    }
    
    if !main_rs_path.exists() {
    let main_rs = format!(r#"//! {} Service Runtime
//! 
//! 这是一个由 Atomo CLI 自动生成的服务运行时。
//! 它将通用的 atomo_server 与特定于该服务的业务代码结合在一起。

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
    
    info!("🚀 Starting {} Service Runtime");
    
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
    
    axum::serve(listener, app).await?;
    
    Ok(())
}}

#[derive(Clone)]
struct AppState {{
    schema: ServiceSchema,
}}

async fn health_check() -> impl IntoResponse {{
    "{} Service is healthy! 🚀"
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
"#, service_name, service_name, port, service_name, service_name);

    tokio::fs::write(runtime_dir.join("src").join("main.rs"), main_rs).await?;
    }
    
    Ok(())
}

/// 生成业务代码 (models.rs, resolvers.rs)
async fn generate_business_code(runtime_dir: &Path, schema_path: &Path) -> Result<()> {
    let models_path = runtime_dir.join("src").join("models.rs");
    let resolvers_path = runtime_dir.join("src").join("resolvers.rs");
    
    // 检查是否需要重新生成
    let schema_modified = schema_path.metadata()
        .and_then(|m| m.modified())
        .unwrap_or(std::time::UNIX_EPOCH);
    
    let models_modified = models_path.metadata()
        .and_then(|m| m.modified())
        .unwrap_or(std::time::UNIX_EPOCH);
    
    if schema_modified <= models_modified && models_path.exists() && resolvers_path.exists() {
        println!("   ♻️  Using cached business code (no schema changes)...");
        return Ok(());
    }
    
    // 读取并解析 schema.ts
    let schema_content = tokio::fs::read_to_string(schema_path).await?;
    
    let parser = TypeScriptParser::new();
    let models = parser.parse(&schema_content)
        .with_context(|| "Failed to parse schema.ts")?;
    
    // 生成 models.rs
    let mut schema_models = std::collections::HashMap::new();
    for model in &models {
        schema_models.insert(model.name.clone(), model.clone());
    }
    let schema = atomo_schema::Schema { models: schema_models };
    
    let models_code = CodeGenerator::generate_rust_models(&schema)
        .with_context(|| "Failed to generate models")?;
    
    tokio::fs::write(&models_path, models_code).await?;
    
    // 生成 resolvers.rs
    let resolver_generator = ResolverGenerator::new();
    let resolvers_code = resolver_generator.generate_resolvers(&models)
        .with_context(|| "Failed to generate resolvers")?;
    
    tokio::fs::write(&resolvers_path, resolvers_code).await?;
    
    println!("   🔄 Regenerated business code from schema changes...");
    
    Ok(())
}

/// 编译并运行服务
async fn compile_and_run_service(runtime_dir: &Path, service_name: &str, port: u16) -> Result<()> {
    println!("   🔧 Compiling service runtime...");
    println!("   📜 Compilation logs:");
    println!();
    
    // 实时显示编译日志
    let mut compile_child = TokioCommand::new("cargo")
        .arg("build")
        .arg("--release")
        .current_dir(runtime_dir)
        .stdout(Stdio::inherit())  // 直接继承终端输出，实时显示
        .stderr(Stdio::inherit())   // 直接继承终端错误输出，实时显示
        .spawn()?;
    
    let compile_status = compile_child.wait().await?;
    
    if !compile_status.success() {
        anyhow::bail!("❌ Failed to compile service runtime. Please check the compilation errors above.");
    }
    
    println!("   ✅ Service compiled successfully");
    println!();
    
    // 清屏并显示服务启动信息
    println!("\x1B[2J\x1B[1;1H"); // ANSI清屏码
    println!("{}", "-".repeat(50).bright_cyan());
    println!("🎉 {} {} {}", 
        "".repeat(5),
        format!("{} Started Successfully!", service_name).bright_green().bold(),
        "".repeat(5)
    );
    println!("{}", "-".repeat(50).bright_cyan());
    println!();
    println!("   🌐 Access your service:");
    println!("   ├─ 🏠 Homepage:           {}", format!("http://localhost:{}", port).bright_blue());
    println!("   ├─ 🔍 GraphQL Playground: {}", format!("http://localhost:{}/playground", port).bright_blue());
    println!("   ├─ 📊 GraphQL API:        {}", format!("http://localhost:{}/graphql", port).bright_blue());
    println!("   └─ 💚 Health Check:      {}", format!("http://localhost:{}/health", port).bright_blue());
    println!();
    println!("{}", "📋 Service Runtime Logs:".bright_yellow().bold());
    println!("{}", "─".repeat(70).yellow());
    
    // 直接运行二进制文件，不通过cargo run，减少输出干扰
    let service_binary = runtime_dir.join("target").join("release").join("service.exe");
    
    let mut child = TokioCommand::new(&service_binary)
        .current_dir(runtime_dir)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()?;
    
    // 等待进程结束
    let status = child.wait().await?;
    
    if !status.success() {
        anyhow::bail!("❌ Service exited with error");
    }
    
    Ok(())
}

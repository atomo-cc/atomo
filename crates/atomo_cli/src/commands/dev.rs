use anyhow::{Result, Context};
use colored::*;
use console::style;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::Command as TokioCommand;
use atomo_schema::{TypeScriptParser, HasuraV2ResolverGenerator, hasura_v2_type_generator::HasuraV2TypeGenerator, OperationDefinitions};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::sync::mpsc;
use std::time::Duration;

/// 即时编译的服务运行时
/// 
/// 核心机制: Atomo 的开发环境采用"即时编译的服务运行时"模式，
/// 实现平台通用性与服务特异性的完美结合。
pub async fn dev_command(
    port: u16,
    workspace: bool,
    isolated: bool,
    service_path: Option<PathBuf>,
    strict_schema_flag: bool,
    verify_schema_flag: bool,
) -> Result<()> {
    // Resolve mode: explicit flags override auto-detection, and log the reason
    let cwd = std::env::current_dir()?;
    let mut use_workspace = false;
    let mut mode_note = String::new();
    if isolated {
        use_workspace = false;
        mode_note = "Using isolated mode (forced by --isolated)".to_string();
    } else if workspace {
        use_workspace = true;
        mode_note = "Using workspace mode (forced by --workspace)".to_string();
    } else {
        match detect_workspace_root_from(cwd.as_path())? {
            Some(root) => {
                use_workspace = true;
                mode_note = format!(
                    "Using workspace mode (auto-detected at {})",
                    root.display()
                );
            }
            None => {
                use_workspace = false;
                mode_note = "Using isolated mode (no workspace detected)".to_string();
            }
        }
    }

    // Determine validation behavior
    // Defaults: workspace -> warn (non-strict), isolated -> strict
    let effective_verify = if verify_schema_flag { true } else { true };
    let effective_strict = if strict_schema_flag {
        true
    } else if use_workspace {
        false
    } else {
        true
    };

    // Dispatch to workspace flow when selected or auto-detected
    println!("   {}", mode_note.dimmed());
    if use_workspace {
        return super::workspace_dev::workspace_dev_command(port, service_path, effective_strict, effective_verify).await;
    }

    println!("🚀 {}", style("Starting Atomo development server (isolated mode)...").cyan());
    
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
    
    // 步骤4: Schema-First开发流程 - 确保Schema与Runtime 100%一致 (简化版)
    let schema_path = current_dir.join("schema.ts");
    if !schema_path.exists() {
        anyhow::bail!("❌ schema.ts not found in current directory");
    }
    
    // 使用统一的操作符定义验证一致性（isolated 默认严格）
    if effective_verify {
        let operation_definitions = OperationDefinitions::hasura_v2_standard();
        operation_definitions
            .validate()
            .context("Failed to validate operation definitions")?;
        println!("   🎯 Using Hasura v2 standard operation definitions");
        println!("      - {} comparison operators", operation_definitions.comparison_ops.len());
        println!("      - {} logical operators", operation_definitions.logical_ops.len());
    }
    
    // 解析并生成代码（分离式生成，支持增量更新）
    generate_business_code_incremental(&runtime_dir, &schema_path).await?;
    println!("   🦀 Generated business code from schema.ts");
    
    // 验证schema一致性 (Hasura v2标准)
    if effective_verify {
        let schema_graphql_path = runtime_dir.join("schema.graphql");
        match verify_schema_consistency(&schema_graphql_path).await {
            Ok(()) => println!("   ✅ Schema consistency verified (Hasura v2 compliant)"),
            Err(e) => {
                if effective_strict { return Err(e); }
                eprintln!("   ⚠️  Schema validation warning: {}", e);
            }
        }
    }
    
    // 步骤5: 编译并运行服务（带文件监听）
    compile_and_run_service_with_watch(&runtime_dir, &service_name, port, &current_dir).await?;
    
    Ok(())
}

/// Shared incremental codegen entry used by both isolated and workspace modes.
/// Generates or updates models.rs, resolvers.rs, and schema.graphql under `runtime_dir/src`.
pub async fn shared_incremental_codegen(runtime_dir: &Path, schema_path: &Path) -> Result<()> {
    generate_business_code_incremental(runtime_dir, schema_path).await
}

/// Optional schema validation shared helper (OperationDefinitions + Hasura v2 checks)
pub async fn shared_validate_schema(runtime_dir: &Path, schema_path: &Path, strict: bool) -> Result<()> {
    // Ensure GraphQL schema exists/updated
    let schema_graphql_path = runtime_dir.join("schema.graphql");
    if !schema_graphql_path.exists() {
        generate_graphql_schema_definition(schema_path, &schema_graphql_path).await?;
    }

    // Validate Hasura v2 compatibility
    if let Err(e) = verify_schema_consistency(&schema_graphql_path).await {
        if strict { return Err(e); }
        eprintln!("   ⚠️  Schema validation warning: {}", e);
    }
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
        println!("   🔄 First run detected - generating runtime");
        true
    } else {
        // 智能增量检测 - 基于文件哈希而非时间戳
        check_incremental_changes(&schema_path, &runtime_dir).await?
    };
    
    if should_regenerate {
        // 🔧 关键优化：不删除整个目录，只重新生成必要的源代码
        // 保留 target/ 目录和 Cargo.lock 以利用编译缓存
        
        // 创建必要的目录结构
        tokio::fs::create_dir_all(&runtime_dir).await?;
        tokio::fs::create_dir_all(runtime_dir.join("src")).await?;
        
        println!("   🏗️  Incremental regeneration (preserving build cache)");
    } else {
        println!("   ⚡ Using cached runtime - fast compilation mode");
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
num-traits = "0.2"

# HTTP
tower-http = {{ version = "0.5", features = ["cors"] }}

# 日志
tracing = "0.1"
tracing-subscriber = {{ version = "0.3", features = ["env-filter"] }}

# 环境变量
dotenvy = "0.15"

# 开发环境编译优化
[profile.dev]
incremental = true
debug = 1  # 减少调试信息，加快编译
opt-level = 0
overflow-checks = false
lto = false
codegen-units = 256  # 增加并行编译单元

# 快速开发配置 - 进一步优化编译速度
[profile.fast-dev]
inherits = "dev"
debug = false  # 完全关闭调试信息
opt-level = 0
incremental = true
codegen-units = 512  # 最大并行度

# 保留 release 配置用于生产环境
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

/// 生成业务代码 (models.rs, resolvers.rs) - 增强版本
/// 真正的细粒度增量代码生成 - 只重新生成变更的部分
pub(crate) async fn generate_business_code_incremental(runtime_dir: &Path, schema_path: &Path) -> Result<()> {
    let models_path = runtime_dir.join("src").join("models.rs");
    let resolvers_path = runtime_dir.join("src").join("resolvers.rs");
    
    // 首先生成GraphQL schema文件用于开发工具
    let schema_graphql_path = runtime_dir.join("schema.graphql");
    
    // 读取并解析schema
    let schema_content = tokio::fs::read_to_string(schema_path).await?;
    let parser = TypeScriptParser::new();
    let models = parser.parse(&schema_content)
        .with_context(|| "Failed to parse schema.ts")?;
    
    // 检查什么需要重新生成
    let change_detection = detect_incremental_changes_detailed(schema_path, runtime_dir, &models).await?;
    
    println!("   📊 Incremental analysis:");
    println!("      ├─ Models changed: {}", change_detection.models_changed);
    println!("      ├─ Resolvers changed: {}", change_detection.resolvers_changed);
    println!("      └─ Schema hash: {}...", change_detection.schema_hash[..8].to_string());
    
    // 始终生成GraphQL schema文件（轻量操作）
    if change_detection.models_changed || change_detection.resolvers_changed {
        println!("   📄 Generating GraphQL schema definition...");
        generate_graphql_schema_definition(schema_path, &schema_graphql_path).await?;
        
        // 🔧 关键优化：只重新生成实际变更的文件，支持并行生成
        match (change_detection.models_changed, change_detection.resolvers_changed) {
            (true, true) => {
                println!("   🏗️  Regenerating models.rs and resolvers.rs in parallel...");
                let models_task = generate_models_only(runtime_dir, schema_path, &models);
                let resolvers_task = generate_resolvers_only(runtime_dir, schema_path, &models);
                
                // 并行执行两个生成任务
                let (models_result, resolvers_result) = tokio::try_join!(models_task, resolvers_task)?;
                println!("   ✅ Parallel generation completed successfully");
            },
            (true, false) => {
                println!("   🏗️  Regenerating models.rs...");
                generate_models_only(runtime_dir, schema_path, &models).await?;
                println!("   ✅ Resolvers unchanged - skipping");
            },
            (false, true) => {
                println!("   🔧 Regenerating resolvers.rs...");
                generate_resolvers_only(runtime_dir, schema_path, &models).await?;
                println!("   ✅ Models unchanged - skipping");
            },
            (false, false) => {
                // 这种情况已经在外层处理了，不应该到达这里
                unreachable!("Both models and resolvers unchanged, should be caught earlier");
            }
        }
        
        // 更新缓存哈希
        update_incremental_cache(runtime_dir, &change_detection).await?;
        
        println!("   ✅ Incremental business code generated successfully");
    } else {
        println!("   ⚡ No business code changes detected - using cache");
    }
    
    Ok(())
}

/// Detect workspace root from a starting directory by looking for a Cargo.toml with [workspace]
fn detect_workspace_root_from(start: &Path) -> Result<Option<PathBuf>> {
    let mut current = start;
    for _ in 0..10 {
        let cargo = current.join("Cargo.toml");
        if cargo.exists() {
            let content = std::fs::read_to_string(&cargo).unwrap_or_default();
            if content.contains("[workspace]") && current.join("crates").exists() {
                return Ok(Some(current.to_path_buf()))
            }
        }
        if let Some(parent) = current.parent() { current = parent; } else { break; }
    }
    Ok(None)
}

/// 原有的完整生成函数，保留用于回退
async fn generate_business_code(runtime_dir: &Path, schema_path: &Path) -> Result<()> {
    let models_path = runtime_dir.join("src").join("models.rs");
    let resolvers_path = runtime_dir.join("src").join("resolvers.rs");
    
    // 首先生成GraphQL schema文件用于开发工具
    let schema_graphql_path = runtime_dir.join("schema.graphql");
    
    // 检查是否需要重新生成
    let schema_modified = schema_path.metadata()
        .and_then(|m| m.modified())
        .unwrap_or(std::time::UNIX_EPOCH);
    
    let models_modified = models_path.metadata()
        .and_then(|m| m.modified())
        .unwrap_or(std::time::UNIX_EPOCH);
    
    let resolvers_modified = resolvers_path.metadata()
        .and_then(|m| m.modified())
        .unwrap_or(std::time::UNIX_EPOCH);
    
    // 始终生成GraphQL schema文件（轻量操作）
    println!("   📄 Generating GraphQL schema definition...");
    generate_graphql_schema_definition(schema_path, &schema_graphql_path).await?;
    
    // 智能判断是否需要重新生成Rust代码
    // 检查schema.ts、生成器源码和生成文件的修改时间
    let generator_source_time = get_generator_source_modification_time();
    
    let should_regenerate = schema_modified > models_modified || 
        schema_modified > resolvers_modified ||
        generator_source_time > models_modified ||
        generator_source_time > resolvers_modified ||
        !models_path.exists() || 
        !resolvers_path.exists();
    
    if !should_regenerate {
        println!("   ♻️  Using cached business code (no schema changes detected)...");
        return Ok(());
    }
    
    println!("   🔄 Regenerating business code from schema changes...");
    
    // 读取并解析 schema.ts
    let schema_content = tokio::fs::read_to_string(schema_path).await?;
    
    let parser = TypeScriptParser::new();
    let models = parser.parse(&schema_content)
        .with_context(|| "Failed to parse schema.ts")?;
    
    // 显示schema分析信息
    println!("   📊 Schema analysis:");
    println!("      ├─ Models detected: {}", models.len());
    println!("      ├─ Complexity: {}", calculate_complexity_score(&models));
    println!("      └─ Hash: {}...", calculate_schema_hash(&schema_content)[..8].to_string());
    
    // 生成 models.rs
    let type_generator = HasuraV2TypeGenerator::new();
    let models_code = type_generator.generate_types(&models)
        .with_context(|| "Failed to generate models")?;
    
    tokio::fs::write(&models_path, models_code).await?;
    
    // 生成 resolvers.rs
    let resolver_generator = HasuraV2ResolverGenerator::new();
    let resolvers_code = resolver_generator.generate_resolvers(&models)
        .with_context(|| "Failed to generate resolvers")?;
    
    tokio::fs::write(&resolvers_path, resolvers_code).await?;
    
    // 保存缓存信息
    save_build_cache(runtime_dir, &schema_content, &models).await?;
    
    println!("   ✅ Business code generated successfully");
    
    Ok(())
}

/// 生成标准GraphQL schema定义文件
async fn generate_graphql_schema_definition(schema_path: &Path, output_path: &Path) -> Result<()> {
    let schema_content = tokio::fs::read_to_string(schema_path).await?;
    let parser = TypeScriptParser::new();
    let models = parser.parse(&schema_content)?;
    
    // 生成标准的GraphQL schema定义
    let type_generator = HasuraV2TypeGenerator::new();
    let graphql_schema = type_generator.generate_graphql_schema_definition(&models)?;
    
    tokio::fs::write(output_path, graphql_schema).await?;
    
    Ok(())
}

/// 计算schema复杂度分数
fn calculate_complexity_score(models: &[atomo_schema::Model]) -> u32 {
    let mut score = 0u32;
    
    for model in models {
        score += model.fields.len() as u32; // 字段数量
        score += model.fields.iter()
            .filter(|(_, f)| matches!(f.field_type, atomo_schema::FieldType::Array(_)))
            .count() as u32 * 2; // 数组字段权重更高
        
        // 关系字段权重
        score += model.fields.iter()
            .filter(|(_, f)| matches!(f.field_type, atomo_schema::FieldType::Reference(_)))
            .count() as u32 * 3;
    }
    
    score
}

/// 计算schema内容哈希
fn calculate_schema_hash(content: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

/// 保存构建缓存信息
async fn save_build_cache(runtime_dir: &Path, schema_content: &str, models: &[atomo_schema::Model]) -> Result<()> {
    let cache_dir = runtime_dir.join("cache");
    tokio::fs::create_dir_all(&cache_dir).await?;
    
    let cache_info = serde_json::json!({
        "timestamp": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        "schema_hash": calculate_schema_hash(schema_content),
        "models_count": models.len(),
        "complexity_score": calculate_complexity_score(models)
    });
    
    let cache_file = cache_dir.join("build_info.json");
    tokio::fs::write(cache_file, serde_json::to_string_pretty(&cache_info)?).await?;
    
    Ok(())
}

/// 编译并运行服务 - 优化版本，支持依赖缓存
async fn compile_and_run_service(runtime_dir: &Path, service_name: &str, port: u16) -> Result<()> {
    println!("   🔧 Compiling service runtime...");
    
    // 步骤1: 检查是否需要预编译依赖
    let dependencies_cached = check_dependencies_cache(runtime_dir).await?;
    if !dependencies_cached {
        println!("   � Pre-compiling dependencies for faster future builds...");
        precompile_dependencies(runtime_dir).await?;
    }
    
    println!("   �📜 Compilation logs:");
    println!();
    
    // 步骤2: 使用优化的编译配置
    let mut compile_child = TokioCommand::new("cargo")
        .arg("build")
        .arg("--profile")
        .arg("fast-dev")  // 使用我们的快速开发配置
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
    
    // 直接运行二进制文件，使用fast-dev目录以匹配编译模式
    let service_binary = runtime_dir
        .join("target")
        .join("fast-dev")
        .join(platform_service_binary_name());
    
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

/// 验证Schema一致性 - 确保生成的GraphQL schema符合Hasura v2标准
async fn verify_schema_consistency(schema_graphql_path: &Path) -> Result<()> {
    if !schema_graphql_path.exists() {
        anyhow::bail!("Schema validation failed: schema.graphql file not found");
    }
    
    let schema_content = tokio::fs::read_to_string(schema_graphql_path).await?;
    
    // 验证基本结构
    verify_basic_structure(&schema_content)?;
    
    // 验证Hasura v2兼容性
    verify_hasura_v2_compliance(&schema_content)?;
    
    // 验证比较操作符完整性
    verify_comparison_operators(&schema_content)?;
    
    Ok(())
}

/// 验证基本GraphQL schema结构
fn verify_basic_structure(content: &str) -> Result<()> {
    let required_elements = [
        "type Query",
        "type Mutation", 
        "input",
        "enum",
    ];
    
    for element in &required_elements {
        if !content.contains(element) {
            anyhow::bail!("Schema validation failed: Missing required element '{}'", element);
        }
    }
    
    Ok(())
}

/// 验证Hasura v2兼容性
fn verify_hasura_v2_compliance(content: &str) -> Result<()> {
    // 检查所有模型是否都有对应的BoolExp类型
    let mut models = Vec::new();
    
    // 提取所有类型定义中的模型名
    for line in content.lines() {
        if line.trim().starts_with("type ") && !line.contains("Query") && !line.contains("Mutation") {
            if let Some(type_name) = extract_type_name(line) {
                if !type_name.ends_with("BoolExp") && 
                   !type_name.ends_with("OrderBy") && 
                   !type_name.ends_with("ComparisonExp") {
                    models.push(type_name);
                }
            }
        }
    }
    
    // 验证每个模型都有对应的BoolExp和OrderBy类型
    for model in &models {
        let bool_exp = format!("{}BoolExp", model);
        let order_by = format!("{}OrderBy", model);
        
        if !content.contains(&bool_exp) {
            anyhow::bail!("Hasura v2 validation failed: Missing BoolExp type for model '{}'", model);
        }
        
        if !content.contains(&order_by) {
            anyhow::bail!("Hasura v2 validation failed: Missing OrderBy type for model '{}'", model);
        }
    }
    
    Ok(())
}

/// 验证比较操作符完整性
fn verify_comparison_operators(content: &str) -> Result<()> {
    let required_operators = [
        "_eq", "_neq", "_gt", "_gte", "_lt", "_lte", 
        "_in", "_nin", "_is_null"
    ];
    
    // 检查基本比较操作符类型是否存在
    let comparison_types = [
        "StringComparisonExp",
        "NumericComparisonExp",   // 用于所有数字类型（Int, Float等）
        "DateTimeComparisonExp",
        "BooleanComparisonExp",
        "UUIDComparisonExp",
        "GenericComparisonExp"    // 通用比较类型
    ];
    
    for comp_type in &comparison_types {
        if !content.contains(comp_type) {
            anyhow::bail!("Schema validation failed: Missing comparison type '{}'", comp_type);
        }
    }
    
    // 验证Hasura v2标准：所有操作符必须有下划线前缀
    println!("   Info: Verifying Hasura v2 operator naming convention (all operators must have _ prefix)");
    
    // 检查每个比较类型中的操作符
    for comp_type in &comparison_types {
        for operator in &required_operators {
            // 检查操作符存在且有正确的下划线前缀
            let pattern = format!("  {}: ", operator);
            if !content.contains(&pattern) {
                // 对于某些操作符对某些类型可能不适用（如Boolean类型不需要_gt, _lt等）
                if comp_type.contains("Boolean") && 
                   (operator.contains("_gt") || operator.contains("_lt") || 
                    operator.contains("_in") || operator.contains("_nin")) {
                    continue; // Boolean类型不需要这些操作符
                }
                if comp_type.contains("UUID") && 
                   (operator.contains("_gt") || operator.contains("_lt")) {
                    continue; // UUID类型不需要大小比较
                }
                println!("   Info: Operator '{}' not found in type '{}' (may be expected)", operator, comp_type);
            }
        }
    }
    
    // 精确验证：确保所有操作符都有下划线前缀（Hasura v2标准）
    let required_operators = [
        "_eq:", "_neq:", "_gt:", "_gte:", "_lt:", "_lte:", 
        "_in:", "_nin:", "_is_null:", "_like:", "_ilike:",
        "_similar:", "_regex:", "_iregex:"
    ];
    
    // 检查是否有无下划线的操作符（使用精确模式匹配）
    let invalid_patterns = [
        "  eq:", "  neq:", "  gt:", "  gte:", "  lt:", "  lte:", 
        "  in:", "  nin:", "  is_null:", "  like:", "  ilike:",
        "  similar:", "  regex:", "  iregex:"
    ];
    
    for pattern in &invalid_patterns {
        if content.contains(pattern) {
            let op_name = pattern.trim();
            anyhow::bail!("Schema validation failed: Found invalid operator '{}' without underscore prefix. Hasura v2 requires all operators to have _ prefix.", op_name.trim_end_matches(':'));
        }
    }
    
    // 验证必需的操作符是否存在
    let mut missing_operators = Vec::new();
    for required_op in &required_operators {
        let pattern = format!("  {}", required_op);
        if !content.contains(&pattern) {
            missing_operators.push(required_op.trim_end_matches(':'));
        }
    }
    
    if !missing_operators.is_empty() {
        println!("   Warning: Some standard Hasura v2 operators may be missing: {:?}", missing_operators);
    }
    
    println!("   Info: All comparison operators follow Hasura v2 naming convention (with _ prefix)");
    
    Ok(())
}

/// 从类型定义行中提取类型名
fn extract_type_name(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.starts_with("type ") {
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.len() >= 2 {
            return Some(parts[1].to_string());
        }
    }
    None
}

/// Schema-First开发流程 - 确保Schema与Runtime使用相同的操作符定义
/// 
/// 核心思想：使用统一的OperationDefinitions作为单一数据源，
/// 先生成GraphQL Schema，验证一致性，再生成Runtime代码
async fn schema_first_generation(runtime_dir: &Path, schema_path: &Path) -> Result<()> {
    println!("   🎯 Starting Schema-First generation with unified operation definitions...");
    
    // 步骤1: 建立统一的操作符定义（单一数据源）
    let operation_definitions = OperationDefinitions::hasura_v2_standard();
    operation_definitions.validate()
        .context("Failed to validate operation definitions")?;
    
    println!("   📋 Using Hasura v2 standard operation definitions:");
    println!("      - {} comparison operators", operation_definitions.comparison_ops.len());
    println!("      - {} logical operators", operation_definitions.logical_ops.len());
    
    // 步骤2: 解析schema.ts
    let schema_content = std::fs::read_to_string(schema_path)
        .context("Failed to read schema.ts")?;
    
    let mut parser = TypeScriptParser::new();
    let models = parser.parse(&schema_content)
        .context("Failed to parse schema.ts")?;
    
    println!("   📝 Parsed {} models from schema.ts", models.len());
    
    // 步骤3: 使用统一定义生成GraphQL Schema
    let type_generator = HasuraV2TypeGenerator;
    let schema_graphql_content = type_generator.generate_graphql_schema_with_operations(&models, &operation_definitions)
        .context("Failed to generate GraphQL schema with unified operations")?;
    
    let schema_graphql_path = runtime_dir.join("schema.graphql");
    std::fs::write(&schema_graphql_path, &schema_graphql_content)
        .context("Failed to write schema.graphql")?;
    
    println!("   📄 Generated schema.graphql with unified operation definitions");
    
    // 步骤4: 验证Schema一致性
    verify_schema_consistency_with_operations(&schema_graphql_path, &operation_definitions).await
        .context("Schema consistency validation failed")?;
    
    println!("   ✅ Schema consistency verified - all operations match definitions");
    
    // 步骤5: 使用相同定义生成Runtime代码
    let resolver_generator = HasuraV2ResolverGenerator::new();
    let rust_models_content = resolver_generator.generate_models_with_operations(&models, &operation_definitions)
        .context("Failed to generate Rust models with unified operations")?;
    
    let models_path = runtime_dir.join("src").join("models.rs");
    std::fs::write(&models_path, &rust_models_content)
        .context("Failed to write models.rs")?;
    
    println!("   🦀 Generated models.rs with identical operation definitions");
    
    // 步骤6: 生成Resolver实现
    let resolver_content = resolver_generator.generate_resolvers_with_operations(&models, &operation_definitions)
        .context("Failed to generate resolvers with unified operations")?;
    
    let resolvers_path = runtime_dir.join("src").join("resolvers.rs");
    std::fs::write(&resolvers_path, &resolver_content)
        .context("Failed to write resolvers.rs")?;
    
    println!("   🔗 Generated resolvers.rs with matching operation definitions");
    
    // 步骤7: 运行时一致性最终验证
    verify_runtime_schema_consistency(&schema_graphql_path, &models_path, &operation_definitions).await
        .context("Runtime-Schema consistency validation failed")?;
    
    println!("   🎉 Schema-First generation completed - 100% consistency guaranteed!");
    
    Ok(())
}

/// 验证Schema一致性（增强版）- 使用统一的操作符定义进行验证
async fn verify_schema_consistency_with_operations(
    schema_graphql_path: &Path, 
    operation_definitions: &OperationDefinitions
) -> Result<()> {
    let content = std::fs::read_to_string(schema_graphql_path)
        .context("Failed to read schema.graphql")?;
    
    println!("   🔍 Verifying schema consistency with unified operation definitions...");
    
    // 验证所有定义的操作符都在schema中存在
    for op in &operation_definitions.comparison_ops {
        let pattern = format!("  {}: ", op.name);
        if !content.contains(&pattern) {
            anyhow::bail!(
                "Operation definition inconsistency: '{}' defined but not found in schema", 
                op.name
            );
        }
    }
    
    // 验证没有未定义的操作符出现在schema中
    let lines: Vec<&str> = content.lines().collect();
    for line in lines {
        if line.trim().starts_with("_") && line.contains(": ") {
            let op_name = line.trim().split(": ").next().unwrap_or("");
            if !operation_definitions.comparison_ops.iter().any(|op| op.name == op_name) &&
               !operation_definitions.logical_ops.iter().any(|op| op.name == op_name) {
                anyhow::bail!(
                    "Undefined operation found in schema: '{}' - not in operation definitions", 
                    op_name
                );
            }
        }
    }
    
    println!("   ✅ All operation definitions properly reflected in schema");
    Ok(())
}

/// 验证Runtime与Schema的一致性
async fn verify_runtime_schema_consistency(
    schema_graphql_path: &Path,
    models_path: &Path, 
    operation_definitions: &OperationDefinitions
) -> Result<()> {
    println!("   🔬 Performing runtime-schema consistency verification...");
    
    let schema_content = std::fs::read_to_string(schema_graphql_path)
        .context("Failed to read schema.graphql")?;
    
    let models_content = std::fs::read_to_string(models_path)
        .context("Failed to read models.rs")?;
    
    // 验证每个操作符在schema和models中都存在
    for op in &operation_definitions.comparison_ops {
        let schema_pattern = format!("  {}: ", op.name);
        let models_pattern = format!("pub {}: ", op.name);
        
        let in_schema = schema_content.contains(&schema_pattern);
        let in_models = models_content.contains(&models_pattern);
        
        if in_schema != in_models {
            anyhow::bail!(
                "Runtime-Schema inconsistency for '{}': schema={}, models={}", 
                op.name, in_schema, in_models
            );
        }
    }
    
    println!("   ✅ Runtime and Schema are 100% consistent");
    Ok(())
}

/// 获取生成器源码的最新修改时间
/// 这样当我们修复生成器代码时，缓存会自动失效
fn get_generator_source_modification_time() -> std::time::SystemTime {
    // 获取当前可执行文件的路径，然后找到项目根目录
    let current_exe = std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from(""));
    let mut project_root = current_exe.parent().unwrap_or(std::path::Path::new(""));
    
    // 向上查找直到找到 Cargo.toml（项目根目录标识）
    for _ in 0..10 {  // 最多向上查找10层
        if project_root.join("Cargo.toml").exists() {
            break;
        }
        if let Some(parent) = project_root.parent() {
            project_root = parent;
        } else {
            break;
        }
    }
    
    let generator_files = [
        "crates/atomo_schema/src/hasura_v2_resolver_generator.rs",
        "crates/atomo_schema/src/hasura_v2_type_generator.rs",
        "crates/atomo_schema/src/operation_definitions.rs",
    ];
    
    let mut latest_time = std::time::UNIX_EPOCH;
    
    for file_path in &generator_files {
        let full_path = project_root.join(file_path);
        if let Ok(metadata) = std::fs::metadata(&full_path) {
            if let Ok(modified) = metadata.modified() {
                if modified > latest_time {
                    latest_time = modified;
                }
            }
        }
    }
    
    latest_time
}

/// 增量变更检测结果
#[derive(Debug)]
struct IncrementalChangeSet {
    schema_hash: String,
    models_changed: bool,
    resolvers_changed: bool,
}

/// 详细的增量变更检测
async fn detect_incremental_changes_detailed(
    schema_path: &Path, 
    runtime_dir: &Path, 
    models: &[atomo_schema::types::Model]
) -> Result<IncrementalChangeSet> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    
    // 计算当前schema的哈希
    let schema_content = tokio::fs::read_to_string(schema_path).await?;
    let mut hasher = DefaultHasher::new();
    schema_content.hash(&mut hasher);
    let current_schema_hash = hasher.finish().to_string();
    
    // 计算模型结构的哈希（只包含结构相关的部分）
    let models_structure = extract_models_structure(models);
    let mut models_hasher = DefaultHasher::new();
    models_structure.hash(&mut models_hasher);
    let current_models_hash = models_hasher.finish().to_string();
    
    // 读取缓存
    let cache_dir = runtime_dir.join(".cache");
    tokio::fs::create_dir_all(&cache_dir).await?;
    
    let schema_cache_file = cache_dir.join("schema.hash");
    let models_cache_file = cache_dir.join("models.hash");
    let resolvers_cache_file = cache_dir.join("resolvers.hash");
    
    let cached_schema_hash = read_cache_file(&schema_cache_file).await;
    let cached_models_hash = read_cache_file(&models_cache_file).await;
    let cached_resolvers_hash = read_cache_file(&resolvers_cache_file).await;
    
    // 检查文件是否存在
    let models_file_exists = runtime_dir.join("src").join("models.rs").exists();
    let resolvers_file_exists = runtime_dir.join("src").join("resolvers.rs").exists();
    
    // 判断是否需要重新生成
    let models_changed = !models_file_exists || 
        cached_models_hash != Some(current_models_hash.clone()) ||
        cached_schema_hash != Some(current_schema_hash.clone());
    
    // Resolvers 依赖于整个 schema，任何 schema 变更都需要重新生成 resolvers
    // 使用 cached_resolvers_hash 而不是 cached_schema_hash，因为我们需要检查 resolvers.hash 文件
    let resolvers_changed = !resolvers_file_exists ||
        cached_resolvers_hash != Some(current_schema_hash.clone());
    
    Ok(IncrementalChangeSet {
        schema_hash: current_schema_hash,
        models_changed,
        resolvers_changed,
    })
}

/// 提取模型结构信息（用于检测结构性变更）
fn extract_models_structure(models: &[atomo_schema::types::Model]) -> String {
    let mut structure = String::new();
    for model in models {
        structure.push_str(&model.name);
        structure.push(':');
        for (field_name, field_type) in &model.fields {
            structure.push_str(field_name);
            structure.push_str(&format!("{:?}", field_type));
            structure.push(',');
        }
        structure.push(';');
    }
    structure
}

/// 保存增量缓存信息
async fn save_incremental_cache(
    runtime_dir: &Path, 
    schema_content: &str, 
    models: &[atomo_schema::types::Model]
) -> Result<()> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    
    let cache_dir = runtime_dir.join(".cache");
    tokio::fs::create_dir_all(&cache_dir).await?;
    
    // 保存schema哈希
    let mut schema_hasher = DefaultHasher::new();
    schema_content.hash(&mut schema_hasher);
    let schema_hash = schema_hasher.finish().to_string();
    tokio::fs::write(cache_dir.join("schema.hash"), &schema_hash).await?;
    
    // 保存模型结构哈希
    let models_structure = extract_models_structure(models);
    let mut models_hasher = DefaultHasher::new();
    models_structure.hash(&mut models_hasher);
    let models_hash = models_hasher.finish().to_string();
    tokio::fs::write(cache_dir.join("models.hash"), &models_hash).await?;
    
    // 解析器哈希与schema哈希相同（因为解析器依赖整个schema）
    tokio::fs::write(cache_dir.join("resolvers.hash"), &schema_hash).await?;
    
    Ok(())
}

/// 读取缓存文件
async fn read_cache_file(path: &Path) -> Option<String> {
    tokio::fs::read_to_string(path)
        .await
        .ok()
        .map(|s| s.trim().to_string())
}

/// 智能增量变更检测 - 基于内容哈希而非时间戳
async fn check_incremental_changes(schema_path: &Path, runtime_dir: &Path) -> Result<bool> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    
    // 计算当前schema的哈希
    let schema_content = tokio::fs::read_to_string(schema_path).await?;
    let mut hasher = DefaultHasher::new();
    schema_content.hash(&mut hasher);
    let current_schema_hash = hasher.finish();
    
    // 检查缓存的哈希
    let cache_file = runtime_dir.join(".cache").join("schema.hash");
    let cached_hash = if cache_file.exists() {
        tokio::fs::read_to_string(&cache_file)
            .await
            .ok()
            .and_then(|s| s.trim().parse::<u64>().ok())
    } else {
        None
    };
    
    let schema_changed = cached_hash != Some(current_schema_hash);
    
    if schema_changed {
        println!("   🔄 Schema changes detected - incremental regeneration");
        
        // 创建缓存目录并保存新哈希
        let cache_dir = runtime_dir.join(".cache");
        tokio::fs::create_dir_all(&cache_dir).await?;
        tokio::fs::write(&cache_file, current_schema_hash.to_string()).await?;
        
        return Ok(true);
    }
    
    // 检查关键文件是否存在
    let required_files = [
        runtime_dir.join("src").join("models.rs"),
        runtime_dir.join("src").join("resolvers.rs"),
        runtime_dir.join("Cargo.toml"),
    ];
    
    for file in &required_files {
        if !file.exists() {
            println!("   🔄 Missing generated file: {} - regenerating", file.display());
            return Ok(true);
        }
    }
    
    println!("   ✅ No changes detected - using cached runtime");
    Ok(false)
}

/// 更新增量缓存
async fn update_incremental_cache(runtime_dir: &Path, changes: &IncrementalChangeSet) -> Result<()> {
    let cache_dir = runtime_dir.join(".cache");
    tokio::fs::create_dir_all(&cache_dir).await?;
    
    // 保存schema哈希 - 这是检测变化的基础
    tokio::fs::write(cache_dir.join("schema.hash"), &changes.schema_hash).await?;
    
    // 当 models 重新生成时，更新 models cache（使用 schema hash，因为 models 直接依赖 schema）
    if changes.models_changed {
        tokio::fs::write(cache_dir.join("models.hash"), &changes.schema_hash).await?;
    }
    
    // 当 resolvers 重新生成时，更新 resolvers cache（使用 schema hash，因为 resolvers 完全依赖 schema）
    if changes.resolvers_changed {
        tokio::fs::write(cache_dir.join("resolvers.hash"), &changes.schema_hash).await?;
    }
    
    Ok(())
}

/// 只生成模型代码
async fn generate_models_only(runtime_dir: &Path, schema_path: &Path, models: &[atomo_schema::types::Model]) -> Result<()> {
    let type_generator = HasuraV2TypeGenerator::new();
    let models_code = type_generator.generate_types(models)
        .with_context(|| "Failed to generate models")?;
    
    let models_path = runtime_dir.join("src").join("models.rs");
    tokio::fs::write(&models_path, models_code).await?;
    
    Ok(())
}

/// 只生成解析器代码
async fn generate_resolvers_only(runtime_dir: &Path, schema_path: &Path, models: &[atomo_schema::types::Model]) -> Result<()> {
    let resolver_generator = HasuraV2ResolverGenerator::new();
    let resolvers_code = resolver_generator.generate_resolvers(models)
        .with_context(|| "Failed to generate resolvers")?;
    
    let resolvers_path = runtime_dir.join("src").join("resolvers.rs");
    tokio::fs::write(&resolvers_path, resolvers_code).await?;
    
    Ok(())
}

/// 检查依赖缓存是否存在
async fn check_dependencies_cache(runtime_dir: &Path) -> Result<bool> {
    let target_dir = runtime_dir.join("target");
    let deps_dir = target_dir.join("fast-dev").join("deps");
    
    // 检查是否存在编译好的依赖
    if deps_dir.exists() {
        // 检查依赖目录是否包含编译好的文件
        let mut entries = tokio::fs::read_dir(&deps_dir).await?;
        let mut has_deps = false;
        
        while let Some(entry) = entries.next_entry().await? {
            let file_name = entry.file_name();
            if let Some(name_str) = file_name.to_str() {
                // 查找编译好的依赖文件（.rlib 或者可执行文件）
                if name_str.contains("lib") && (name_str.ends_with(".rlib") || name_str.ends_with(".rmeta")) {
                    has_deps = true;
                    break;
                }
            }
        }
        
        return Ok(has_deps);
    }
    
    Ok(false)
}

/// 预编译依赖项
async fn precompile_dependencies(runtime_dir: &Path) -> Result<()> {
    // 创建一个临时的虚拟main.rs用于编译依赖
    let temp_main = runtime_dir.join("src").join("temp_main.rs");
    let original_main = runtime_dir.join("src").join("main.rs");
    
    // 备份原始main.rs
    if original_main.exists() {
        let backup_main = runtime_dir.join("src").join("main.rs.backup");
        tokio::fs::copy(&original_main, &backup_main).await?;
    }
    
    // 创建最小化的main.rs来编译依赖
    let minimal_main = r#"
// 临时文件，仅用于预编译依赖
fn main() {
    println!("Dependencies compiled successfully");
}
"#;
    tokio::fs::write(&original_main, minimal_main).await?;
    
    // 编译依赖（这会编译所有依赖但不会构建我们的代码）
    let mut deps_compile = TokioCommand::new("cargo")
        .arg("build")
        .arg("--profile")
        .arg("fast-dev")
        .arg("--lib")  // 只编译库依赖
        .current_dir(runtime_dir)
        .stdout(Stdio::null())  // 静默模式
        .stderr(Stdio::null())
        .spawn()?;
    
    let _deps_status = deps_compile.wait().await?;
    
    // 恢复原始main.rs
    let backup_main = runtime_dir.join("src").join("main.rs.backup");
    if backup_main.exists() {
        tokio::fs::copy(&backup_main, &original_main).await?;
        tokio::fs::remove_file(&backup_main).await?;
    }
    
    // 清理临时文件
    if temp_main.exists() {
        tokio::fs::remove_file(&temp_main).await?;
    }
    
    Ok(())
}

/// 编译并运行服务（带文件监听和热重载）
async fn compile_and_run_service_with_watch(
    runtime_dir: &Path, 
    service_name: &str, 
    port: u16,
    service_dir: &Path
) -> Result<()> {
    println!("   🔧 Compiling service runtime...");
    
    // 步骤1: 检查是否需要预编译依赖
    let dependencies_cached = check_dependencies_cache(runtime_dir).await?;
    if !dependencies_cached {
        println!("   📦 Pre-compiling dependencies for faster future builds...");
        precompile_dependencies(runtime_dir).await?;
    }
    
    println!("   📜 Compilation logs:");
    println!();
    
    // 步骤2: 初始编译
    let mut compile_child = TokioCommand::new("cargo")
        .arg("build")
        .arg("--profile")
        .arg("fast-dev")  // 使用我们的快速开发配置
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
    println!("{}", "   🔥 Hot Reload: Watching for changes...".bright_yellow().bold());
    println!("{}", "─".repeat(70).yellow());
    
    // 步骤3: 设置文件监听
    let (tx, rx) = mpsc::channel();
    let mut watcher = RecommendedWatcher::new(tx, notify::Config::default())?;
    
    // 监听 schema.ts 文件
    let schema_path = service_dir.join("schema.ts");
    watcher.watch(&schema_path, RecursiveMode::NonRecursive)?;
    
    println!("   👀 Watching: {}", schema_path.display().to_string().dimmed());
    println!();
    
    // 步骤4: 启动带进程管理的热重载服务
    let service_binary = runtime_dir
        .join("target")
        .join("fast-dev")
        .join(platform_service_binary_name());
    let service_dir_clone = service_dir.to_path_buf();
    let runtime_dir_clone = runtime_dir.to_path_buf();
    
    start_service_with_hot_reload(&service_binary, runtime_dir, &service_dir_clone, &runtime_dir_clone, rx).await?;
    
    Ok(())
}

#[inline]
fn platform_service_binary_name() -> &'static str {
    if cfg!(windows) { "service.exe" } else { "service" }
}

/// 热重载：当文件变更时快速重新生成和重编译
async fn hot_reload_service(runtime_dir: &Path, service_dir: &Path) -> Result<()> {
    let schema_path = service_dir.join("schema.ts");
    
    // 重新生成代码（只生成变更的部分）
    generate_business_code_incremental(runtime_dir, &schema_path).await?;
    
    // 快速重新编译
    let mut compile_child = TokioCommand::new("cargo")
        .arg("build")
        .arg("--profile")
        .arg("fast-dev")
        .current_dir(runtime_dir)
        .stdout(Stdio::piped())  // 隐藏详细输出，保持界面简洁
        .stderr(Stdio::piped())
        .spawn()?;
    
    let compile_status = compile_child.wait().await?;
    if !compile_status.success() {
        // 获取编译错误信息
        let output = compile_child.wait_with_output().await?;
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Compilation failed:\n{}", stderr);
    }
    
    Ok(())
}

/// 启动服务并管理热重载进程重启
async fn start_service_with_hot_reload(
    service_binary: &Path,
    runtime_dir: &Path, 
    service_dir: &Path,
    runtime_dir_clone: &Path,
    rx: mpsc::Receiver<notify::Result<notify::Event>>
) -> Result<()> {
    use std::sync::{Arc, Mutex};
    use tokio::process::Child;
    
    let current_process: Arc<Mutex<Option<Child>>> = Arc::new(Mutex::new(None));
    
    // 启动初始服务进程
    {
        let mut process_guard = current_process.lock().unwrap();
        let service_process = TokioCommand::new(service_binary)
            .current_dir(runtime_dir)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()?;
        *process_guard = Some(service_process);
    }
    
    // 监听文件变更 - 添加事件去重机制
    let mut last_reload_time = std::time::Instant::now();
    
    loop {
        if let Ok(event) = rx.recv_timeout(Duration::from_millis(100)) {
            match event {
                Ok(notify::Event { kind: notify::EventKind::Modify(_), .. }) => {
                    // 事件去重：防止短时间内的重复触发（编辑器保存时通常产生多个事件）
                    let now = std::time::Instant::now();
                    let time_since_last = now.duration_since(last_reload_time);
                    // println!("🔍 DEBUG: Event received, time since last reload: {:?}", time_since_last);
                    
                    if time_since_last < Duration::from_millis(10000) {  // 增加到10秒以捕获延迟事件
                        // 如果距离上次重载不到10秒，跳过这次事件
                        println!("   ⏭ Event ignored due to deduplication (< 10000ms)");
                        continue;
                    }
                    last_reload_time = now;
                    println!("🔄 {}", "Schema change detected! Reloading...".bright_yellow());
                    
                    // 1. 停止当前服务进程
                    {
                        let mut process_guard = current_process.lock().unwrap();
                        if let Some(mut process) = process_guard.take() {
                            let _ = process.kill().await;
                            let _ = process.wait().await;
                            println!("   � Service stopped");
                        }
                    }
                    
                    // 2. 重新生成代码和编译
                    if let Err(e) = hot_reload_service(runtime_dir_clone, service_dir).await {
                        eprintln!("   ❌ Hot reload failed: {}", e);
                        continue;
                    }
                    
                    // 3. 重新启动服务进程
                    {
                        let mut process_guard = current_process.lock().unwrap();
                        match TokioCommand::new(service_binary)
                            .current_dir(runtime_dir)
                            .stdout(Stdio::inherit())
                            .stderr(Stdio::inherit())
                            .spawn() {
                            Ok(new_process) => {
                                *process_guard = Some(new_process);
                                println!("   ✅ {} {}", "Hot reload completed!".bright_green(), "Service restarted 🚀".bright_blue());
                            },
                            Err(e) => {
                                eprintln!("   ❌ Failed to restart service: {}", e);
                            }
                        }
                    }
                },
                _ => {
                    // 忽略其他类型的事件
                }
            }
        }
        
        // 检查服务进程是否还在运行
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

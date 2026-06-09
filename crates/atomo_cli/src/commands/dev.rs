use anyhow::{Context, Result};
use atomo_schema::{
    hasura_v2_type_generator::HasuraV2TypeGenerator, is_builder_dsl, parse_builder_dsl,
    HasuraV2ResolverGenerator, Model, TypeScriptParser,
};

fn parse_schema_models(content: &str) -> Result<Vec<Model>> {
    if is_builder_dsl(content) {
        let schema = parse_builder_dsl(content)?;
        Ok(schema.models.into_values().collect())
    } else {
        let parser = TypeScriptParser::new();
        parser.parse(content).map_err(Into::into)
    }
}
use colored::*;
use console::style;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::mpsc;
use std::time::Duration;
use tokio::process::Command as TokioCommand;

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
    _verify_schema_flag: bool,
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
                mode_note = format!("Using workspace mode (auto-detected at {})", root.display());
            }
            None => {
                use_workspace = false;
                mode_note = "Using isolated mode (no workspace detected)".to_string();
            }
        }
    }

    // Determine validation behavior
    // Defaults: workspace -> warn (non-strict), isolated -> strict
    let effective_verify = true; // always verify; flag reserved for future use
    let effective_strict = if strict_schema_flag {
        true
    } else {
        !use_workspace
    };

    // Dispatch to workspace flow when selected or auto-detected
    println!("   {}", mode_note.dimmed());
    if use_workspace {
        return super::workspace_dev::workspace_dev_command(
            port,
            service_path,
            effective_strict,
            effective_verify,
        )
        .await;
    }

    println!(
        "🚀 {}",
        style("Starting Atomo development server (isolated mode)...").cyan()
    );

    // Detect service directory
    let current_dir = std::env::current_dir()?;
    let service_name = detect_service_context(&current_dir)?;
    println!("   📋 Detected service: {}", service_name.bright_yellow());

    let schema_path = current_dir.join("schema.ts");
    if !schema_path.exists() {
        anyhow::bail!("❌ schema.ts not found in current directory");
    }

    // Find or build atomo-server binary
    let server_binary = find_or_build_server_binary(&current_dir).await?;
    println!(
        "   🔧 Using server: {}",
        server_binary.display().to_string().dimmed()
    );

    // Load .env from service directory
    let _ = dotenv::from_path(current_dir.join(".env"));

    // Run atomo-server with schema pointed at this service, with file watching
    run_server_with_watch(&server_binary, &schema_path, &service_name, port, &current_dir).await?;

    Ok(())
}

/// Find the atomo-server binary in the workspace target dir, or build it.
async fn find_or_build_server_binary(service_dir: &Path) -> Result<PathBuf> {
    let binary_name = if cfg!(windows) {
        "atomo-server.exe"
    } else {
        "atomo-server"
    };

    // Look in workspace target directories
    if let Some(workspace_root) = detect_workspace_root_from(service_dir)? {
        for profile in &["debug", "release"] {
            let candidate = workspace_root.join("target").join(profile).join(binary_name);
            if candidate.exists() {
                return Ok(candidate);
            }
        }

        // Not found — build it
        println!("   📦 Building atomo-server (first time only)...");
        let mut build = TokioCommand::new("cargo")
            .arg("build")
            .arg("-p")
            .arg("atomo_server")
            .arg("--bin")
            .arg("atomo-server")
            .current_dir(&workspace_root)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()?;

        let status = build.wait().await?;
        if !status.success() {
            anyhow::bail!("❌ Failed to build atomo-server");
        }

        let built = workspace_root.join("target").join("debug").join(binary_name);
        if built.exists() {
            return Ok(built);
        }
    }

    anyhow::bail!("❌ atomo-server not found. Build with: cargo build -p atomo_server --bin atomo-server")
}

/// Run atomo-server as a child process with file watching for hot reload.
async fn run_server_with_watch(
    server_binary: &Path,
    schema_path: &Path,
    service_name: &str,
    port: u16,
    service_dir: &Path,
) -> Result<()> {
    // Set up file watcher
    let (tx, rx) = mpsc::channel();
    let mut watcher = RecommendedWatcher::new(tx, notify::Config::default())?;
    watcher.watch(schema_path, RecursiveMode::NonRecursive)?;

    let schema_path_str = schema_path.display().to_string().replace('\\', "/");

    let mut last_reload_time = std::time::Instant::now();

    loop {
        // Start server process
        println!();
        println!("{}", "-".repeat(50).bright_cyan());
        println!(
            "🎉  {} {}",
            service_name.bright_green().bold(),
            "Starting...".bright_green()
        );
        println!("{}", "-".repeat(50).bright_cyan());
        println!();
        println!("   🌐 Access your service:");
        println!(
            "   ├─ 🏠 Homepage:           {}",
            format!("http://localhost:{}", port).bright_blue()
        );
        println!(
            "   ├─ 🔍 GraphQL Playground: {}",
            format!("http://localhost:{}/playground", port).bright_blue()
        );
        println!(
            "   ├─ 📊 GraphQL API:        {}",
            format!("http://localhost:{}/graphql", port).bright_blue()
        );
        println!(
            "   ├─ 🔐 Admin UI:           {}",
            format!("http://localhost:{}/admin", port).bright_blue()
        );
        println!(
            "   └─ 💚 Health Check:       {}",
            format!("http://localhost:{}/health", port).bright_blue()
        );
        println!();
        println!(
            "   🔥 {}",
            "Hot Reload: Watching schema.ts for changes..."
                .bright_yellow()
        );
        println!("{}", "─".repeat(70).yellow());

        let mut child = TokioCommand::new(server_binary)
            .env("ATOMO_SCHEMA_PATH", &schema_path_str)
            .env("PORT", port.to_string())
            .current_dir(service_dir)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()?;

        // Wait for either: process exit or schema file change
        loop {
            // Check for file change events (non-blocking)
            if let Ok(event) = rx.recv_timeout(Duration::from_millis(200)) {
                if let Ok(notify::Event {
                    kind: notify::EventKind::Modify(_),
                    ..
                }) = event
                {
                    let now = std::time::Instant::now();
                    if now.duration_since(last_reload_time) < Duration::from_secs(2) {
                        continue;
                    }
                    last_reload_time = now;

                    println!(
                        "\n🔄 {}",
                        "Schema change detected! Restarting server...".bright_yellow()
                    );

                    let _ = child.kill().await;
                    let _ = child.wait().await;
                    break; // restart the outer loop
                }
            }

            // Check if the server process exited on its own
            if let Ok(Some(status)) = child.try_wait() {
                if !status.success() {
                    anyhow::bail!("❌ atomo-server exited with error");
                }
                return Ok(());
            }
        }
    }
}

/// Shared incremental codegen entry used by both isolated and workspace modes.
/// Generates or updates models.rs, resolvers.rs, and schema.graphql under `runtime_dir/src`.
pub async fn shared_incremental_codegen(runtime_dir: &Path, schema_path: &Path) -> Result<()> {
    generate_business_code_incremental(runtime_dir, schema_path).await
}

/// Optional schema validation shared helper (OperationDefinitions + Hasura v2 checks)
pub async fn shared_validate_schema(
    runtime_dir: &Path,
    schema_path: &Path,
    strict: bool,
) -> Result<()> {
    // Ensure GraphQL schema exists/updated
    let schema_graphql_path = runtime_dir.join("schema.graphql");
    if !schema_graphql_path.exists() {
        generate_graphql_schema_definition(schema_path, &schema_graphql_path).await?;
    }

    // Validate Hasura v2 compatibility
    if let Err(e) = verify_schema_consistency(&schema_graphql_path).await {
        if strict {
            return Err(e);
        }
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


/// 生成业务代码 (models.rs, resolvers.rs) - 增强版本
/// 真正的细粒度增量代码生成 - 只重新生成变更的部分
pub(crate) async fn generate_business_code_incremental(
    runtime_dir: &Path,
    schema_path: &Path,
) -> Result<()> {
    let _models_path = runtime_dir.join("src").join("models.rs");
    let _resolvers_path = runtime_dir.join("src").join("resolvers.rs");

    // 首先生成GraphQL schema文件用于开发工具
    let schema_graphql_path = runtime_dir.join("schema.graphql");

    // 读取并解析schema
    let schema_content = tokio::fs::read_to_string(schema_path).await?;
    let models = parse_schema_models(&schema_content)
        .with_context(|| "Failed to parse schema.ts")?;

    // 检查什么需要重新生成
    let change_detection =
        detect_incremental_changes_detailed(schema_path, runtime_dir, &models).await?;

    println!("   📊 Incremental analysis:");
    println!(
        "      ├─ Models changed: {}",
        change_detection.models_changed
    );
    println!(
        "      ├─ Resolvers changed: {}",
        change_detection.resolvers_changed
    );
    println!(
        "      └─ Schema hash: {}...",
        &change_detection.schema_hash[..8]
    );

    // 始终生成GraphQL schema文件（轻量操作）
    if change_detection.models_changed || change_detection.resolvers_changed {
        println!("   📄 Generating GraphQL schema definition...");
        generate_graphql_schema_definition(schema_path, &schema_graphql_path).await?;

        // 🔧 关键优化：只重新生成实际变更的文件，支持并行生成
        match (
            change_detection.models_changed,
            change_detection.resolvers_changed,
        ) {
            (true, true) => {
                println!("   🏗️  Regenerating models.rs and resolvers.rs in parallel...");
                let models_task = generate_models_only(runtime_dir, schema_path, &models);
                let resolvers_task = generate_resolvers_only(runtime_dir, schema_path, &models);

                // 并行执行两个生成任务
                let (_models_result, _resolvers_result) =
                    tokio::try_join!(models_task, resolvers_task)?;
                println!("   ✅ Parallel generation completed successfully");
            }
            (true, false) => {
                println!("   🏗️  Regenerating models.rs...");
                generate_models_only(runtime_dir, schema_path, &models).await?;
                println!("   ✅ Resolvers unchanged - skipping");
            }
            (false, true) => {
                println!("   🔧 Regenerating resolvers.rs...");
                generate_resolvers_only(runtime_dir, schema_path, &models).await?;
                println!("   ✅ Models unchanged - skipping");
            }
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
                return Ok(Some(current.to_path_buf()));
            }
        }
        if let Some(parent) = current.parent() {
            current = parent;
        } else {
            break;
        }
    }
    Ok(None)
}


/// 生成标准GraphQL schema定义文件
async fn generate_graphql_schema_definition(schema_path: &Path, output_path: &Path) -> Result<()> {
    let schema_content = tokio::fs::read_to_string(schema_path).await?;
    let models = parse_schema_models(&schema_content)?;

    // 生成标准的GraphQL schema定义
    let type_generator = HasuraV2TypeGenerator::new();
    let graphql_schema = type_generator.generate_graphql_schema_definition(&models)?;

    tokio::fs::write(output_path, graphql_schema).await?;

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
    let required_elements = ["type Query", "type Mutation", "input", "enum"];

    for element in &required_elements {
        if !content.contains(element) {
            anyhow::bail!(
                "Schema validation failed: Missing required element '{}'",
                element
            );
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
        if line.trim().starts_with("type ") && !line.contains("Query") && !line.contains("Mutation")
        {
            if let Some(type_name) = extract_type_name(line) {
                if !type_name.ends_with("BoolExp")
                    && !type_name.ends_with("OrderBy")
                    && !type_name.ends_with("ComparisonExp")
                {
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
            anyhow::bail!(
                "Hasura v2 validation failed: Missing BoolExp type for model '{}'",
                model
            );
        }

        if !content.contains(&order_by) {
            anyhow::bail!(
                "Hasura v2 validation failed: Missing OrderBy type for model '{}'",
                model
            );
        }
    }

    Ok(())
}

/// 验证比较操作符完整性
fn verify_comparison_operators(content: &str) -> Result<()> {
    let required_operators = [
        "_eq", "_neq", "_gt", "_gte", "_lt", "_lte", "_in", "_nin", "_is_null",
    ];

    // 检查基本比较操作符类型是否存在
    let comparison_types = [
        "StringComparisonExp",
        "NumericComparisonExp", // 用于所有数字类型（Int, Float等）
        "DateTimeComparisonExp",
        "BooleanComparisonExp",
        "UUIDComparisonExp",
        "GenericComparisonExp", // 通用比较类型
    ];

    for comp_type in &comparison_types {
        if !content.contains(comp_type) {
            anyhow::bail!(
                "Schema validation failed: Missing comparison type '{}'",
                comp_type
            );
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
                if comp_type.contains("Boolean")
                    && (operator.contains("_gt")
                        || operator.contains("_lt")
                        || operator.contains("_in")
                        || operator.contains("_nin"))
                {
                    continue; // Boolean类型不需要这些操作符
                }
                if comp_type.contains("UUID")
                    && (operator.contains("_gt") || operator.contains("_lt"))
                {
                    continue; // UUID类型不需要大小比较
                }
                println!(
                    "   Info: Operator '{}' not found in type '{}' (may be expected)",
                    operator, comp_type
                );
            }
        }
    }

    // 精确验证：确保所有操作符都有下划线前缀（Hasura v2标准）
    let required_operators = [
        "_eq:",
        "_neq:",
        "_gt:",
        "_gte:",
        "_lt:",
        "_lte:",
        "_in:",
        "_nin:",
        "_is_null:",
        "_like:",
        "_ilike:",
        "_similar:",
        "_regex:",
        "_iregex:",
    ];

    // 检查是否有无下划线的操作符（使用精确模式匹配）
    let invalid_patterns = [
        "  eq:",
        "  neq:",
        "  gt:",
        "  gte:",
        "  lt:",
        "  lte:",
        "  in:",
        "  nin:",
        "  is_null:",
        "  like:",
        "  ilike:",
        "  similar:",
        "  regex:",
        "  iregex:",
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
        println!(
            "   Warning: Some standard Hasura v2 operators may be missing: {:?}",
            missing_operators
        );
    }

    println!(
        "   Info: All comparison operators follow Hasura v2 naming convention (with _ prefix)"
    );

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
    models: &[atomo_schema::types::Model],
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
    let models_changed = !models_file_exists
        || cached_models_hash != Some(current_models_hash.clone())
        || cached_schema_hash != Some(current_schema_hash.clone());

    // Resolvers 依赖于整个 schema，任何 schema 变更都需要重新生成 resolvers
    // 使用 cached_resolvers_hash 而不是 cached_schema_hash，因为我们需要检查 resolvers.hash 文件
    let resolvers_changed =
        !resolvers_file_exists || cached_resolvers_hash != Some(current_schema_hash.clone());

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
    models: &[atomo_schema::types::Model],
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


/// 更新增量缓存
async fn update_incremental_cache(
    runtime_dir: &Path,
    changes: &IncrementalChangeSet,
) -> Result<()> {
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
async fn generate_models_only(
    runtime_dir: &Path,
    _schema_path: &Path,
    models: &[atomo_schema::types::Model],
) -> Result<()> {
    let type_generator = HasuraV2TypeGenerator::new();
    let models_code = type_generator
        .generate_types(models)
        .with_context(|| "Failed to generate models")?;

    let models_path = runtime_dir.join("src").join("models.rs");
    tokio::fs::write(&models_path, models_code).await?;

    Ok(())
}

/// 只生成解析器代码
async fn generate_resolvers_only(
    runtime_dir: &Path,
    _schema_path: &Path,
    models: &[atomo_schema::types::Model],
) -> Result<()> {
    let resolver_generator = HasuraV2ResolverGenerator::new();
    let resolvers_code = resolver_generator
        .generate_resolvers(models)
        .with_context(|| "Failed to generate resolvers")?;

    let resolvers_path = runtime_dir.join("src").join("resolvers.rs");
    tokio::fs::write(&resolvers_path, resolvers_code).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    // ── detect_service_context ──────────────────────────────────────────

    #[test]
    fn detect_service_context_returns_dir_name_when_schema_exists() {
        let tmp = tempfile::tempdir().unwrap();
        let svc_dir = tmp.path().join("crm-service");
        fs::create_dir_all(&svc_dir).unwrap();
        fs::write(svc_dir.join("schema.ts"), "// schema").unwrap();

        let name = detect_service_context(&svc_dir).unwrap();
        assert_eq!(name, "crm-service");
    }

    #[test]
    fn detect_service_context_returns_dir_name_at_repo_root() {
        let tmp = tempfile::tempdir().unwrap();
        // tmp.path() itself acts as the "repo root" directory
        fs::write(tmp.path().join("schema.ts"), "// schema").unwrap();

        let name = detect_service_context(tmp.path()).unwrap();
        // Should return whatever the temp dir is named
        let expected = tmp
            .path()
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        assert_eq!(name, expected);
    }

    #[test]
    fn detect_service_context_errors_when_no_schema() {
        let tmp = tempfile::tempdir().unwrap();
        // No schema.ts created
        let result = detect_service_context(tmp.path());
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("schema.ts not found"),
            "unexpected error message: {}",
            msg
        );
    }

    // ── detect_workspace_root_from ──────────────────────────────────────

    #[test]
    fn detect_workspace_root_finds_root_from_subdirectory() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();

        // Create a workspace-style root: Cargo.toml with [workspace] + crates/
        fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/*\"]\n",
        )
        .unwrap();
        fs::create_dir_all(root.join("crates").join("my_crate").join("src")).unwrap();

        let deep = root.join("crates").join("my_crate").join("src");
        let found = detect_workspace_root_from(&deep).unwrap();
        assert_eq!(found, Some(root.to_path_buf()));
    }

    #[test]
    fn detect_workspace_root_finds_root_from_immediate_child() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();

        fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/*\"]\n",
        )
        .unwrap();
        fs::create_dir_all(root.join("crates")).unwrap();

        let child = root.join("crates");
        let found = detect_workspace_root_from(&child).unwrap();
        assert_eq!(found, Some(root.to_path_buf()));
    }

    #[test]
    fn detect_workspace_root_returns_none_without_workspace() {
        let tmp = tempfile::tempdir().unwrap();
        // A plain Cargo.toml without [workspace] and no crates/ dir
        fs::write(
            tmp.path().join("Cargo.toml"),
            "[package]\nname = \"solo\"\n",
        )
        .unwrap();

        let found = detect_workspace_root_from(tmp.path()).unwrap();
        assert_eq!(found, None);
    }

    #[test]
    fn detect_workspace_root_returns_none_when_no_crates_dir() {
        let tmp = tempfile::tempdir().unwrap();
        // Has [workspace] in Cargo.toml but no crates/ directory
        fs::write(
            tmp.path().join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/*\"]\n",
        )
        .unwrap();

        let found = detect_workspace_root_from(tmp.path()).unwrap();
        assert_eq!(found, None);
    }

    #[test]
    fn detect_workspace_root_returns_none_for_empty_tree() {
        let tmp = tempfile::tempdir().unwrap();
        // Completely empty directory — no Cargo.toml anywhere
        let found = detect_workspace_root_from(tmp.path()).unwrap();
        assert_eq!(found, None);
    }
}


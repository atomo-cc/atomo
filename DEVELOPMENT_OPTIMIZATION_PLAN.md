# Atomo 开发流程优化方案

## 🎯 目标
将当前的开发周期从 **30-60秒** 缩短到 **3-5秒**

## 🔍 当前问题分析

### 性能瓶颈
1. **完整重新生成**: 每次都删除 `.atomo/runtime` 目录
2. **粗粒度变更检测**: 只检查 `schema.ts` 时间戳
3. **Release模式编译**: 开发环境不需要完整优化
4. **单体代码生成**: 所有代码一次性重新生成

### 耗时分析
- 代码生成: ~5-10秒
- Cargo编译: ~20-50秒  
- 总计: ~30-60秒

## 💡 优化方案

### 1. 智能增量代码生成

```rust
// 建议实现的增量检测逻辑
struct ChangeDetector {
    schema_hash: String,
    models_hash: String,
    resolvers_hash: String,
    config_hash: String,
}

impl ChangeDetector {
    fn detect_changes(&self, schema_path: &Path) -> ChangeSet {
        let current_schema = hash_file(schema_path);
        
        ChangeSet {
            schema_changed: current_schema != self.schema_hash,
            models_need_regen: self.models_affected(&current_schema),
            resolvers_need_regen: self.resolvers_affected(&current_schema),
            config_need_regen: self.config_affected(&current_schema),
        }
    }
}
```

### 2. 分层代码生成

将代码生成分为独立的层次：

```
.atomo/runtime/
├── src/
│   ├── models.rs       # 模型定义 (较少变更)
│   ├── resolvers.rs    # GraphQL解析器 (经常变更)
│   ├── types.rs        # 类型定义 (较少变更)
│   └── config.rs       # 配置文件 (很少变更)
├── .cache/
│   ├── schema.hash     # Schema哈希缓存
│   ├── models.hash     # 模型哈希缓存
│   └── last_build.json # 上次构建信息
```

### 3. 开发模式优化

```toml
# 开发环境 Cargo.toml 优化
[profile.dev]
incremental = true
debug = true
opt-level = 0         # 无优化，最快编译
overflow-checks = false
lto = false

# 可选的快速编译配置
[profile.fast-dev]
inherits = "dev"
opt-level = 1
debug = false
```

### 4. 热重载机制

```rust
// 文件监听和热重载
async fn watch_and_reload(service_dir: &Path) -> Result<()> {
    let (tx, mut rx) = tokio::sync::mpsc::channel(10);
    
    // 监听文件变更
    let watcher = create_file_watcher(service_dir, tx)?;
    
    // 启动服务进程
    let mut service_process = start_service_process().await?;
    
    while let Some(change_event) = rx.recv().await {
        match change_event {
            ChangeType::SchemaOnly => {
                // 只重新生成resolver
                regenerate_resolvers().await?;
                reload_service(&mut service_process).await?;
            },
            ChangeType::ModelChange => {
                // 重新生成model和resolver
                regenerate_models_and_resolvers().await?;
                restart_service(&mut service_process).await?;
            },
            ChangeType::ConfigChange => {
                // 完整重启
                restart_service(&mut service_process).await?;
            }
        }
    }
}
```

### 5. 编译缓存优化

```rust
// 使用 sccache 或自定义缓存机制
async fn setup_build_cache(runtime_dir: &Path) -> Result<()> {
    // 设置共享编译缓存
    std::env::set_var("RUSTC_WRAPPER", "sccache");
    
    // 或者使用项目本地缓存
    std::env::set_var("CARGO_TARGET_DIR", 
        runtime_dir.join("target").to_string_lossy());
}
```

## 🛠️ 实现优先级

### Phase 1: 快速优化 (1-2天)
- [x] 切换到 `dev` 编译模式 ✅ 已完成
- [x] 实现基础的文件哈希检测 ✅ 已完成  
- [x] 分离模型和解析器生成 ✅ 已完成

### Phase 2: 增量生成 (3-5天)  
- [x] 实现智能变更检测 ✅ 已完成
- [ ] 分层代码生成
- [ ] 构建缓存优化

### Phase 3: 热重载 (1-2周)
- [ ] 文件监听机制
- [ ] 进程热重载
- [ ] WebSocket更新通知

## 📈 预期效果

| 变更类型 | 当前耗时 | 优化后耗时 | 提升倍数 |
|----------|----------|------------|----------|
| Schema微调 | 30-60s | 3-5s | 10-15x |
| 添加字段 | 30-60s | 5-8s | 6-10x |
| 配置变更 | 30-60s | 8-12s | 4-6x |
| 首次启动 | 60-90s | 20-30s | 3x |

## 🚀 即时可用的优化

在完整方案实现前，可以立即应用这些优化：

1. **修改编译模式**:
   ```rust
   // 在 compile_and_run_service 中
   TokioCommand::new("cargo")
       .arg("build")           // 移除 --release
       .arg("--bin")
       .arg("service")
   ```

2. **启用增量编译**:
   ```toml
   [profile.dev]
   incremental = true
   debug = 1
   ```

3. **缓存依赖编译**:
   ```bash
   # 预编译依赖
   cargo build --dependencies-only
   ```

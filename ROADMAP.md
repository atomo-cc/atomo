# Atomo 开发路线图 (Development Roadmap)

### 📋 Phase 1 待办事项 (Priority Order)

#### P0 - 核心基础设施
1. **完善 CLI 工具链**
   - [x] `atomo init` - 项目初始化
   - [x] `atomo generate` - 代码生成
   - [x] `atomo migrate` - 数据库迁移
   - [x] `atomo dev` - 开发服务器

2. **Schema 定义与代码生成**
   - [x] TypeScript schema 解析器 ✨ 增强版本完成
   - [x] Rust 代码生成器 ✨ 基础版本完成
   - [x] 数据库迁移生成器 ✨ 基础版本完成
   - [x] GraphQL schema 生成器 ✨ 基础版本完成

3. **基础数据层**
   - [x] PostgreSQL 连接与配置
   - [x] 基础 CRUD 操作
   - [ ] Audit log 基础设施

#### P1 - API 与 Admin UI
4. **GraphQL API**
   - [ ] 动态 schema 构建
   - [ ] 基础 CRUD resolvers
   - [ ] 认证与授权基础

5. **Admin UI 基础**
   - [ ] 元数据 API
   - [ ] 动态表单生成
   - [ ] 基础 CRUD 界面

#### P2 - 开发者体验优化
6. **端到端类型安全**
   - [ ] `atomo codegen` - 前端类型生成
   - [ ] TypeScript SDK 生成
   - [ ] React/Vue hooks 生成

7. **基础钩子系统**
   - [ ] 事件系统基础
   - [ ] Rust 插件接口
   - [ ] 生命周期钩子

### 🔄 Phase 2 预览 - 架构升维
- 迁移至事件溯源 + CQRS
- AI 集成 (pg_vector)
- WASM 插件系统
- 边缘计算投影

### 🚀 Phase 3 预览 - 生态扩张
- 业务流程编排引擎
- "解决方案即代码"市场
- Atomo Cloud 平台

## 📈 成功指标

### Phase 1 交付标准
- [ ] 开发者可以在 5 分钟内从零开始创建一个功能完整的 API
- [ ] 完整的端到端类型安全 (TS schema -> Rust -> GraphQL -> TS client)
- [ ] 生成的 Admin UI 提供所有基础 CRUD 功能
- [ ] CLI 工具提供世界级的开发者体验
- [ ] 完整的文档和示例

### 技术债务控制
- [ ] 测试覆盖率 > 80%
- [ ] 所有 Clippy 警告修复
- [ ] 性能基准测试建立
- [ ] 安全审计基础完成

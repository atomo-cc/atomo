# Atomo 开发路线图 (Development Roadmap)

## 🎯 当前状态：Phase 1 - 奠定基石

**目标**: 交付一个功能强大、体验惊艳的开源 Headless 核心，吸引首批技术信徒。

### ✅ 已完成
- [x] 基础项目结构搭建
- [x] Rust workspace 配置
- [x] 基础 CLI 框架 (`atomo_cli`)
- [x] 核心类型定义 (`atomo_core`)
- [x] 示例服务配置 (`services/crm-service`)
- [x] **架构清理完成** - 平台与业务代码完全分离
- [x] **TypeScript schema 解析器** - 支持接口、枚举、类型别名
- [x] **Rust 代码生成器** - 自动生成模型和事件类型
- [x] **GraphQL schema 生成器** - 完整的 CRUD 操作定义
- [x] **CLI 工具链核心功能** - generate、migrate、dev 命令正常工作

### ✅ 刚完成的重大成就
- [x] **双模定义系统** (`schema.ts` -> Rust 代码生成) ✨ 完成
- [x] **智能迁移 CLI** (`atomo generate` 和 `atomo migrate`) ✨ 完成  
- [x] **动态 GraphQL API** 基础架构 ✨ **DONE! 解析器自动生成完成**
- [x] **GraphQL 解析器自动生成** - 完整的 CRUD 操作 (863行代码) ✨ **DONE!**
- [x] **完整的代码生成流水线** - TypeScript → Rust Models → GraphQL Schema → Resolvers ✨ **DONE!**
- [x] **GraphQL Schema SDL 生成** - 从schema.ts动态生成完整GraphQL定义 ✨ **DONE!**
- [x] **架构分离修正** - 移除硬编码CRM类型，实现平台/服务正确分离 ✨ **DONE!**
- [x] **动态实体查询集成** - Phase 1 基础版本，支持动态查询CRM实体 ✨ **DONE!**

### 🚧 下一步重点任务

**立即开始的任务 (优先级排序):**

1. **验证和测试动态GraphQL查询** 🚀 **<-- 当前重点**
   - ✅ 集成完成：GraphQL现在包含schemaModels, entities等动态查询
   - 🔧 需要测试：验证数据库查询是否正常工作
   - 📊 需要数据：运行迁移创建测试数据进行验证

2. **完善数据库字段映射** 🔧
   - 修复生成代码中的数据类型映射
   - 确保 SQL 查询与实际数据库表结构匹配
   - 添加枚举类型支持

3. **优化开发者体验** ✨
   - 清理编译警告
   - 改进错误处理和日志
   - 增强 CLI 输出格式

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
   - [x] Audit log 基础设施

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

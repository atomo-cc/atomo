---
title: 路线图
description: Atomo 的实施状态与未来里程碑（权威版本）。
---

# 路线图

本页是 Atomo 的唯一权威路线图，汇总实施状态与接下来要做的工作。长期愿景与架构请参见“愿景与架构”。

- 愿景：/zh/vision

## 状态总览

- CLI 与开发运行时：已实现（init、migrate、codegen、dev、dev --workspace）
- Schema → Rust/GraphQL/代码生成：已实现，支持热重载
- GraphQL API：已实现，并与平台查询合并
- Admin UI：动态渲染核心已实现；工作区模式下代理
- 认证（JWT + RBAC）：已实现；密码哈希为开发占位（见说明）
- 审计日志：已实现（REST 端点 + 平台 GraphQL）
- TypeScript SDK：已实现（类型与 React hooks）
- WASM 插件运行时：清单与权限类型已实现；运行时执行集成待完成
- 实时协作：基础已具备；WebSocket/CRDT 集成待完成

## 已交付亮点

- 开发运行时
  - 生成 `.atomo/runtime`、增量编译与热重载
  - 工作区模式监听核心 crate 与服务 schema；代理 Admin UI 到 `/admin`
- GraphQL 与元数据
  - 合并服务与平台 schema（用户、会话、审计）
  - 提供 `/meta/schema` 与开发时 `GET /schema.ts`
- GraphQL IDE：服务下 `/graphql`；工作区模式 `/playground`
- 认证与会话
  - JWT 签发/校验；会话存储于 Postgres
  - 角色模型（Admin/Manager/Sales/Support/Viewer）与 RBAC 校验
  - REST：`/auth/login`、`/auth/logout`、`/auth/me`
- 审计
  - REST：`/audit/logs`、`/audit/user/:id/activity`、`/audit/entity/:type/:id/audit`、`/audit/statistics`
  - 平台 GraphQL 查询：用户与会话
- SDK
  - 在 `packages/atomo-client-sdk` 生成类型与 React hooks 脚手架

## 进行中 / 近期计划

- WASM 运行时执行与沙箱（wasmtime 集成）
- 生产级密码哈希（bcrypt/argon2）与策略
- 实时订阅与协作（WebSocket/CRDT 接线）
- AI 集成（pgvector、内容理解 API）
- 安全加固、权限矩阵与多租户支持

## 文档 vs 代码（真实状态）

- 密码哈希
  - 代码：当前为开发占位实现
  - 文档：在认证章节已明确；生产启用 bcrypt/argon2
- 限流
  - 文档：API 概览中提供了示例头
  - 代码：服务尚未内置限流 → 未来工作
- WASM 插件
  - 代码：存在 `PluginManifest`/`Permission`/`PluginContext`
  - 文档：已记录清单与权限；执行运行时待完成

## 阶段（高层级）

### 第 1 阶段 — 开发者体验核心（4–6 个月）
- P0 核心基础设施（大部分完成）
  - CLI 工具链（`init`、`generate`、`migrate`、`dev`、`codegen`、`dev --workspace`）
  - 双模定义（TS → Rust/GraphQL），支持热重载
  - 事件友好数据层与审计日志
- P1 动态 API 与 Admin UI（基本完成）
  - 动态 GraphQL API（schema 合并、CRUD 解析器）
  - 认证与授权（JWT + RBAC）、元数据 API
  - 基于 Schema 的动态渲染引擎
- P2 可扩展性与 AI 基础（部分）
  - Hook/Access DSL 与插件接口
  - WASM 运行时脚手架
  - AI 基础（pgvector、内容 API）

### 第 2 阶段 — 认知与边缘（6–9 个月）
- ES/CQRS 成熟度（回放、可观测、运维预案）
- 本地优先同步（SDK alpha）、实时订阅
- 后端 WASM 运行时（权限与沙箱）
- 边缘投影（Workers/Vercel KV）、相似度检索

### 第 3 阶段 — 生态与解决方案（8–12 个月）
- 可视化流程编排，内置 AI 节点
- “解决方案即代码”市场与官方模板
- 企业级特性（RBAC/ABAC、SSO、多租户）与 Atomo Cloud 发布

## 成功指标与质量门槛
- 测试：覆盖率 > 85%；关键路径 100%
- 性能：核心路径较常见 Node.js 方案快 3–5×
- 安全：通过独立安全审计
- 可靠性：≥ 99.9% 服务可用性
- 社区：GitHub 影响与模板采用率

## 里程碑时间线（建议）
- 2024 Q4 — 第 1 阶段启动；首个完整 CRM 演示
- 2025 Q1–Q2 — 第 1 阶段完成；开源发布与社区建设
- 2025 Q3–Q4 — 第 2 阶段执行；Atomo Cloud 私测
- 2026 Q1+ — 第 3 阶段扩展；解决方案市场成熟

## 下一步里程碑

- 安全
  - 切换密码哈希为 bcrypt/argon2；增加迁移与配置门禁
  - 强化认证中间件与权限模式
- 运行时与 DX
  - 集成 wasmtime 沙箱与权限检查
  - 稳定更广范围的热重载；优化防抖
- 协作
  - 启用 GraphQL 订阅 / WebSocket 通道
  - 在合适场景引入 CRDT 支撑
- 可观测
  - 结构化日志、Tracing spans、请求 ID
  - 可选限流与指标端点

若评估 Atomo，可先阅读“指南”中的开发流程；若需平台理念与架构，请参阅“愿景与架构”。


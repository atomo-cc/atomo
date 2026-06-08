# Atomo CRM Service

这是基于 Atomo 平台构建的客户关系管理(CRM)服务。它展示了如何使用 Atomo 作为底层平台来快速构建功能完整的业务应用。

## 🚀 快速开始

### ⚠️ 重要：启动位置要求

**开发服务器必须在服务目录(`services/crm-service`)中启动，而不是项目根目录。**

这是因为Atomo CLI需要：
- 在当前目录找到`schema.ts`文件来加载数据模型
- 读取`atomo.config.ts`来获取服务配置
- 应用服务特定的设置和插件

### 启动开发服务器

```bash
# ✅ 正确方式 - 修改相关代码后需要执行 以下即可，不要拆分，直接一行使用： 
```
cd C:\Users\Chris\Projects\atomo; cargo build --release; cd C:\Users\Chris\Projects\atomo\services\crm-service; C:\Users\Chris\Projects\atomo\target\release\atomo-cli.exe dev
```

# ❌ 错误方式 - 在项目根目录启动会失败
cd ../../  # 项目根目录
cargo run --bin atomo-cli -- dev  # 会报错：schema.ts not found
```

### 访问服务

启动成功后可访问：
- **GraphQL Playground**: http://localhost:3000/graphql
- **管理后台**: http://localhost:3000/admin
- **API文档**: http://localhost:3000/docs

### 当前推荐方式

```bash
# From repo root
pnpm dev:admin
pnpm --filter @atomo-cc/client-sdk dev
pnpm --filter atomo-crm-service generate
```

CRM demo 的 source of truth 是 `services/crm-service/schema.ts`。修改 schema 后先重新生成 CRM artifacts，再构建 SDK：

```bash
pnpm --filter atomo-crm-service generate
pnpm --filter @atomo-cc/client-sdk build
```

前端/SDK 基线应保持通过：`pnpm --filter "./packages/*" test` 会验证 Admin UI 与 TypeScript SDK。

### 访问服务

- **Admin UI dev server**: http://localhost:5173
- **GraphQL/API routes**: 取决于当前 `atomo dev`/workspace runtime 配置；优先参考 `docs/guide/dev-runtime.md`

## 🎯 架构概览

```
services/crm-service/
├── atomo.config.ts     # ⚙️  Atomo 平台配置
├── schema.ts           # 📊  CRM 数据模型定义
├── workflows/          # 🔄  业务流程自动化
├── plugins/            # 🧩  业务扩展插件
├── admin/              # 🎨  后台界面定制
└── Dockerfile          # 🐳  部署配置
```

## 🚀 核心理念

**CRM 不是一个独立的应用，而是 Atomo 平台的一个配置实例。**

- **后端接口**: 由 `atomo_server` 根据 `schema.ts` 自动生成
- **数据库**: 由 Atomo 根据模型定义自动创建和迁移
- **GraphQL API**: 完全自动生成，包含所有 CRUD 操作
- **后台界面**: 基于 `atomo-admin-ui` 自动渲染，支持定制
- **业务逻辑**: 通过 actions、外部 workers 和工作流扩展

## 📊 数据模型

### 核心实体

- **Contact (联系人)**: 客户的基本信息和联系方式
- **Company (公司)**: 客户所属的公司或组织
- **Deal (商机)**: 销售机会和交易记录

### 可组合内容块

所有富文本字段（notes, description）支持 Atomo 的"流动画布"内容块：

- `ParagraphBlock`: 普通文本段落
- `CallLogBlock`: 通话记录
- `MeetingNoteBlock`: 会议纪要
- `TaskBlock`: 任务和待办事项

## 🛠️ 开发工作流

### 1. 启动开发环境

```bash
# 在项目根目录运行
pnpm dev --service crm

pnpm dev:admin
pnpm --filter @atomo-cc/client-sdk dev
pnpm --filter atomo-crm-service generate
```

这个命令会：
- 启动 `atomo_server` 并加载 CRM 配置
- 自动创建/迁移数据库表
- 生成完整的 GraphQL API
- 启动后台管理界面
- 启动 action 分发器

- 启动 Admin UI 开发服务器
- 让 TypeScript SDK 进入 watch/build 循环
- 基于 CRM schema 重新生成当前 demo artifacts

### 2. 访问服务
- **GraphQL API**: http://localhost:8080/graphql
- **后台管理**: http://localhost:3000
- **API 文档**: http://localhost:8080/graphql (GraphQL Playground)

- **后台管理**: http://localhost:5173
- **API / Playground**: 运行 workspace runtime 时按 `docs/guide/dev-runtime.md` 中的端口和路由访问

### 3. 修改数据模型
编辑 `schema.ts` 文件，保存后：
- 数据库自动迁移
- GraphQL schema 自动更新
- 后台界面自动重新渲染

编辑 `schema.ts` 文件，保存后：

- 运行 `pnpm --filter atomo-crm-service generate`
- 运行 `pnpm --filter @atomo-cc/client-sdk build`
- 在 Admin UI dev server 中检查 schema/metadata 消费效果

### 4. 添加业务逻辑

在 `plugins/` 目录下创建 TypeScript 文件：

```typescript
// plugins/validate-email/index.ts
import { onEvent } from "@atomo-cc/plugin-sdk";

onEvent("Contact.Created", async (event) => {
  const { email } = event.payload;
  if (email && !isValidEmail(email)) {
    throw new Error("Invalid email address");
  }
});
```

事件触发后 action 分发器会自动将任务入队，由外部 worker 处理。

## 🎨 后台界面定制

### 主题定制

编辑 `admin/theme.ts` 来自定义：

- 品牌颜色
- 字体和排版
- 布局样式
- 组件外观

### 自定义组件

在 `admin/components/` 目录下创建 React 组件：

```typescript
// admin/components/CustomDealView.tsx
export function CustomDealPipelineView() {
  // 自定义看板视图
}
```

组件会自动集成到后台界面中。

## 📌 使用看板与时间线

### 商机看板 (Kanban)

- 导航到“商机看板”或访问 `http://localhost:3000/deals/board`
- 将卡片拖到不同列以变更阶段；在同一列拖动可调整顺序
- 看板顺序通过 `Deal.position` 字段持久化，跨列移动会批量更新

### 联系人时间线

- 在联系人详情中点击“查看时间线”，或访问 `/contacts/:id/timeline`
- 顶部可快速添加“备注”或“活动”（通话、会议、邮件、任务）
- 时间线合并显示联系人备注(Blocks)与 Activity 记录，并按时间倒序排列

> 提示：你可以扩展 Activity 的 `metadata` 字段来存储结构化数据（例如通话时长、会议参与者）。

<!-- 截图占位符：放置到 admin 截图目录 -->
<!-- ![Deals Kanban](./admin/screenshots/deals-kanban.png) -->
<!-- ![Contact Timeline](./admin/screenshots/contact-timeline.png) -->

## 🔄 工作流自动化

在 `workflows/` 目录下定义业务流程：

```yaml
# workflows/sales-pipeline.yml
name: "Sales Pipeline Automation"
triggers:
  - event: "Deal.Updated"
    conditions:
      - field: "stage"
        changed: true

steps:
  - name: "send_notification"
    type: "action"
    action: |
      await sendEmail({
        to: "sales@company.com",
        subject: `Deal ${deal.title} moved to ${deal.stage}`
      });
```

## 🧪 测试和数据

### 种子数据

有两种方式：

1. 使用 Atomo CLI（读取当前服务目录 `.env` 的 `DATABASE_URL`）

```bash
cd services/crm-service
../../target/debug/atomo-cli seed           # 使用默认 ./seed.sql
../../target/debug/atomo-cli seed --file ./seed-demo.sql
```

2. 使用 SQL 脚本（psql）（推荐用于演示数据）

```bash
export DATABASE_URL=postgresql://user:pass@localhost:5432/atomo_dev
pnpm seed:sql --filter ./services/crm-service
```

脚本会重置并插入一组完整演示数据：

- 4 个公司和 4 个联系人，联系人通过 `company_id` 关联公司
- 12 个商机，覆盖 `lead`、`qualified`、`proposal`、`negotiation`、`won`、`lost` 六个阶段
- 每个阶段使用从 0 开始的 `deal.position`，用于看板列内排序
- 6 条 Activity 时间线记录，覆盖 note、call、meeting、email、task 类型

> 提示：`deal.position` 为数值型（NUMERIC），与代码生成类型保持一致；看板拖拽后通过批量变更 mutation `updateDealPositions` 持久化顺序。

### 演示检查脚本

1. 运行 seed 后打开 Admin UI，先进入 Contacts，确认 John/Jane/Peter/Maya 都显示公司。
2. 进入 Companies，打开任意公司并检查关联联系人和商机。
3. 进入 Deals/Kanban，确认六个阶段都有卡片，且同列卡片按 `position` 从小到大排列。
4. 将一个 Deal 拖到另一列，再刷新页面确认阶段和顺序保持。
5. 打开联系人时间线，确认联系人 notes 与 Activity 记录按时间倒序一起展示。

### 单元测试

```bash
pnpm --filter atomo-crm-service test
```

测试数据模型、插件和工作流。

## 🚀 部署

### 开发环境部署

```bash
pnpm build --service crm
docker build -t my-company/crm-service ./services/crm-service
docker run -p 8080:8080 my-company/crm-service
```

### 生产环境部署

```bash
pnpm deploy --service crm --env production
```

部署流程仍在整理中。当前文档只把 Admin UI + SDK + CRM demo 作为 MVP 目标；生产构建、镜像发布和托管环境部署需要在对应 CLI/server 流程稳定后再补充。

## 📈 扩展能力

### 添加新模型

1. 在 `schema.ts` 中定义新的接口
2. 添加到 `schema.models` 配置中
3. 重启开发服务器

### 集成外部 API

在 `plugins/` 目录下创建集成插件：

```typescript
// plugins/integrate-salesforce/index.ts
onEvent("Deal.Updated", async (event) => {
  await syncToSalesforce(event.payload);
});
```

### 自定义权限

在 `atomo.config.ts` 中配置：

```typescript
export default defineConfig({
  auth: {
    permissions: {
      sales_manager: ["deals:*", "contacts:*"],
      sales_rep: ["deals:read", "contacts:read"],
    },
  },
});
```

## 🛡️ 安全考虑

- 所有 API 访问都需要身份验证
- 支持基于角色的权限控制
- 自动的 SQL 注入防护
- 敏感数据字段加密
- 完整的操作审计日志

## 🔮 未来计划

- [ ] 集成企业微信/钉钉
- [ ] AI 智能客户分析
- [ ] 高级报表和仪表板
- [ ] 移动端支持
- [ ] 事件溯源架构升级

## 📞 支持

如有问题，请联系 Atomo 技术团队或查看文档：

- 📖 [Atomo 平台文档](https://docs.atomo.dev)
- 💬 [技术支持论坛](https://forum.atomo.dev)
- 📧 support@atomo.dev

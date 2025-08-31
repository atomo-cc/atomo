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

## 🎯 架构概览

```
services/crm-service/
├── atomo.config.ts     # ⚙️  Atomo 平台配置
├── schema.ts           # 📊  CRM 数据模型定义
├── workflows/          # 🔄  业务流程自动化
├── plugins/            # 🧩  WASM 业务插件
├── admin/              # 🎨  后台界面定制
└── Dockerfile          # 🐳  部署配置
```

## 🚀 核心理念

**CRM 不是一个独立的应用，而是 Atomo 平台的一个配置实例。**

- **后端接口**: 由 `atomo_server` 根据 `schema.ts` 自动生成
- **数据库**: 由 Atomo 根据模型定义自动创建和迁移
- **GraphQL API**: 完全自动生成，包含所有 CRUD 操作
- **后台界面**: 基于 `atomo-admin-ui` 自动渲染，支持定制
- **业务逻辑**: 通过 WASM 插件和工作流扩展

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
```

这个命令会：
- 启动 `atomo_server` 并加载 CRM 配置
- 自动创建/迁移数据库表
- 生成完整的 GraphQL API
- 启动后台管理界面
- 热加载 WASM 插件

### 2. 访问服务

- **GraphQL API**: http://localhost:8080/graphql
- **后台管理**: http://localhost:3000
- **API 文档**: http://localhost:8080/graphql (GraphQL Playground)

### 3. 修改数据模型

编辑 `schema.ts` 文件，保存后：
- 数据库自动迁移
- GraphQL schema 自动更新
- 后台界面自动重新渲染

### 4. 添加业务逻辑

在 `plugins/` 目录下创建 TypeScript 文件：

```typescript
// plugins/validate-email/index.ts
import { onEvent } from '@atomo/plugin-sdk';

onEvent('Contact.Created', async (event) => {
  const { email } = event.payload;
  if (email && !isValidEmail(email)) {
    throw new Error('Invalid email address');
  }
});
```

插件会自动编译为 WASM 并热加载。

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

```bash
pnpm seed --service crm
```

自动创建示例的联系人、公司和商机数据。

### 单元测试

```bash
pnpm test --service crm
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

## 📈 扩展能力

### 添加新模型

1. 在 `schema.ts` 中定义新的接口
2. 添加到 `schema.models` 配置中
3. 重启开发服务器

### 集成外部 API

在 `plugins/` 目录下创建集成插件：

```typescript
// plugins/integrate-salesforce/index.ts
onEvent('Deal.Updated', async (event) => {
  await syncToSalesforce(event.payload);
});
```

### 自定义权限

在 `atomo.config.ts` 中配置：

```typescript
export default defineConfig({
  auth: {
    permissions: {
      'sales_manager': ['deals:*', 'contacts:*'],
      'sales_rep': ['deals:read', 'contacts:read']
    }
  }
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

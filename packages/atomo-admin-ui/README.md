# Atomo Admin UI

基于《Atomo Admin UI 开发蓝图》实施的下一代动态管理界面。

## 📋 项目概述

Atomo Admin UI 是一个"活的"、可无限进化的业务操作台，它不是静态的后台模板，而是一个能深度适应各种业务需求的智能操作系统。

### 🎯 核心特性

- **Schema 驱动，动态渲染** - 基于元数据自动生成界面，与后端模型绝对同步
- **默认美观，深度可定制** - 开箱即用的美观界面，支持从品牌化到WASM插件的全方位定制
- **性能优先** - 虚拟化列表、代码分割、极致响应速度
- **开发者体验至上** - 类型安全、HMR、清晰的组件API

## 🏗️ 架构设计

### 三层蛋糕 UI 构建策略

1. **地基层 - Tailwind CSS**: 原子化样式、响应式设计
2. **骨架层 - Radix UI**: 无障碍、功能完备的组件逻辑
3. **精装修层 - Atomo Components**: 风格化的产品组件库

### 核心组件架构

```
src/
├── components/
│   ├── DynamicRenderer.tsx      # 🧠 核心渲染引擎
│   ├── Navigation.tsx           # 🧭 动态导航
│   ├── views/
│   │   ├── Dashboard.tsx        # 📊 仪表盘视图
│   │   ├── EntityListView.tsx   # 📋 动态列表视图
│   │   └── EntityDetailView.tsx # 📄 动态详情视图
│   ├── forms/
│   │   ├── DynamicForm.tsx      # 📝 智能表单引擎
│   │   ├── FormField.tsx        # 🔧 动态字段组件
│   │   ├── BlocksEditor.tsx     # ✨ 富文本块编辑器
│   │   ├── ReferenceSelect.tsx  # 🔗 关联数据选择器
│   │   └── TagInput.tsx         # 🏷️ 标签输入组件
│   ├── tables/
│   │   └── EntityTable.tsx      # 📊 虚拟化数据表格
│   └── ui/                      # 🎨 基础UI组件库
├── lib/
│   ├── api.ts                   # 🌐 统一API客户端
│   ├── types.ts                 # 📐 类型系统
│   ├── validation.ts            # ✅ 验证规则生成器
│   └── utils.ts                 # 🛠️ 工具函数
└── design-tokens.ts             # 🎨 设计令牌系统
```

## 🚀 已实现功能 (Phase 1)

### ✅ 核心渲染引擎
- [x] DynamicRenderer - 基于路由的智能组件选择
- [x] Schema元数据驱动的界面生成
- [x] 错误边界和加载状态管理

### ✅ 数据展示组件
- [x] Dashboard - 自动生成的仪表盘概览
- [x] EntityListView - 支持搜索、排序、分页的列表视图
- [x] EntityTable - 虚拟化大数据量表格渲染
- [x] EntityDetailView - 多标签页详情/编辑界面

### ✅ 表单系统
- [x] DynamicForm - 基于Schema自动生成表单
- [x] FormField - 智能字段类型识别和渲染
- [x] 支持所有基础数据类型（string, number, boolean, date, etc.）
- [x] ReferenceSelect - 异步关联数据选择器
- [x] TagInput - 智能标签输入组件
- [x] BlocksEditor - Atomo流动画布富文本编辑器

### ✅ UI组件库
- [x] 基于Radix UI + Tailwind的完整组件系统
- [x] Button, Input, Textarea, Select, Switch, Checkbox等基础组件
- [x] Card, Tabs, Dialog等布局组件
- [x] 统一的设计语言和交互模式

### ✅ 开发体验
- [x] TypeScript类型安全
- [x] React Query数据管理
- [x] React Hook Form表单状态管理
- [x] Zod运行时验证
- [x] 统一的错误处理

## 🔧 技术栈

- **前端框架**: React 18 + TypeScript
- **构建工具**: Vite
- **样式系统**: Tailwind CSS + Design Tokens
- **组件基础**: Radix UI (无障碍)
- **状态管理**: React Query + React Hook Form
- **路由**: React Router v6
- **验证**: Zod
- **图标**: Lucide React
- **虚拟化**: TanStack Virtual

## 📦 安装与使用

```bash
# 安装依赖
pnpm install

# 开发模式
pnpm dev

# 构建生产版本
pnpm build

# 类型检查
pnpm type-check

# 代码格式化
pnpm format
```

`pnpm type-check` should stay green. From the repo root, `pnpm --filter "./packages/*" test` verifies the Admin UI and TypeScript SDK baseline.

## 🎨 定制指南

### 1. 修改设计令牌

编辑 `design-tokens.ts` 文件来自定义颜色、字体、间距等设计属性：

```typescript
export const colors = {
  primary: {
    500: '#your-brand-color',
    // ...
  }
}
```

### 2. 添加自定义字段组件

在 `FormField.tsx` 中添加新的字段类型支持：

```typescript
case 'your-custom-type':
  return <YourCustomComponent {...fieldProps} />
```

### 3. 扩展表格列渲染

在 `EntityListView.tsx` 中自定义列渲染逻辑：

```typescript
render: (value: any, row: EntityData) => {
  // 自定义渲染逻辑
}
```

## 🔮 后续规划 (Phase 2 & 3)

### Phase 2: 高级组件与开发者体验
- [ ] WASM UI插件运行时
- [ ] 高级筛选器和搜索
- [ ] 媒体上传组件
- [ ] 实时协作功能

### Phase 3: 工作空间与智能化
- [ ] 工作流监控界面
- [ ] 事件河流可视化
- [ ] AI辅助功能
- [ ] 全局知识库搜索

## 🤝 贡献指南

1. Fork 项目
2. 创建功能分支 (`git checkout -b feature/amazing-feature`)
3. 提交更改 (`git commit -m 'Add some amazing feature'`)
4. 推送到分支 (`git push origin feature/amazing-feature`)
5. 创建 Pull Request

## 📄 许可证

本项目采用 MIT 许可证。详见 [LICENSE](../../LICENSE) 文件。

---

**Atomo Admin UI** - 让管理界面的创建变得前所未有的简单和强大。

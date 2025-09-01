# Atomo TypeScript DSL for Hooks and Access Control

## 🚀 类型安全 Hook 和 Access Control 系统

Atomo 实现了一个革命性的 TypeScript DSL，用于定义模型的业务逻辑和权限控制。我们提供了**端到端的类型安全**和**编译时错误检查**。

## ✨ 核心特性

### 🔒 类型安全的 Access Control
- **编译时类型检查**：所有用户对象、查询条件都是强类型的
- **类型安全的查询构建器**：`access.where('field').equals(value)` 替代脆弱的字符串对象
- **智能自动补全**：VS Code 提供完整的智能提示

### 🪝 强类型的 Hook 系统
- **精确的上下文类型**：`context.data`、`context.user` 都是完全类型化的
- **细粒度的钩子**：`hooks.change('field', ...)` 监听特定字段变化
- **异步支持**：天然支持 async/await 模式

### 🛡️ 运行时安全
- **自动代码生成**：从 TypeScript DSL 生成 Rust 执行代码
- **编译时验证**：拼写错误、类型不匹配立即发现
- **可测试性**：所有逻辑都是标准的 TypeScript 函数，易于单元测试

## 📚 使用示例

### 基础模型定义

```typescript
// schema.ts
export interface Product {
  id: string;
  title: string;
  description: string;
  price: number;
  priceInCents: number;
  status: ProductStatus;
  sellerId: string;
  seller?: User;
  createdAt: Date;
  updatedAt: Date;
}

export enum ProductStatus {
  DRAFT = "draft",
  PUBLISHED = "published", 
  SOLD = "sold"
}
```

### Hook 和 Access Control DSL

```typescript
export const ProductModel = defineModel({
  // 🔒 类型安全的权限控制
  access: {
    // 只有登录用户可以创建商品
    create: ({ user }: access.Context<User>) => !!user,

    // 已发布的商品任何人可读，草稿只有卖家可读
    read: ({ user }: access.Context<User>) => {
      if (user) {
        return access.or(
          access.where('status').equals('published'),
          access.where('sellerId').equals(user.id)
        );
      }
      return access.where('status').equals('published');
    },

    // 只有卖家可以更新和删除
    update: ({ user }: access.Context<User>) => 
      access.where('sellerId').equals(user.id),
    delete: ({ user }: access.Context<User>) => 
      access.where('sellerId').equals(user.id),
  },

  // 🪝 业务逻辑钩子
  hooks: {
    // 创建前：价格转换为分
    beforeOperation: [
      hooks.create(async (context: hooks.OperationContext<Product, User>) => {
        if (context.operation === 'create') {
          context.data.priceInCents = context.data.price * 100;
          delete context.data.price;
        }
      }),
    ],

    // 创建后：发送通知
    afterOperation: [
      hooks.create(async (context: hooks.OperationContext<Product, User>) => {
        if (context.operation === 'create') {
          await slack.sendMessage('#new-products', 
            `New product: ${context.result.title}`);
        }
      }),
    ],

    // 字段变化监听：状态发布前验证
    beforeChange: [
      hooks.change('status', async (context: hooks.ChangeContext<ProductStatus, Product>) => {
        if (context.value === 'published' && !context.originalDoc.description) {
          context.addValidationError('Description required for publishing', 'description');
        }
      }),
    ],

    // 读取后：数据转换和虚拟字段
    afterRead: [
      hooks.read(async (doc: Product) => ({
        ...doc,
        price: doc.priceInCents / 100,
        seller: doc.seller ? {
          ...doc.seller,
          displayName: `${doc.seller.firstName} ${doc.seller.lastName}`,
        } : null,
      })),
    ],
  },
});
```

## 🔧 代码生成

### 1. 生成类型安全的执行代码

```bash
atomo codegen -o ./generated
```

这会生成：
- `hooks_access.rs` - Rust 执行代码
- `dsl-types.ts` - TypeScript DSL 类型定义
- `types.ts` - 模型类型定义

### 2. 生成的 Rust 代码结构

```rust
// hooks_access.rs

/// 访问控制上下文
pub struct AccessContext {
    pub user: Option<Value>,
    pub operation: String,
    pub resource_id: Option<String>,
}

/// Product 模型的访问控制
pub struct ProductAccessControl;

impl ProductAccessControl {
    pub fn check_create(context: &AccessContext) -> Result<bool> {
        let user = context.user.as_ref();
        Ok(user.is_some())
    }

    pub fn check_read(context: &AccessContext) -> Result<AccessQuery> {
        Ok(AccessQuery::or(vec![
            AccessQuery::where_clause("status", "equals", serde_json::json!("published")),
            AccessQuery::where_clause("sellerId", "equals", 
                context.user.as_ref()
                    .and_then(|u| u.get("id"))
                    .cloned()
                    .unwrap_or(serde_json::Value::Null))
        ]))
    }
}

/// Product 模型的钩子
pub struct ProductHooks;

impl ProductHooks {
    pub async fn before_operation_0(context: &mut HookContext) -> Result<HookResult> {
        // 价格转换逻辑
        Ok(HookResult { success: true, data: Some(context.data.clone()), errors: Vec::new() })
    }

    pub async fn after_operation_0(context: &mut HookContext) -> Result<HookResult> {
        // 通知发送逻辑  
        Ok(HookResult { success: true, data: None, errors: Vec::new() })
    }
}
```

### Atomo 示例（安全）
```typescript
// ✅ 编译时检查：字段名错误立即发现
access: {
  read: ({ user }: access.Context<User>) => 
    access.where('seller').equals(user.id)  // 编译错误：字段不存在
}
```

## 🚀 快速开始

1. **定义模型和 DSL**
   ```typescript
   // schema.ts
   export const MyModel = defineModel({
     access: { /* 权限定义 */ },
     hooks: { /* 业务逻辑 */ }
   });
   ```

2. **生成代码**
   ```bash
   atomo codegen
   ```

3. **集成到应用**
   ```rust
   use crate::generated::hooks_access::*;
   
   let runtime = HookAccessRuntime::new();
   let allowed = runtime.check_access("Product", "create", &context)?;
   ```

## 🔮 未来规划

- [ ] **可视化编辑器**：拖拽式 Hook 和 Access 规则编辑
- [ ] **高级查询**：支持 JSON 查询、地理位置等复杂条件
- [ ] **性能优化**：查询计划优化、缓存策略
- [ ] **多语言支持**：生成 Go、Python 等多种语言的执行代码

## 📖 深入学习

- [Hook 系统详解](./docs/hooks.md)
- [Access Control 指南](./docs/access-control.md)
- [类型系统原理](./docs/type-system.md)
- [性能优化建议](./docs/performance.md)

---

**Atomo** - 下一代内容管理平台，用类型安全重新定义业务逻辑。

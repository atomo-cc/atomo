# Atomo Client SDK

一个为Atomo平台提供完整类型安全的TypeScript SDK。

## 特性

✨ **完全类型安全** - 端到端TypeScript支持，从schema.ts到前端组件  
🚀 **零配置** - 自动从GraphQL端点生成类型  
🎯 **React友好** - 内置hooks和组件支持  
📦 **模块化设计** - 按需导入，减少bundle大小  
🔄 **实时更新** - 支持GraphQL订阅  
🛠️ **开发者体验** - 完整的智能提示和错误检查  

## 安装

```bash
npm install @atomo/client-sdk
# 或
pnpm add @atomo/client-sdk
```

如果使用React hooks：
```bash
npm install @atomo/client-sdk react
```

## 快速开始

### 1. 创建客户端实例

```typescript
import { AtomoClient } from '@atomo/client-sdk';

const client = new AtomoClient({
  endpoint: 'http://localhost:3000/graphql',
  authToken: 'your-auth-token', // 可选
  timeout: 30000 // 可选，默认30秒
});
```

### 2. 基础使用

```typescript
// 获取联系人
const contact = await client.getContact('contact-id');
console.log(contact.data?.firstName);

// 创建联系人
const newContact = await client.createContact({
  firstName: 'John',
  lastName: 'Doe', 
  email: 'john@example.com'
});

// 查询联系人列表
const contacts = await client.listContacts({
  page: 1,
  limit: 20,
  where: {
    firstName: { contains: 'John' }
  },
  orderBy: 'createdAt',
  orderDirection: 'desc'
});
```

### 3. React Hooks (推荐用法)

```tsx
import { 
  AtomoProvider, 
  useContacts, 
  useCreateContact 
} from '@atomo/client-sdk/hooks';

// App组件
function App() {
  return (
    <AtomoProvider client={client}>
      <ContactsList />
    </AtomoProvider>
  );
}

// 联系人列表组件
function ContactsList() {
  const { 
    data: contacts, 
    loading, 
    error, 
    refetch 
  } = useContacts({
    page: 1,
    limit: 20
  });
  
  const { 
    execute: createContact, 
    loading: creating 
  } = useCreateContact();

  const handleCreate = async () => {
    try {
      await createContact({
        firstName: 'Jane',
        lastName: 'Smith',
        email: 'jane@example.com'
      });
      refetch(); // 自动刷新列表
    } catch (error) {
      console.error('创建失败:', error);
    }
  };

  if (loading) return <div>加载中...</div>;
  if (error) return <div>错误: {error.message}</div>;

  return (
    <div>
      <button onClick={handleCreate} disabled={creating}>
        {creating ? '创建中...' : '新建联系人'}
      </button>
      
      <ul>
        {contacts.map(contact => (
          <li key={contact.id}>
            {contact.firstName} {contact.lastName}
          </li>
        ))}
      </ul>
    </div>
  );
}
```

## API参考

### 客户端配置

```typescript
interface AtomoClientConfig {
  /** GraphQL端点URL */
  endpoint: string;
  /** 认证令牌 */
  authToken?: string;
  /** 请求头 */
  headers?: Record<string, string>;
  /** 超时时间（毫秒） */
  timeout?: number;
}
```

### 联系人操作

```typescript
// 获取单个联系人
client.getContact(id: EntityId): Promise<ContactResponse>

// 获取联系人列表
client.listContacts(options?: ContactQueryOptions): Promise<ContactListResponse>

// 创建联系人
client.createContact(input: ContactCreateInput): Promise<ContactResponse>

// 更新联系人
client.updateContact(id: EntityId, input: ContactUpdateInput): Promise<ContactResponse>

// 删除联系人
client.deleteContact(id: EntityId): Promise<boolean>
```

### 公司操作

```typescript
// 获取单个公司
client.getCompany(id: EntityId): Promise<CompanyResponse>

// 获取公司列表
client.listCompanies(options?: CompanyQueryOptions): Promise<CompanyListResponse>

// 创建公司
client.createCompany(input: CompanyCreateInput): Promise<CompanyResponse>

// 更新公司
client.updateCompany(id: EntityId, input: CompanyUpdateInput): Promise<CompanyResponse>

// 删除公司
client.deleteCompany(id: EntityId): Promise<boolean>
```

### 销售机会操作

```typescript
// 获取单个销售机会
client.getDeal(id: EntityId): Promise<DealResponse>

// 获取销售机会列表
client.listDeals(options?: DealQueryOptions): Promise<DealListResponse>

// 创建销售机会
client.createDeal(input: DealCreateInput): Promise<DealResponse>

// 更新销售机会
client.updateDeal(id: EntityId, input: DealUpdateInput): Promise<DealResponse>

// 删除销售机会
client.deleteDeal(id: EntityId): Promise<boolean>
```

## 类型系统

### 核心类型

```typescript
// 基础实体接口
interface BaseEntity {
  id: EntityId;
  streamId?: StreamId;
  createdAt: Timestamp;
  updatedAt: Timestamp;
  version?: Version;
}

// 业务模型
interface Contact extends BaseEntity {
  firstName: string;
  lastName: string;
  email: string;
  phone?: string;
  companyId?: string;
  notes?: Block[];
  tags?: string[];
}

interface Company extends BaseEntity {
  name: string;
  domain?: string;
  website?: string;
  industry?: string;
  size?: CompanySize;
  description?: string;
  notes?: Block[];
  tags?: string[];
}

interface Deal extends BaseEntity {
  title: string;
  description?: string;
  amount?: number;
  currency?: string;
  stage: DealStage;
  contactId?: string;
  companyId?: string;
  expectedCloseDate?: string;
  probability?: number;
  notes?: Block[];
  tags?: string[];
}
```

### 可组合内容系统

```typescript
// 内容块类型
type Block = 
  | ParagraphBlock 
  | CallLogBlock 
  | MeetingNoteBlock 
  | TaskBlock;

interface ParagraphBlock {
  type: "paragraph";
  content: string;
}

interface CallLogBlock {
  type: "call_log";
  duration: number;
  outcome: string;
  notes: string;
  recordedAt: Timestamp;
}

interface MeetingNoteBlock {
  type: "meeting_note";
  title: string;
  attendees: string[];
  agenda: string;
  notes: string;
  actionItems: string[];
  meetingDate: Timestamp;
}

interface TaskBlock {
  type: "task";
  title: string;
  description?: string;
  assignedTo?: string;
  dueDate?: Timestamp;
  completed: boolean;
}
```

### 查询和过滤

```typescript
// 通用查询选项
interface QueryOptions extends PaginationParams {
  where?: WhereCondition<T>;
  orderBy?: string;
  orderDirection?: "asc" | "desc";
}

// 条件查询
type WhereCondition<T> = {
  [K in keyof T]?: T[K] | {
    eq?: T[K];
    ne?: T[K];
    gt?: T[K];
    gte?: T[K];
    lt?: T[K];
    lte?: T[K];
    in?: T[K][];
    nin?: T[K][];
    contains?: T[K];
    startsWith?: T[K];
    endsWith?: T[K];
  };
};
```

## React Hooks

### 数据获取

```typescript
// 获取单个实体
const { data, loading, error, refetch } = useContact(id);
const { data, loading, error, refetch } = useCompany(id);
const { data, loading, error, refetch } = useDeal(id);

// 获取列表
const { 
  data, 
  loading, 
  error, 
  refetch,
  hasMore,
  total,
  page,
  limit 
} = useContacts(options);
```

### 数据变更

```typescript
// 创建
const { execute, loading, error, reset } = useCreateContact();
const result = await execute(input);

// 更新
const { execute, loading, error, reset } = useUpdateContact();
const result = await execute({ id, input });
```

### 工具hooks

```typescript
// 防抖
const debouncedValue = useDebounce(value, 300);

// 分页
const { 
  page, 
  limit, 
  nextPage, 
  prevPage, 
  goToPage, 
  reset 
} = usePagination();
```

## 错误处理

```typescript
try {
  const contact = await client.getContact(id);
  console.log(contact.data);
} catch (error) {
  if (error instanceof Error) {
    console.error('请求失败:', error.message);
  }
}

// 在React hooks中
const { data, error } = useContact(id);
if (error) {
  console.error('获取联系人失败:', error.message);
}
```

## 开发指南

### 本地开发

```bash
# 克隆项目
git clone https://github.com/your-org/atomo.git
cd atomo/packages/atomo-client-sdk

# 安装依赖
pnpm install

# 开发模式
pnpm dev

# 构建
pnpm build

# 类型检查
pnpm type-check
```

### 代码生成

SDK类型会根据`schema.ts`文件自动生成：

```bash
# 在项目根目录运行
atomo generate

# 这会更新 packages/atomo-client-sdk/types.ts 文件
```

## 贡献指南

1. Fork项目
2. 创建特性分支 (`git checkout -b feature/AmazingFeature`)
3. 提交更改 (`git commit -m 'Add some AmazingFeature'`)
4. 推送到分支 (`git push origin feature/AmazingFeature`)
5. 提交Pull Request

## 许可证

MIT License - 详见 [LICENSE](LICENSE) 文件

## 支持

- 📖 [文档](https://docs.atomo.dev)
- 💬 [讨论区](https://github.com/your-org/atomo/discussions)
- 🐛 [问题反馈](https://github.com/your-org/atomo/issues)
- 📧 [邮件支持](mailto:support@atomo.dev)

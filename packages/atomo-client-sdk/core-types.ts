/**
 * Atomo Core Schema Types
 * 
 * 这个文件定义了Atomo平台的核心类型接口，提供：
 * 1. 端到端类型安全
 * 2. 统一的类型定义（前端、后端、CLI共享）
 * 3. 完整的TypeScript智能提示
 * 
 * 这些类型被以下组件使用：
 * - schema.ts 文件（用户定义业务模型）
 * - atomo CLI（代码生成）
 * - 前端SDK（类型安全的API调用）
 * - 后端Rust代码（通过生成器）
 */

// ================================
// 核心平台类型
// ================================

/** 实体ID类型 - 使用ULID确保时序性 */
export type EntityId = string;

/** 流ID类型 - 事件溯源中的流标识符 */
export type StreamId = string;

/** 时间戳类型 - 统一使用ISO 8601字符串 */
export type Timestamp = string;

/** 版本号类型 - 用于乐观并发控制 */
export type Version = number;

// ================================
// 可组合内容系统（"流动的画布"）
// ================================

/** 内容块基础接口 */
export interface BaseBlock {
  /** 块类型标识符 */
  type: string;
  /** 块唯一ID */
  id?: string;
  /** 创建时间 */
  createdAt?: Timestamp;
  /** 更新时间 */
  updatedAt?: Timestamp;
}

/** 段落块 - 基础文本内容 */
export interface ParagraphBlock extends BaseBlock {
  type: "paragraph";
  content: string;
}

/** 任务块 - 通用任务管理 */
export interface TaskBlock extends BaseBlock {
  type: "task";
  title: string; // 任务标题
  description?: string; // 任务描述
  assignedTo?: string; // 负责人
  dueDate?: Timestamp; // 截止时间
  completed: boolean; // 完成状态
}

/** 核心块类型联合（应用可以扩展这些） */
export type CoreBlock = ParagraphBlock | TaskBlock;

/** 可扩展的块类型 - 应用可以定义自己的块类型 */
export type Block = CoreBlock;

// ================================
// 实体基础接口
// ================================

/** 
 * 所有Atomo实体的基础接口
 * 提供统一的字段和行为
 */
export interface BaseEntity {
  /** 实体唯一标识符 */
  id: EntityId;
  /** 事件流标识符 */
  streamId?: StreamId;
  /** 创建时间 */
  createdAt: Timestamp;
  /** 更新时间 */
  updatedAt: Timestamp;
  /** 版本号（乐观锁） */
  version?: Version;
}

// ================================
// 事件系统类型
// ================================

/** 事件元数据 */
export interface EventMetadata {
  /** 触发用户ID */
  userId?: string;
  /** 请求IP地址 */
  ipAddress?: string;
  /** 用户代理字符串 */
  userAgent?: string;
  /** 请求关联ID */
  correlationId?: string;
  /** 额外上下文 */
  context?: Record<string, any>;
}

/** 领域事件基础接口 */
export interface BaseDomainEvent<T = any> {
  /** 事件唯一标识符 */
  eventId: string;
  /** 事件流标识符 */
  streamId: StreamId;
  /** 事件类型 */
  eventType: string;
  /** 事件载荷 */
  payload: T;
  /** 事件元数据 */
  metadata: EventMetadata;
  /** 事件时间戳 */
  timestamp: Timestamp;
  /** 流中的版本号 */
  version: Version;
}

// ================================
// API响应类型
// ================================

/** 标准API响应包装器 */
export interface ApiResponse<T> {
  /** 响应数据 */
  data?: T;
  /** 错误信息 */
  error?: {
    code: string;
    message: string;
    details?: any;
  };
  /** 元数据 */
  meta?: {
    total?: number;
    page?: number;
    limit?: number;
    hasMore?: boolean;
  };
}

/** 分页查询参数 */
export interface PaginationParams {
  /** 页码（从1开始） */
  page?: number;
  /** 每页条数 */
  limit?: number;
  /** 排序字段 */
  orderBy?: string;
  /** 排序方向 */
  orderDirection?: "asc" | "desc";
}

/** 查询过滤器 */
export interface QueryFilter {
  /** 字段名 */
  field: string;
  /** 操作符 */
  operator: "eq" | "ne" | "gt" | "gte" | "lt" | "lte" | "in" | "nin" | "contains" | "startsWith" | "endsWith";
  /** 值 */
  value: any;
}

// ================================
// GraphQL相关类型
// ================================

/** GraphQL查询变量 */
export type GraphQLVariables = Record<string, any>;

/** GraphQL查询选项 */
export interface QueryOptions {
  /** 变量 */
  variables?: GraphQLVariables;
  /** 查询上下文 */
  context?: Record<string, any>;
  /** 缓存策略 */
  fetchPolicy?: "cache-first" | "cache-and-network" | "network-only" | "cache-only" | "standby";
}

// ================================
// 工具类型
// ================================

/** 深度可选类型 */
export type DeepPartial<T> = {
  [P in keyof T]?: T[P] extends object ? DeepPartial<T[P]> : T[P];
};

/** 创建输入类型（去除自动生成的字段） */
export type CreateInput<T extends BaseEntity> = Omit<T, 'id' | 'streamId' | 'createdAt' | 'updatedAt' | 'version'>;

/** 更新输入类型（所有字段可选，去除只读字段） */
export type UpdateInput<T extends BaseEntity> = DeepPartial<Omit<T, 'id' | 'streamId' | 'createdAt' | 'updatedAt' | 'version'>>;

/** 查询条件类型 */
export type WhereCondition<T> = {
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

// ================================
// 配置类型
// ================================

/** Atomo配置接口 */
export interface AtomoConfig {
  /** 数据库连接URL */
  databaseUrl?: string;
  /** 服务器端口 */
  port?: number;
  /** 环境 */
  environment?: "development" | "staging" | "production";
  /** GraphQL配置 */
  graphql?: {
    playground?: boolean;
    introspection?: boolean;
    path?: string;
  };
  /** 认证配置 */
  auth?: {
    jwtSecret?: string;
    sessionTimeout?: number;
  };
  /** 日志配置 */
  logging?: {
    level?: "debug" | "info" | "warn" | "error";
    format?: "json" | "text";
  };
}

// ================================
// 所有类型已在上方直接导出
// ================================

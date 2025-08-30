/**
 * Atomo React Hooks Types
 * 
 * 为React应用提供类型安全的hooks接口定义
 * 注意：这个文件定义了通用的hook接口，具体的CRM实现应该在应用层
 */

import { AtomoClient } from './index';
import { EntityId } from './core-types';

// ================================
// 通用数据状态接口
// ================================

export interface DataState<T> {
  data: T | null;
  loading: boolean;
  error: Error | null;
  refetch: () => Promise<void>;
}

export interface ListDataState<T> {
  data: T[];
  loading: boolean;
  error: Error | null;
  refetch: () => Promise<void>;
  hasMore: boolean;
  total: number;
  page: number;
  limit: number;
}

export interface MutationState<T> {
  data: T | null;
  loading: boolean;
  error: Error | null;
  execute: (input: any) => Promise<T | null>;
  reset: () => void;
}

// ================================
// 通用Hook接口定义
// ================================

export interface EntityHooks<T, TQueryOptions = any> {
  useEntity: (id: EntityId) => DataState<T>;
  useEntities: (options?: TQueryOptions) => ListDataState<T>;
  useCreateEntity: () => MutationState<T>;
  useUpdateEntity: () => MutationState<T>;
}

// ================================
// Provider接口定义
// ================================

export interface AtomoProviderProps {
  client: AtomoClient;
  children: any; // React.ReactNode
}

export interface AtomoClientHook {
  (): AtomoClient;
}

// ================================
// 工具hooks接口
// ================================

export interface DebounceHook {
  <T>(value: T, delay: number): T;
}

export interface PaginationHook {
  (initialPage?: number, initialLimit?: number): {
    page: number;
    limit: number;
    setLimit: (limit: number) => void;
    nextPage: () => void;
    prevPage: () => void;
    goToPage: (page: number) => void;
    reset: () => void;
  };
}

// ================================
// 使用示例常量
// ================================

export const REACT_HOOKS_USAGE_EXAMPLE = `
// 这是一个通用的SDK，具体业务hooks应该在应用层实现
// 例如：在 packages/atomo-crm-app 中创建 CRM 特定的hooks

// 1. 安装核心SDK
npm install @atomo/client-sdk

// 2. 在CRM应用中创建业务特定的hooks
// packages/atomo-crm-app/src/hooks/useContacts.ts
import { useEntity, AtomoClient } from '@atomo/client-sdk';
import { Contact, ContactQueryOptions } from '@atomo/client-sdk/types';

export function useContacts(options?: ContactQueryOptions) {
  const client = useAtomoClient();
  // CRM特定的逻辑实现
}

// 3. Provider设置 (App.tsx)
import { AtomoClient, AtomoProvider } from '@atomo/client-sdk';

const client = new AtomoClient({
  endpoint: 'http://localhost:3000/graphql',
  authToken: 'your-auth-token'
});

function App() {
  return (
    <AtomoProvider client={client}>
      <ContactsList />
    </AtomoProvider>
  );
}
`;

// ================================
// 所有类型已在上方定义并导出
// ================================
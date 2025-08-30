/**
 * 自动生成的业务模型类型
 * 
 * 这个文件由 atomo CLI 从 schema.ts 自动生成
 * 请勿手动编辑 - 所有更改将被覆盖
 * 
 * 生成时间: 2025-08-30T03:45:47.846525600+00:00
 * 源文件: packages/atomo-crm-app/atomo/schema.ts
 */

import { 
  BaseEntity, 
  Block, 
  CreateInput, 
  UpdateInput,
  WhereCondition,
  ApiResponse,
  PaginationParams
} from './core-types';

// ================================
// 业务模型接口 (从schema.ts生成)
// ================================

/** 销售机会实体 */
export interface Deal extends BaseEntity {
  stage: DealStage;
  description: Block[];
  title: string;
  value: number;
  actualCloseDate?: string;
  companyId?: string;
  contactId: string;
  expectedCloseDate?: string;
}

/** 联系人实体 */
export interface Contact extends BaseEntity {
  notes: Block[];
  lastName: string;
  tags: string[];
  firstName: string;
  companyId?: string;
  email: string;
  phone?: string;
}

/** 公司实体 */
export interface Company extends BaseEntity {
  size?: CompanySize;
  notes: Block[];
  name: string;
  industry?: string;
  address?: string;
  website?: string;
}

// ================================
// 输入类型（用于创建和更新）
// ================================

export type DealCreateInput = CreateInput<Deal>;
export type DealUpdateInput = UpdateInput<Deal>;
export type DealWhereInput = WhereCondition<Deal>;

export type ContactCreateInput = CreateInput<Contact>;
export type ContactUpdateInput = UpdateInput<Contact>;
export type ContactWhereInput = WhereCondition<Contact>;

export type CompanyCreateInput = CreateInput<Company>;
export type CompanyUpdateInput = UpdateInput<Company>;
export type CompanyWhereInput = WhereCondition<Company>;

// ================================
// API响应类型
// ================================

export type DealResponse = ApiResponse<Deal>;
export type DealListResponse = ApiResponse<Deal[]>;

export type ContactResponse = ApiResponse<Contact>;
export type ContactListResponse = ApiResponse<Contact[]>;

export type CompanyResponse = ApiResponse<Company>;
export type CompanyListResponse = ApiResponse<Company[]>;

// ================================
// 查询选项类型
// ================================

export interface DealQueryOptions extends PaginationParams {
  where?: DealWhereInput;
  include?: {
  };
}

export interface ContactQueryOptions extends PaginationParams {
  where?: ContactWhereInput;
  include?: {
  };
}

export interface CompanyQueryOptions extends PaginationParams {
  where?: CompanyWhereInput;
  include?: {
  };
}

// ================================
// 统计和聚合类型
// ================================

export interface DealStats {
  totalDeals: number;
  totalValue: number;
  averageValue: number;
  count: number;
  winRate: number;
  stageDistribution: Record<DealStage, number>;
}

export interface ContactStats {
  totalContacts: number;
  contactsWithoutCompany: number;
  contactsWithDeals: number;
}

export interface CompanyStats {
  totalCompanys: number;
  sizeDistribution: Record<CompanySize, number>;
  topIndustries: Array<{ industry: string; count: number }>;
}


/**
 * 自动生成的业务模型类型
 * 
 * 这个文件由 atomo CLI 从 schema.ts 自动生成
 * 请勿手动编辑 - 所有更改将被覆盖
 * 
 * 生成时间: 2025-08-30T00:58:24.481520200+00:00
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
// 枚举定义 (从schema.ts生成)
// ================================

/** 公司规模枚举 */
export enum CompanySize {
  STARTUP = "startup",
  SMALL = "small", 
  MEDIUM = "medium",
  LARGE = "large",
  ENTERPRISE = "enterprise"
}

/** 销售阶段枚举 */
export enum DealStage {
  LEAD = "lead",
  QUALIFIED = "qualified", 
  PROPOSAL = "proposal",
  NEGOTIATION = "negotiation",
  WON = "won",
  LOST = "lost"
}

// ================================
// 业务模型接口 (从schema.ts生成)
// ================================

/** 联系人实体 */
export interface Contact extends BaseEntity {
  notes: Block[];
  companyId?: string;
  email: string;
  phone?: string;
  firstName: string;
  tags: string[];
  lastName: string;
}

/** 公司实体 */
export interface Company extends BaseEntity {
  notes: Block[];
  size?: CompanySize;
  address?: string;
  industry?: string;
  website?: string;
  name: string;
}

/** 销售机会实体 */
export interface Deal extends BaseEntity {
  contactId: string;
  companyId?: string;
  description: Block[];
  title: string;
  stage: DealStage;
  expectedCloseDate?: string;
  actualCloseDate?: string;
  value: number;
}

// ================================
// 输入类型（用于创建和更新）
// ================================

export type ContactCreateInput = CreateInput<Contact>;
export type ContactUpdateInput = UpdateInput<Contact>;
export type ContactWhereInput = WhereCondition<Contact>;

export type CompanyCreateInput = CreateInput<Company>;
export type CompanyUpdateInput = UpdateInput<Company>;
export type CompanyWhereInput = WhereCondition<Company>;

export type DealCreateInput = CreateInput<Deal>;
export type DealUpdateInput = UpdateInput<Deal>;
export type DealWhereInput = WhereCondition<Deal>;

// ================================
// API响应类型
// ================================

export type ContactResponse = ApiResponse<Contact>;
export type ContactListResponse = ApiResponse<Contact[]>;

export type CompanyResponse = ApiResponse<Company>;
export type CompanyListResponse = ApiResponse<Company[]>;

export type DealResponse = ApiResponse<Deal>;
export type DealListResponse = ApiResponse<Deal[]>;

// ================================
// 查询选项类型
// ================================

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

export interface DealQueryOptions extends PaginationParams {
  where?: DealWhereInput;
  include?: {
  };
}

// ================================
// 统计和聚合类型
// ================================

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

export interface DealStats {
  totalDeals: number;
  totalValue: number;
  averageValue: number;
  count: number;
  winRate: number;
  stageDistribution: Record<DealStage, number>;
}


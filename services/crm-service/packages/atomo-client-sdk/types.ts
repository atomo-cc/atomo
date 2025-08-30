/**
 * 自动生成的业务模型类型
 * 
 * 这个文件由 atomo CLI 从 schema.ts 自动生成
 * 请勿手动编辑 - 所有更改将被覆盖
 * 
 * 生成时间: 2025-08-30T12:41:42.790891500+00:00
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

/** DealStage实体 */
export interface DealStage extends BaseEntity {
  QUALIFIED: string;
  NEGOTIATION: string;
  LOST: string;
  _enum_type: string;
  LEAD: string;
  PROPOSAL: string;
  WON: string;
}

/** 公司实体 */
export interface Company extends BaseEntity {
  name: string;
  notes: Block[];
  industry?: string;
  website?: string;
  size?: CompanySize;
  address?: string;
}

/** 销售机会实体 */
export interface Deal extends BaseEntity {
  expectedCloseDate?: string;
  value: number;
  title: string;
  actualCloseDate?: string;
  stage: DealStage;
  contactId: string;
  description: Block[];
  companyId?: string;
}

/** 联系人实体 */
export interface Contact extends BaseEntity {
  lastName: string;
  companyId?: string;
  tags: string[];
  firstName: string;
  notes: Block[];
  email: string;
  phone?: string;
}

/** CompanySize实体 */
export interface CompanySize extends BaseEntity {
  ENTERPRISE: string;
  _enum_type: string;
  STARTUP: string;
  SMALL: string;
  MEDIUM: string;
  LARGE: string;
}

// ================================
// 输入类型（用于创建和更新）
// ================================

export type DealStageCreateInput = CreateInput<DealStage>;
export type DealStageUpdateInput = UpdateInput<DealStage>;
export type DealStageWhereInput = WhereCondition<DealStage>;

export type CompanyCreateInput = CreateInput<Company>;
export type CompanyUpdateInput = UpdateInput<Company>;
export type CompanyWhereInput = WhereCondition<Company>;

export type DealCreateInput = CreateInput<Deal>;
export type DealUpdateInput = UpdateInput<Deal>;
export type DealWhereInput = WhereCondition<Deal>;

export type ContactCreateInput = CreateInput<Contact>;
export type ContactUpdateInput = UpdateInput<Contact>;
export type ContactWhereInput = WhereCondition<Contact>;

export type CompanySizeCreateInput = CreateInput<CompanySize>;
export type CompanySizeUpdateInput = UpdateInput<CompanySize>;
export type CompanySizeWhereInput = WhereCondition<CompanySize>;

// ================================
// API响应类型
// ================================

export type DealStageResponse = ApiResponse<DealStage>;
export type DealStageListResponse = ApiResponse<DealStage[]>;

export type CompanyResponse = ApiResponse<Company>;
export type CompanyListResponse = ApiResponse<Company[]>;

export type DealResponse = ApiResponse<Deal>;
export type DealListResponse = ApiResponse<Deal[]>;

export type ContactResponse = ApiResponse<Contact>;
export type ContactListResponse = ApiResponse<Contact[]>;

export type CompanySizeResponse = ApiResponse<CompanySize>;
export type CompanySizeListResponse = ApiResponse<CompanySize[]>;

// ================================
// 查询选项类型
// ================================

export interface DealStageQueryOptions extends PaginationParams {
  where?: DealStageWhereInput;
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

export interface ContactQueryOptions extends PaginationParams {
  where?: ContactWhereInput;
  include?: {
  };
}

export interface CompanySizeQueryOptions extends PaginationParams {
  where?: CompanySizeWhereInput;
  include?: {
  };
}

// ================================
// 统计和聚合类型
// ================================

export interface DealStageStats {
  totalDealStages: number;
  createdToday: number;
  createdThisWeek: number;
  createdThisMonth: number;
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

export interface ContactStats {
  totalContacts: number;
  contactsWithoutCompany: number;
  contactsWithDeals: number;
}

export interface CompanySizeStats {
  totalCompanySizes: number;
  createdToday: number;
  createdThisWeek: number;
  createdThisMonth: number;
}


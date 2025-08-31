/**
 * 自动生成的业务模型类型
 * 
 * 这个文件由 atomo CLI 从 schema.ts 自动生成
 * 请勿手动编辑 - 所有更改将被覆盖
 * 
 * 生成时间: 2025-08-30T14:18:33.231064100+00:00
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
  value: number;
  description: Block[];
  contactId: string;
  stage: DealStage;
  actualCloseDate?: string;
  companyId?: string;
  title: string;
  expectedCloseDate?: string;
}

/** CompanySize实体 */
export interface CompanySize extends BaseEntity {
  ENTERPRISE: string;
  SMALL: string;
  STARTUP: string;
  _enum_type: string;
  MEDIUM: string;
  LARGE: string;
}

/** 公司实体 */
export interface Company extends BaseEntity {
  size?: CompanySize;
  industry?: string;
  name: string;
  address?: string;
  website?: string;
  notes: Block[];
}

/** DealStage实体 */
export interface DealStage extends BaseEntity {
  _enum_type: string;
  LEAD: string;
  QUALIFIED: string;
  NEGOTIATION: string;
  PROPOSAL: string;
  WON: string;
  LOST: string;
}

/** 联系人实体 */
export interface Contact extends BaseEntity {
  lastName: string;
  firstName: string;
  tags: string[];
  companyId?: string;
  email: string;
  phone?: string;
  notes: Block[];
}

// ================================
// 输入类型（用于创建和更新）
// ================================

export type DealCreateInput = CreateInput<Deal>;
export type DealUpdateInput = UpdateInput<Deal>;
export type DealWhereInput = WhereCondition<Deal>;

export type CompanySizeCreateInput = CreateInput<CompanySize>;
export type CompanySizeUpdateInput = UpdateInput<CompanySize>;
export type CompanySizeWhereInput = WhereCondition<CompanySize>;

export type CompanyCreateInput = CreateInput<Company>;
export type CompanyUpdateInput = UpdateInput<Company>;
export type CompanyWhereInput = WhereCondition<Company>;

export type DealStageCreateInput = CreateInput<DealStage>;
export type DealStageUpdateInput = UpdateInput<DealStage>;
export type DealStageWhereInput = WhereCondition<DealStage>;

export type ContactCreateInput = CreateInput<Contact>;
export type ContactUpdateInput = UpdateInput<Contact>;
export type ContactWhereInput = WhereCondition<Contact>;

// ================================
// API响应类型
// ================================

export type DealResponse = ApiResponse<Deal>;
export type DealListResponse = ApiResponse<Deal[]>;

export type CompanySizeResponse = ApiResponse<CompanySize>;
export type CompanySizeListResponse = ApiResponse<CompanySize[]>;

export type CompanyResponse = ApiResponse<Company>;
export type CompanyListResponse = ApiResponse<Company[]>;

export type DealStageResponse = ApiResponse<DealStage>;
export type DealStageListResponse = ApiResponse<DealStage[]>;

export type ContactResponse = ApiResponse<Contact>;
export type ContactListResponse = ApiResponse<Contact[]>;

// ================================
// 查询选项类型
// ================================

export interface DealQueryOptions extends PaginationParams {
  where?: DealWhereInput;
  include?: {
  };
}

export interface CompanySizeQueryOptions extends PaginationParams {
  where?: CompanySizeWhereInput;
  include?: {
  };
}

export interface CompanyQueryOptions extends PaginationParams {
  where?: CompanyWhereInput;
  include?: {
  };
}

export interface DealStageQueryOptions extends PaginationParams {
  where?: DealStageWhereInput;
  include?: {
  };
}

export interface ContactQueryOptions extends PaginationParams {
  where?: ContactWhereInput;
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

export interface CompanySizeStats {
  totalCompanySizes: number;
  createdToday: number;
  createdThisWeek: number;
  createdThisMonth: number;
}

export interface CompanyStats {
  totalCompanys: number;
  sizeDistribution: Record<CompanySize, number>;
  topIndustries: Array<{ industry: string; count: number }>;
}

export interface DealStageStats {
  totalDealStages: number;
  createdToday: number;
  createdThisWeek: number;
  createdThisMonth: number;
}

export interface ContactStats {
  totalContacts: number;
  contactsWithoutCompany: number;
  contactsWithDeals: number;
}


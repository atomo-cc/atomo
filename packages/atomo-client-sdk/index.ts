/**
 * Atomo Client SDK
 * 
 * 为Atomo平台提供类型安全的前端SDK
 * 支持GraphQL查询、变更操作和实时订阅
 */

// 暂时移除graphql-request依赖，使用fetch API
// import { GraphQLClient } from 'graphql-request';
import {
  ContactCreateInput, ContactUpdateInput, ContactQueryOptions,
  CompanyCreateInput, CompanyUpdateInput, CompanyQueryOptions,
  DealCreateInput, DealUpdateInput, DealQueryOptions,
  ContactResponse, ContactListResponse,
  CompanyResponse, CompanyListResponse, 
  DealResponse, DealListResponse,
  ContactStats, CompanyStats, DealStats
} from './types';
import { EntityId, ApiResponse } from './core-types';

// ================================
// SDK配置
// ================================

export interface AtomoClientConfig {
  /** GraphQL端点URL */
  endpoint: string;
  /** 认证令牌 */
  authToken?: string;
  /** 请求头 */
  headers?: Record<string, string>;
  /** 超时时间（毫秒） */
  timeout?: number;
}

// ================================
// GraphQL查询字符串
// ================================

const CONTACT_FIELDS = `
  id
  streamId
  firstName
  lastName
  email
  phone
  companyId
  notes {
    type
    id
    createdAt
    updatedAt
    ... on ParagraphBlock {
      content
    }
    ... on CallLogBlock {
      duration
      outcome
      notes
      recordedAt
    }
    ... on MeetingNoteBlock {
      title
      attendees
      agenda
      notes
      actionItems
      meetingDate
    }
    ... on TaskBlock {
      title
      description
      assignedTo
      dueDate
      completed
    }
  }
  tags
  createdAt
  updatedAt
  version
`;

const COMPANY_FIELDS = `
  id
  streamId
  name
  domain
  website
  industry
  size
  description
  notes {
    type
    id
    createdAt
    updatedAt
    ... on ParagraphBlock {
      content
    }
    ... on CallLogBlock {
      duration
      outcome
      notes
      recordedAt
    }
    ... on MeetingNoteBlock {
      title
      attendees
      agenda
      notes
      actionItems
      meetingDate
    }
    ... on TaskBlock {
      title
      description
      assignedTo
      dueDate
      completed
    }
  }
  tags
  createdAt
  updatedAt
  version
`;

const DEAL_FIELDS = `
  id
  streamId
  title
  description
  amount
  currency
  stage
  contactId
  companyId
  expectedCloseDate
  probability
  notes {
    type
    id
    createdAt
    updatedAt
    ... on ParagraphBlock {
      content
    }
    ... on CallLogBlock {
      duration
      outcome
      notes
      recordedAt
    }
    ... on MeetingNoteBlock {
      title
      attendees
      agenda
      notes
      actionItems
      meetingDate
    }
    ... on TaskBlock {
      title
      description
      assignedTo
      dueDate
      completed
    }
  }
  tags
  createdAt
  updatedAt
  version
`;

// ================================
// 简单GraphQL客户端实现
// ================================

interface GraphQLRequest {
  query: string;
  variables?: Record<string, any>;
}

interface GraphQLResponse<T = any> {
  data?: T;
  errors?: Array<{ message: string; path?: string[] }>;
}

class SimpleGraphQLClient {
  constructor(
    private endpoint: string,
    private headers: Record<string, string> = {},
    private timeout: number = 30000
  ) {}

  async request<T = any>(query: string, variables?: Record<string, any>): Promise<T> {
    const requestBody: GraphQLRequest = { query, variables };
    
    const controller = new AbortController();
    const timeoutId = setTimeout(() => controller.abort(), this.timeout);

    try {
      const response = await fetch(this.endpoint, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          ...this.headers
        },
        body: JSON.stringify(requestBody),
        signal: controller.signal
      });

      clearTimeout(timeoutId);

      if (!response.ok) {
        throw new Error(`HTTP ${response.status}: ${response.statusText}`);
      }

      const result: GraphQLResponse<T> = await response.json();

      if (result.errors && result.errors.length > 0) {
        throw new Error(`GraphQL Error: ${result.errors[0].message}`);
      }

      return result.data as T;
    } catch (error) {
      clearTimeout(timeoutId);
      if (error instanceof Error && error.name === 'AbortError') {
        throw new Error(`Request timeout after ${this.timeout}ms`);
      }
      throw error;
    }
  }
}

// ================================
// Atomo客户端主类  
// ================================

export class AtomoClient {
  private graphqlClient: SimpleGraphQLClient;

  constructor(config: AtomoClientConfig) {
    const headers: Record<string, string> = {
      'Content-Type': 'application/json',
      ...config.headers
    };

    if (config.authToken) {
      headers['Authorization'] = `Bearer ${config.authToken}`;
    }

    this.graphqlClient = new SimpleGraphQLClient(
      config.endpoint,
      headers,
      config.timeout || 30000
    );
  }

  // ================================
  // 联系人操作
  // ================================

  async getContact(id: EntityId): Promise<ContactResponse> {
    const query = `
      query GetContact($id: ID!) {
        contact(id: $id) {
          ${CONTACT_FIELDS}
        }
      }
    `;

    const data = await this.graphqlClient.request(query, { id });
    return { data: data.contact };
  }

  async listContacts(options: ContactQueryOptions = {}): Promise<ContactListResponse> {
    const query = `
      query ListContacts($where: ContactWhereInput, $orderBy: String, $orderDirection: String, $page: Int, $limit: Int) {
        contacts(where: $where, orderBy: $orderBy, orderDirection: $orderDirection, page: $page, limit: $limit) {
          data {
            ${CONTACT_FIELDS}
          }
          meta {
            total
            page
            limit
            hasMore
          }
        }
      }
    `;

    const result = await this.graphqlClient.request(query, options);
    return {
      data: result.contacts.data,
      meta: result.contacts.meta
    };
  }

  async createContact(input: ContactCreateInput): Promise<ContactResponse> {
    const mutation = `
      mutation CreateContact($input: ContactCreateInput!) {
        createContact(input: $input) {
          ${CONTACT_FIELDS}
        }
      }
    `;

    const data = await this.graphqlClient.request(mutation, { input });
    return { data: data.createContact };
  }

  async updateContact(id: EntityId, input: ContactUpdateInput): Promise<ContactResponse> {
    const mutation = `
      mutation UpdateContact($id: ID!, $input: ContactUpdateInput!) {
        updateContact(id: $id, input: $input) {
          ${CONTACT_FIELDS}
        }
      }
    `;

    const data = await this.graphqlClient.request(mutation, { id, input });
    return { data: data.updateContact };
  }

  async deleteContact(id: EntityId): Promise<boolean> {
    const mutation = `
      mutation DeleteContact($id: ID!) {
        deleteContact(id: $id)
      }
    `;

    const data = await this.graphqlClient.request(mutation, { id });
    return data.deleteContact;
  }

  // ================================
  // 公司操作
  // ================================

  async getCompany(id: EntityId): Promise<CompanyResponse> {
    const query = `
      query GetCompany($id: ID!) {
        company(id: $id) {
          ${COMPANY_FIELDS}
        }
      }
    `;

    const data = await this.graphqlClient.request(query, { id });
    return { data: data.company };
  }

  async listCompanies(options: CompanyQueryOptions = {}): Promise<CompanyListResponse> {
    const query = `
      query ListCompanies($where: CompanyWhereInput, $orderBy: String, $orderDirection: String, $page: Int, $limit: Int) {
        companies(where: $where, orderBy: $orderBy, orderDirection: $orderDirection, page: $page, limit: $limit) {
          data {
            ${COMPANY_FIELDS}
          }
          meta {
            total
            page
            limit
            hasMore
          }
        }
      }
    `;

    const result = await this.graphqlClient.request(query, options);
    return {
      data: result.companies.data,
      meta: result.companies.meta
    };
  }

  async createCompany(input: CompanyCreateInput): Promise<CompanyResponse> {
    const mutation = `
      mutation CreateCompany($input: CompanyCreateInput!) {
        createCompany(input: $input) {
          ${COMPANY_FIELDS}
        }
      }
    `;

    const data = await this.graphqlClient.request(mutation, { input });
    return { data: data.createCompany };
  }

  async updateCompany(id: EntityId, input: CompanyUpdateInput): Promise<CompanyResponse> {
    const mutation = `
      mutation UpdateCompany($id: ID!, $input: CompanyUpdateInput!) {
        updateCompany(id: $id, input: $input) {
          ${COMPANY_FIELDS}
        }
      }
    `;

    const data = await this.graphqlClient.request(mutation, { id, input });
    return { data: data.updateCompany };
  }

  async deleteCompany(id: EntityId): Promise<boolean> {
    const mutation = `
      mutation DeleteCompany($id: ID!) {
        deleteCompany(id: $id)
      }
    `;

    const data = await this.graphqlClient.request(mutation, { id });
    return data.deleteCompany;
  }

  // ================================
  // 销售机会操作
  // ================================

  async getDeal(id: EntityId): Promise<DealResponse> {
    const query = `
      query GetDeal($id: ID!) {
        deal(id: $id) {
          ${DEAL_FIELDS}
        }
      }
    `;

    const data = await this.graphqlClient.request(query, { id });
    return { data: data.deal };
  }

  async listDeals(options: DealQueryOptions = {}): Promise<DealListResponse> {
    const query = `
      query ListDeals($where: DealWhereInput, $orderBy: String, $orderDirection: String, $page: Int, $limit: Int) {
        deals(where: $where, orderBy: $orderBy, orderDirection: $orderDirection, page: $page, limit: $limit) {
          data {
            ${DEAL_FIELDS}
          }
          meta {
            total
            page
            limit
            hasMore
          }
        }
      }
    `;

    const result = await this.graphqlClient.request(query, options);
    return {
      data: result.deals.data,
      meta: result.deals.meta
    };
  }

  async createDeal(input: DealCreateInput): Promise<DealResponse> {
    const mutation = `
      mutation CreateDeal($input: DealCreateInput!) {
        createDeal(input: $input) {
          ${DEAL_FIELDS}
        }
      }
    `;

    const data = await this.graphqlClient.request(mutation, { input });
    return { data: data.createDeal };
  }

  async updateDeal(id: EntityId, input: DealUpdateInput): Promise<DealResponse> {
    const mutation = `
      mutation UpdateDeal($id: ID!, $input: DealUpdateInput!) {
        updateDeal(id: $id, input: $input) {
          ${DEAL_FIELDS}
        }
      }
    `;

    const data = await this.graphqlClient.request(mutation, { id, input });
    return { data: data.updateDeal };
  }

  async deleteDeal(id: EntityId): Promise<boolean> {
    const mutation = `
      mutation DeleteDeal($id: ID!) {
        deleteDeal(id: $id)
      }
    `;

    const data = await this.graphqlClient.request(mutation, { id });
    return data.deleteDeal;
  }

  // ================================
  // 统计和分析
  // ================================

  async getContactStats(): Promise<ApiResponse<ContactStats>> {
    const query = `
      query GetContactStats {
        contactStats {
          totalContacts
          contactsWithoutCompany
          contactsWithDeals
        }
      }
    `;

    const data = await this.graphqlClient.request(query);
    return { data: data.contactStats };
  }

  async getCompanyStats(): Promise<ApiResponse<CompanyStats>> {
    const query = `
      query GetCompanyStats {
        companyStats {
          totalCompanies
          sizeDistribution
          topIndustries {
            industry
            count
          }
        }
      }
    `;

    const data = await this.graphqlClient.request(query);
    return { data: data.companyStats };
  }

  async getDealStats(): Promise<ApiResponse<DealStats>> {
    const query = `
      query GetDealStats {
        dealStats {
          totalValue
          averageValue
          count
          winRate
          stageDistribution
        }
      }
    `;

    const data = await this.graphqlClient.request(query);
    return { data: data.dealStats };
  }
}

// ================================
// 便利导出
// ================================

export * from './types';
export * from './core-types';

// 默认导出
export default AtomoClient;

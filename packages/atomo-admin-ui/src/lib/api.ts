/**
 * Atomo API Client
 * 
 * 统一的API客户端，用于与Atomo Core通信
 */

import axios, { AxiosInstance } from 'axios'
import { SchemaMetadata, EntityData, QueryOptions } from './types'

class AtomoApiClient {
  private client: AxiosInstance
  private baseUrl: string

  constructor(baseUrl: string = '/api') {
    this.baseUrl = baseUrl
    this.client = axios.create({
      baseURL: baseUrl,
      headers: {
        'Content-Type': 'application/json',
      },
    })

    // 请求拦截器 - 添加认证 token
    this.client.interceptors.request.use((config) => {
      const token = localStorage.getItem('atomo_auth_token')
      if (token) {
        config.headers.Authorization = `Bearer ${token}`
      }
      return config
    })

    // 响应拦截器 - 统一错误处理
    this.client.interceptors.response.use(
      (response) => response,
      (error) => {
        if (error.response?.status === 401) {
          // 认证失败，清除 token 并跳转登录
          localStorage.removeItem('atomo_auth_token')
          window.location.href = '/login'
        }
        return Promise.reject(error)
      }
    )
  }

  /**
   * 获取服务的 Schema 元数据
   * 这是动态渲染的基础 - Admin UI 根据这个元数据生成界面
   */
  async getSchemaMetadata(): Promise<SchemaMetadata> {
    const response = await this.client.get('/meta/schema')
    return response.data
  }

  /**
   * GraphQL 查询
   */
  async graphql(query: string, variables?: Record<string, any>): Promise<any> {
    const response = await this.client.post('/graphql', {
      query,
      variables,
    })
    
    if (response.data.errors) {
      throw new Error(response.data.errors[0].message)
    }
    
    return response.data.data
  }

  /**
   * 列表查询 - 支持分页、排序、筛选
   */
  async listEntities(modelName: string, options: QueryOptions = {}): Promise<{
    data: EntityData[]
    total: number
    page: number
    limit: number
  }> {
    const {
      page = 1,
      limit = 20,
      sort = 'createdAt',
      order = 'desc',
      filters = {},
      search,
    } = options

    // 构建 GraphQL 查询
    const filterConditions = Object.entries(filters)
      .filter(([_, value]) => value !== undefined && value !== '')
      .map(([key, value]) => {
        if (typeof value === 'string') {
          return `${key}: { contains: "${value}" }`
        }
        return `${key}: { equals: ${JSON.stringify(value)} }`
      })
      .join(', ')

    const searchCondition = search 
      ? `search: "${search}"` 
      : ''

    const whereClause = [searchCondition, filterConditions]
      .filter(Boolean)
      .join(', ')

    const query = `
      query List${modelName}s($offset: Int!, $limit: Int!) {
        ${modelName.toLowerCase()}s(
          offset: $offset
          limit: $limit
          orderBy: { ${sort}: ${order.toUpperCase()} }
          ${whereClause ? `where: { ${whereClause} }` : ''}
        ) {
          nodes {
            id
            createdAt
            updatedAt
            # 这里需要根据 schema 动态生成字段
            ... on ${modelName} {
              # 所有字段会在实际实现中动态插入
            }
          }
          totalCount
        }
      }
    `

    const result = await this.graphql(query, {
      offset: (page - 1) * limit,
      limit,
    })

    const nodes = result[`${modelName.toLowerCase()}s`]
    return {
      data: nodes.nodes,
      total: nodes.totalCount,
      page,
      limit,
    }
  }

  /**
   * 获取单个实体
   */
  async getEntity(modelName: string, id: string): Promise<EntityData> {
    const query = `
      query Get${modelName}($id: ID!) {
        ${modelName.toLowerCase()}(id: $id) {
          id
          createdAt
          updatedAt
          # 动态字段
        }
      }
    `

    const result = await this.graphql(query, { id })
    return result[modelName.toLowerCase()]
  }

  /**
   * 创建实体
   */
  async createEntity(modelName: string, data: Partial<EntityData>): Promise<EntityData> {
    const query = `
      mutation Create${modelName}($input: Create${modelName}Input!) {
        create${modelName}(input: $input) {
          id
          createdAt
          updatedAt
        }
      }
    `

    const result = await this.graphql(query, { input: data })
    return result[`create${modelName}`]
  }

  /**
   * 更新实体
   */
  async updateEntity(modelName: string, id: string, data: Partial<EntityData>): Promise<EntityData> {
    const query = `
      mutation Update${modelName}($id: ID!, $input: Update${modelName}Input!) {
        update${modelName}(id: $id, input: $input) {
          id
          updatedAt
        }
      }
    `

    const result = await this.graphql(query, { id, input: data })
    return result[`update${modelName}`]
  }

  /**
   * 删除实体
   */
  async deleteEntity(modelName: string, id: string): Promise<boolean> {
    const query = `
      mutation Delete${modelName}($id: ID!) {
        delete${modelName}(id: $id)
      }
    `

    const result = await this.graphql(query, { id })
    return result[`delete${modelName}`]
  }

  /**
   * 批量操作
   */
  async bulkDelete(modelName: string, ids: string[]): Promise<number> {
    const query = `
      mutation BulkDelete${modelName}($ids: [ID!]!) {
        bulkDelete${modelName}(ids: $ids)
      }
    `

    const result = await this.graphql(query, { ids })
    return result[`bulkDelete${modelName}`]
  }
}

// 导出单例实例
export const apiClient = new AtomoApiClient()
export default apiClient

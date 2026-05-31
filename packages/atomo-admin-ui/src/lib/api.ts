/**
 * Atomo API Client
 * 
 * 统一的API客户端，用于与Atomo Core通信
 */

import axios, { AxiosInstance } from 'axios'
import { SchemaMetadata, EntityData, QueryOptions } from './types'
import { loadSchemaMetadata } from './schema-parser'

class AtomoApiClient {
  private client: AxiosInstance
  private baseUrl: string

  constructor(baseUrl: string = '') {
    // Simplified URL detection - more reliable than complex logic
    this.baseUrl = baseUrl || this.getApiBaseUrl()
    this.client = axios.create({
      baseURL: this.baseUrl,
      headers: {
        'Content-Type': 'application/json',
      },
      timeout: 10000, // Add timeout for better error handling
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
   * Simplified and more reliable API base URL detection
   */
  private getApiBaseUrl(): string {
    // Check for explicit environment variable first
    const envUrl = (import.meta as any).env?.VITE_API_URL
    if (envUrl) {
      return envUrl
    }

    // In development, try common backend ports
    if ((import.meta as any).env?.DEV) {
      const currentHost = window.location.hostname
      // Try CRM service port first (most common)
      return `http://${currentHost}:3000`
    }

    // In production, use relative URLs (should be proxied)
    return '/api'
  }

  /**
   * 获取服务的 Schema 元数据
   */
  async getSchemaMetadata(): Promise<SchemaMetadata> {
    return loadSchemaMetadata()
  }

  /**
   * GraphQL 查询 with better error handling
   */
  async graphql(query: string, variables?: Record<string, any>): Promise<any> {
    try {
      const response = await this.client.post('/graphql', {
        query,
        variables,
      })

      if (response.data.errors) {
        console.error('GraphQL errors:', response.data.errors)
        throw new Error(response.data.errors[0].message)
      }

      return response.data.data
    } catch (error: any) {
      console.error('GraphQL request failed:', error)
      // If it's a network error and we're in development, provide helpful message
      if (error.code === 'ECONNREFUSED' && (import.meta as any).env?.DEV) {
        throw new Error(`Cannot connect to API server. Make sure the backend service is running on ${this.baseUrl}`)
      }
      throw error
    }
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
    const { page = 1, limit = 20, sort, order = 'desc', filters = {} } = options
    const offset = (page - 1) * limit

    // Build where filter
    const where_: Record<string, any> = {}
    for (const [key, value] of Object.entries(filters)) {
      if (value !== undefined && value !== '') {
        where_[key] = typeof value === 'string' ? { contains: value } : { equals: value }
      }
    }

    const orderBy = sort ? { [sort]: order.toUpperCase() } : undefined

    const result = await this.graphql(`
      query($model: String!, $where: JSON, $orderBy: JSON, $limit: Int, $offset: Int) {
        paginatedRecords(model: $model, where: $where, orderBy: $orderBy, limit: $limit, offset: $offset) {
          data
          pageInfo { totalCount hasNextPage hasPreviousPage }
        }
      }
    `, {
      model: modelName,
      where: Object.keys(where_).length ? where_ : undefined,
      orderBy,
      limit,
      offset,
    })

    const paginated = result.paginatedRecords
    return {
      data: paginated?.data || [],
      total: paginated?.pageInfo?.totalCount || 0,
      page,
      limit,
    }
  }

  /**
   * 获取单个实体
   */
  async getEntity(modelName: string, id: string): Promise<EntityData> {
    const result = await this.graphql(`
      query($model: String!, $id: String!) {
        record(model: $model, id: $id)
      }
    `, { model: modelName, id })
    return result.record
  }

  /**
   * 创建实体
   */
  async createEntity(modelName: string, data: Record<string, any>): Promise<EntityData> {
    const result = await this.graphql(`
      mutation($model: String!, $data: JSON!) {
        create(model: $model, data: $data)
      }
    `, { model: modelName, data })
    return result.create
  }

  /**
   * 更新实体
   */
  async updateEntity(modelName: string, id: string, data: Record<string, any>): Promise<EntityData> {
    const result = await this.graphql(`
      mutation($model: String!, $where: JSON!, $data: JSON!) {
        update(model: $model, where: $where, data: $data)
      }
    `, { model: modelName, where: { id: { equals: id } }, data })
    return result.update
  }

  /**
   * 删除实体
   */
  async deleteEntity(modelName: string, id: string): Promise<void> {
    await this.graphql(`
      mutation($model: String!, $where: JSON!) {
        delete(model: $model, where: $where)
      }
    `, { model: modelName, where: { id: { equals: id } } })
  }

  /**
   * 批量操作
   */
  async bulkDelete(modelName: string, ids: string[]): Promise<number> {
    const query = `
      mutation BulkDelete${modelName}($where: CompanyBoolExp!) {
        delete${modelName}(where: $where) {
          affectedRows
        }
      }
    `

    const result = await this.graphql(query, { where: { id: { _in: ids } } })
    return result[`affectedRows`]
  }

  /**
   * REST fallback methods for basic operations (inspired by CRM service)
   */
  async getEntityRest(entityType: string, id: string): Promise<any> {
    try {
      const response = await this.client.get(`/${entityType.toLowerCase()}s/${id}`)
      return response.data
    } catch (error) {
      console.error(`Failed to fetch ${entityType}:`, error)
      throw error
    }
  }

  async listEntitiesRest(entityType: string, options: {
    filters?: Record<string, any>
    sort?: string
    order?: 'asc' | 'desc'
    limit?: number
    offset?: number
  } = {}): Promise<{ data: any[]; total: number }> {
    try {
      const params = new URLSearchParams()
      if (options.filters) {
        params.append('filters', JSON.stringify(options.filters))
      }
      if (options.sort) params.append('sort', options.sort)
      if (options.order) params.append('order', options.order)
      if (options.limit) params.append('limit', options.limit.toString())
      if (options.offset) params.append('offset', options.offset.toString())

      const response = await this.client.get(`/${entityType.toLowerCase()}s?${params}`)
      return response.data
    } catch (error) {
      console.error(`Failed to fetch ${entityType}s:`, error)
      throw error
    }
  }

  async createEntityRest(entityType: string, data: any): Promise<any> {
    try {
      const response = await this.client.post(`/${entityType.toLowerCase()}s`, data)
      return response.data
    } catch (error) {
      console.error(`Failed to create ${entityType}:`, error)
      throw error
    }
  }

  async updateEntityRest(entityType: string, id: string, data: any): Promise<any> {
    try {
      const response = await this.client.put(`/${entityType.toLowerCase()}s/${id}`, data)
      return response.data
    } catch (error) {
      console.error(`Failed to update ${entityType}:`, error)
      throw error
    }
  }

  // Extend method to allow extending the client with custom methods
  extend<T>(methods: T): AtomoApiClient & T {
    return Object.assign(this, methods)
  }
}

// 导出单例实例
export const apiClient = new AtomoApiClient()
export default apiClient

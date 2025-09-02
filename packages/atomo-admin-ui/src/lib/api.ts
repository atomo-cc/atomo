/**
 * Atomo API Client
 * 
 * 统一的API客户端，用于与Atomo Core通信
 */

import axios, { AxiosInstance } from 'axios'
import { SchemaMetadata, EntityData, QueryOptions } from './types'
import { loadSchemaMetadata } from './schema-parser'

/**
 * 将camelCase字段名转换为snake_case（用于GraphQL输入）
 */
function camelToSnakeCase(obj: Record<string, any>): Record<string, any> {
  const result: Record<string, any> = {}
  
  for (const [key, value] of Object.entries(obj)) {
    // 跳过ID字段和时间戳字段（它们已经是正确格式）
    if (['id', 'createdAt', 'updatedAt', 'created_at', 'updated_at'].includes(key)) {
      result[key] = value
    } else {
      // 将camelCase转换为snake_case
      const snakeKey = key.replace(/([A-Z])/g, '_$1').toLowerCase()
      result[snakeKey] = value
    }
  }
  
  return result
}

class AtomoApiClient {
  private client: AxiosInstance
  private baseUrl: string

  constructor(baseUrl: string = '') {
    // 🎯 新架构：智能检测API基础URL
    // 在开发环境中自动适配到后端服务端口
    this.baseUrl = baseUrl || this.detectApiBaseUrl()
    this.client = axios.create({
      baseURL: this.baseUrl,
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
   * 智能检测API基础URL
   * 🎯 解决开发环境中Admin UI和后端服务运行在不同端口的问题
   */
  private detectApiBaseUrl(): string {
    const currentPort = window.location.port
    const currentHost = window.location.hostname
    
    // 如果当前在5173端口（Vite开发服务器），需要重定向到后端服务
    if (currentPort === '5173') {
      // 首先检查环境变量中的后端端口配置
      const envBackendPort = (window as any).__ATOMO_BACKEND_PORT__ || 
                            (import.meta as any).env?.VITE_BACKEND_PORT ||
                            (import.meta as any).env?.VITE_API_PORT
      
      if (envBackendPort) {
        return `http://${currentHost}:${envBackendPort}`
      }
      
      // 回退到默认端口（workspace dev的默认配置）
      const defaultBackendPort = '3001'
      return `http://${currentHost}:${defaultBackendPort}`
    }
    
    // 其他情况使用相对路径（通过代理访问）
    return ''
  }

  /**
   * 获取服务的 Schema 元数据
   * 🎯 符合架构原则：直接从schema.ts文件解析元数据
   * 而不是依赖后端硬编码的元数据
   */
  async getSchemaMetadata(): Promise<SchemaMetadata> {
    return loadSchemaMetadata()
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

    // 获取schema元数据来动态生成字段
    const schema = await this.getSchemaMetadata()
    const modelMetadata = schema.models[modelName]
    
    if (!modelMetadata) {
      throw new Error(`未找到模型 ${modelName} 的元数据`)
    }

    // 动态生成GraphQL字段
    const modelFields = Object.keys(modelMetadata.fields)
      .filter(field => field !== 'id' && field !== 'createdAt' && field !== 'updatedAt') // 这些字段已经在外层包含了
      .map(field => {
        const fieldMeta = modelMetadata.fields[field]
        // 根据字段类型决定是否需要子字段选择
        if (fieldMeta.type === 'reference') {
          // 引用类型需要子字段（但只有在确实是外键引用时才这样做）
          return `          ${field} { id name title }`
        }
        // 所有其他类型（string, number, boolean, json, datetime等）都是标量类型
        // 直接返回字段名，不需要子字段选择
        return `          ${field}`
      })
      .join('\n')

    // 构建筛选条件
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

    // 构建查询参数
    const queryParams = [
      'offset: $offset',
      'limit: $limit',
      `orderBy: { ${sort}: ${order.toUpperCase()} }`
    ]
    
    if (whereClause) {
      queryParams.push(`where: { ${whereClause} }`)
    }
    
    const paramString = queryParams.join('\n          ')
    const aggregateParams = whereClause ? `where: { ${whereClause} }` : ''

    const fieldsSection = modelFields ? `\n${modelFields}` : ''
    
    const query = `
      query List${modelName}s($offset: Int!, $limit: Int!) {
        ${modelName.toLowerCase()}s(
          ${paramString}
        ) {
          id${fieldsSection}
          createdAt
          updatedAt
        }
        ${modelName.toLowerCase()}sAggregate${aggregateParams ? `(${aggregateParams})` : ''} {
          aggregate {
            count
          }
        }
      }
    `

    const result = await this.graphql(query, {
      offset: (page - 1) * limit,
      limit,
    })

    const entities = result[`${modelName.toLowerCase()}s`] || []
    const aggregate = result[`${modelName.toLowerCase()}sAggregate`]
    const total = aggregate?.aggregate?.count || 0

    return {
      data: entities,
      total,
      page,
      limit,
    }
  }

  /**
   * 获取单个实体
   */
  async getEntity(modelName: string, id: string): Promise<EntityData> {
    // 获取schema元数据来动态生成字段
    const schema = await this.getSchemaMetadata()
    const modelMetadata = schema.models[modelName]
    
    if (!modelMetadata) {
      throw new Error(`未找到模型 ${modelName} 的元数据`)
    }

    // 动态生成GraphQL字段
    const modelFields = Object.keys(modelMetadata.fields)
      .filter(field => field !== 'id' && field !== 'createdAt' && field !== 'updatedAt')
      .map(field => {
        const fieldMeta = modelMetadata.fields[field]
        if (fieldMeta.type === 'reference') {
          return `          ${field} { id name title }`
        }
        // 所有其他类型都是标量类型
        return `          ${field}`
      })
      .join('\n')

    const fieldsSection = modelFields ? `\n${modelFields}` : ''
    
    const query = `
      query Get${modelName}($id: ID!) {
        ${modelName.toLowerCase()}ByPk(id: $id) {
          id${fieldsSection}
          createdAt
          updatedAt
        }
      }
    `

    const result = await this.graphql(query, { id })
    return result[`${modelName.toLowerCase()}ByPk`]
  }

  /**
   * 创建实体
   */
  async createEntity(modelName: string, data: Partial<EntityData>): Promise<EntityData> {
    const mutationName = `insert${modelName}One`
    
    // 转换字段名为snake_case
    const snakeCaseData = camelToSnakeCase(data)
    
    const query = `
      mutation Create${modelName}($object: ${modelName}InsertInput!) {
        ${mutationName}(object: $object) {
          id
          createdAt
          updatedAt
        }
      }
    `

    const result = await this.graphql(query, { object: snakeCaseData })
    return result[mutationName]
  }

  /**
   * 更新实体
   */
  async updateEntity(modelName: string, id: string, data: Partial<EntityData>): Promise<EntityData> {
    const mutationName = `update${modelName}ByPk`
    
    // 转换字段名为snake_case
    const snakeCaseData = camelToSnakeCase(data)
    
    const query = `
      mutation Update${modelName}($pkColumns: ${modelName}PkColumnsInput!, $set: ${modelName}SetInput!) {
        ${mutationName}(pkColumns: $pkColumns, set: $set) {
          id
          updatedAt
        }
      }
    `

    const result = await this.graphql(query, { 
      pkColumns: { id },
      set: snakeCaseData 
    })
    return result[mutationName]
  }

  /**
   * 删除实体
   */
  async deleteEntity(modelName: string, id: string): Promise<boolean> {
    const mutationName = `delete${modelName}ByPk`
    
    const query = `
      mutation Delete${modelName}($id: ID!) {
        ${mutationName}(id: $id) {
          id
        }
      }
    `

    const result = await this.graphql(query, { id })
    return !!result[mutationName]
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
}

// 导出单例实例
export const apiClient = new AtomoApiClient()
export default apiClient

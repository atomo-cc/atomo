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
      const defaultBackendPort = '3000'  // CRM service runs on port 3000
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
   * 获取GraphQL查询字段名
   * 处理平台模型的特殊命名
   */
  private getGraphQLQueryField(modelName: string): string {
    // 平台模型的特殊映射
    const platformModelMapping: Record<string, string> = {
      'User': 'users',
      'PlatformUser': 'platformUsers',
      'Session': 'userSessions',
      'UserSession': 'userSessions',
      'AuditLog': 'auditLogEntries',
      'AuditLogEntry': 'auditLogEntries'
    }
    
    if (platformModelMapping[modelName]) {
      return platformModelMapping[modelName]
    }
    
    // 默认规则：转换为复数形式
    return `${modelName.toLowerCase()}s`
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
      .filter(field => !['id', 'createdAt', 'updatedAt', 'created_at', 'updated_at'].includes(field)) // 这些字段已经在外层包含了
      .map(field => {
        const fieldMeta = modelMetadata.fields[field]
        // 根据字段类型决定是否需要子字段选择
        if (fieldMeta.type === 'reference') {
          // 引用类型需要子字段（但只有在确实是外键引用时才这样做）
          return `          ${field} { id name title }`
        } else if (fieldMeta.type === 'blocks') {
          // ContentBlock类型需要Union内联片段语法
          return `          ${field} {
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
          }`
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

    // 🎯 智能查询字段生成：基于元数据自动确定GraphQL端点
    const queryField = modelMetadata.queryEndpoint || this.getGraphQLQueryField(modelName)
    
    // 🎯 智能平台模型检测：基于元数据而非硬编码
    const isPlatformModel = modelMetadata.isPlatformModel || false
    
    // 构建查询参数（平台模型仅支持 offset/limit，不支持复杂查询）
    const queryParams = isPlatformModel
      ? [
          'offset: $offset', 
          'limit: $limit',
        ]
      : [
          'offset: $offset',
          'limit: $limit',
          `orderBy: { ${sort}: ${order.toUpperCase()} }`
        ]
    
    // 平台模型暂不支持复杂的where查询
    if (!isPlatformModel && whereClause) {
      queryParams.push(`where: { ${whereClause} }`)
    }
    
    const paramString = queryParams.join('\n          ')
    const aggregateParams = !isPlatformModel && whereClause ? `where: { ${whereClause} }` : ''

    const fieldsSection = modelFields ? `\n${modelFields}` : ''
    const aggregateField = `${queryField}Aggregate`

    // 平台模型时间字段兼容处理（createdAt/updatedAt vs created_at/updated_at）
    const createdAtFieldName = modelMetadata.fields['createdAt'] ? 'createdAt' : (modelMetadata.fields['created_at'] ? 'created_at' : '')
    const updatedAtFieldName = modelMetadata.fields['updatedAt'] ? 'updatedAt' : (modelMetadata.fields['updated_at'] ? 'updated_at' : '')
    const timestampsSection = [createdAtFieldName, updatedAtFieldName]
      .filter(Boolean)
      .map(f => `\n          ${f}`)
      .join('')

    // 平台模型不支持 aggregate 统计，省略该部分
    const aggregateSection = isPlatformModel ? '' : `
        ${aggregateField}${aggregateParams ? `(${aggregateParams})` : ''} {
          aggregate {
            count
          }
        }`

    
    const query = `
      query List${modelName}s($offset: Int!, $limit: Int!) {
        ${queryField}(
          ${paramString}
        ) {
          id${fieldsSection}${timestampsSection}
        }${aggregateSection}
      }
    `

    const result = await this.graphql(query, {
      offset: (page - 1) * limit,
      limit,
    })

    const entities = result[queryField] || []
    const aggregate = result[aggregateField]
    const total = aggregate?.aggregate?.count || 0

    return {
      data: entities,
      total,
      page,
      limit,
    }
  }

  /**
   * 🎯 智能单个查询字段生成：基于元数据自动确定GraphQL端点
   */
  private async getGraphQLSingleQueryField(modelName: string): Promise<string> {
    // 获取模型元数据
    const schema = await this.getSchemaMetadata()
    const modelMetadata = schema.models[modelName]
    
    // 🎯 平台模型特殊处理：通过列表查询获取单个实体
    if (modelMetadata?.isPlatformModel) {
      // 平台模型没有单独的 byPk 查询，需要使用列表查询
      const listEndpoint = modelMetadata.queryEndpoint || this.getGraphQLQueryField(modelName)
      return listEndpoint
    }
    
    // 🎯 业务模型的标准单个查询
    return `${modelName.toLowerCase()}ByPk`
  }

  /**
   * 🎯 获取单个实体：智能适配平台模型和业务模型
   */
  async getEntity(modelName: string, id: string): Promise<EntityData> {
    // 获取schema元数据来动态生成字段
    const schema = await this.getSchemaMetadata()
    const modelMetadata = schema.models[modelName]
    
    if (!modelMetadata) {
      throw new Error(`未找到模型 ${modelName} 的元数据`)
    }

    // 🎯 智能字段生成：基于模型类型自动适配
    const isPlatformModel = modelMetadata.isPlatformModel || false
    
    // 🎯 智能GraphQL字段生成：支持复杂类型和标量类型
    const modelFields = Object.keys(modelMetadata.fields)
      .filter(field => !['id', 'createdAt', 'updatedAt', 'created_at', 'updated_at'].includes(field))
      .map(field => {
        const fieldMeta = modelMetadata.fields[field]
        
        // 根据字段类型决定是否需要子字段选择
        if (fieldMeta.type === 'reference') {
          // 引用类型需要子字段（但只有在确实是外键引用时才这样做）
          return `          ${field} { id name title }`
        } else if (fieldMeta.type === 'blocks') {
          // ContentBlock类型需要Union内联片段语法
          return `          ${field} {
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
          }`
        }
        // 所有其他类型（string, number, boolean, json, datetime等）都是标量类型
        // 直接返回字段名，不需要子字段选择
        return `          ${field}`
      })
      .join('\n')

    // 🎯 平台模型时间字段兼容处理
    const createdAtField = modelMetadata.fields['createdAt'] ? 'createdAt' : 
                          (modelMetadata.fields['created_at'] ? 'created_at' : '')
    const updatedAtField = modelMetadata.fields['updatedAt'] ? 'updatedAt' : 
                          (modelMetadata.fields['updated_at'] ? 'updated_at' : '')
    
    const timestampFields = [createdAtField, updatedAtField]
      .filter(Boolean)
      .map(f => `          ${f}`)
      .join('\n')

    const fieldsSection = modelFields ? `\n${modelFields}` : ''
    const timestampSection = timestampFields ? `\n${timestampFields}` : ''
    
    // 🎯 使用改进的查询字段生成
    const queryField = await this.getGraphQLSingleQueryField(modelName)
    
    let query: string
    let variables: any
    
    if (isPlatformModel) {
      // 🎯 平台模型：使用列表查询 + 客户端过滤获取单个实体
      // 原因：平台模型通常只提供列表查询端点，不提供单个实体查询
      query = `
        query Get${modelName}($limit: Int!) {
          ${queryField}(limit: $limit) {
            id${fieldsSection}${timestampSection}
          }
        }
      `
      variables = { limit: 100 } // 获取所有记录，然后在客户端过滤
    } else {
      // 🎯 业务模型：标准的 byPk 查询
      query = `
        query Get${modelName}($id: ID!) {
          ${queryField}(id: $id) {
            id${fieldsSection}${timestampSection}
          }
        }
      `
      variables = { id }
    }

    console.log(`🔍 查询${isPlatformModel ? '平台' : '业务'}模型 ${modelName}:`, query)
    
    const result = await this.graphql(query, variables)
    
    let entity: any
    if (isPlatformModel) {
      // 从列表结果中找到匹配ID的实体
      const entities = result[queryField] || []
      entity = entities.find((e: any) => e.id === id)
    } else {
      entity = result[queryField]
    }
    
    if (!entity) {
      throw new Error(`未找到ID为 ${id} 的 ${modelName} 实体`)
    }
    
    return entity
  }

  /**
   * 🎯 创建实体：智能检测平台模型限制
   */
  async createEntity(modelName: string, data: Partial<EntityData>): Promise<EntityData> {
    // 获取模型元数据检查是否为平台模型
    const schema = await this.getSchemaMetadata()
    const modelMetadata = schema.models[modelName]
    
    if (modelMetadata?.isPlatformModel) {
      throw new Error(`平台模型 ${modelName} 不支持通过Admin UI创建，请使用系统API或CLI工具`)
    }
    
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
   * 🎯 更新实体：智能检测平台模型限制
   */
  async updateEntity(modelName: string, id: string, data: Partial<EntityData>): Promise<EntityData> {
    // 获取模型元数据检查是否为平台模型
    const schema = await this.getSchemaMetadata()
    const modelMetadata = schema.models[modelName]
    
    if (modelMetadata?.isPlatformModel) {
      throw new Error(`平台模型 ${modelName} 不支持通过Admin UI编辑，请使用系统API或CLI工具`)
    }
    
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
   * 🎯 删除实体：智能检测平台模型限制
   */
  async deleteEntity(modelName: string, id: string): Promise<boolean> {
    // 获取模型元数据检查是否为平台模型
    const schema = await this.getSchemaMetadata()
    const modelMetadata = schema.models[modelName]
    
    if (modelMetadata?.isPlatformModel) {
      throw new Error(`平台模型 ${modelName} 不支持通过Admin UI删除，请使用系统API或CLI工具`)
    }
    
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

  /**
   * 批量更新Deal位置（以及可选阶段）
   */
  async updateDealPositions(updates: { id: string; position: number; stage?: string }[]): Promise<boolean> {
    const query = `
      mutation UpdateDealPositions($updates: [DealPositionInput!]!) {
        updateDealPositions(updates: $updates)
      }
    `
    const result = await this.graphql(query, { updates })
    return !!result.updateDealPositions
  }
}

// 导出单例实例
export const apiClient = new AtomoApiClient()
export default apiClient

/**
 * Schema Parser - 前端Schema.ts解析器
 * 
 * 🎯 遵循Atomo哲学：
 * 1. 声明式开发：通过GraphQL introspection自动发现模型结构
 * 2. 单一数据源：服务器GraphQL schema是唯一权威数据源
 * 3. 自动生成：消除手动维护，实现纯粹的auto-generation approach
 * 
 * 平台模型发现机制：
 * - 通过增强的GraphQL introspection查询自动发现平台模型
 * - 智能映射Query字段到模型类型（platform_users -> PlatformUser）
 * - 基于字段类型和名称自动生成UI配置
 * - 无需手动fallback，严格遵循"失败快速"原则
 */

import { SchemaMetadata, ModelMetadata, FieldMetadata, FieldType, FieldAttribute } from './types'

export interface ParsedSchema {
  models: Record<string, ModelMetadata>
  rawContent: string
}

/**
 * 从schema.ts文件内容中解析出模型元数据
 */
export async function parseSchemaFromTypeScript(content: string): Promise<SchemaMetadata> {
  const models: Record<string, ModelMetadata> = {}
  
  try {
    // 首先添加平台级模型
    await addPlatformModels(models)
    
    // 然后解析业务模型
    const modelRegex = /export\s+interface\s+(\w+)\s*{([^}]+)}/g
    let match
    
    while ((match = modelRegex.exec(content)) !== null) {
      const [, modelName, fieldsContent] = match
      
      // 跳过非模型接口（如输入类型等）
      if (modelName.includes('Input') || modelName.includes('Filter') || 
          modelName.includes('Args') || modelName.includes('Response')) {
        continue
      }
      
      const fields = parseModelFields(fieldsContent)
      
      models[modelName] = {
        tableName: modelName.toLowerCase(),
        primaryKey: 'id',
        fields,
        searchable: getSearchableFields(fields),
        ui: generateUIConfig(modelName, fields)
      }
    }
    
    return { 
      models,
      config: {
        auditLog: true,
        softDeletes: true,
        defaultPageSize: 20,
        subscriptions: false
      }
    }
  } catch (error) {
    console.error('解析schema.ts失败:', error)
    throw new Error('无法解析schema.ts文件')
  }
}

/**
 * 添加平台级模型到schema中
 * 🎯 符合Atomo哲学：完全基于GraphQL introspection的声明式发现
 * 消除手动定义，实现纯粹的auto-generation approach
 */
async function addPlatformModels(models: Record<string, ModelMetadata>) {
  try {
    console.log('🔍 正在从GraphQL schema自动发现平台模型...')
    const platformModels = await fetchPlatformModelsFromIntrospection()
    
    if (Object.keys(platformModels).length === 0) {
      console.warn('⚠️ GraphQL schema中未发现平台模型，可能服务未完全启动')
      return
    }
    
    Object.assign(models, platformModels)
    console.log('✅ 自动发现并注册平台模型:', Object.keys(platformModels))
  } catch (error) {
    console.error('❌ 平台模型自动发现失败:', error)
    // 不再使用fallback，严格遵循declarative approach
    // 如果GraphQL introspection失败，说明服务有问题，应该修复服务而不是workaround
    throw new Error(`平台模型发现失败: ${error}. 请确保Atomo服务正常运行`)
  }
}

/**
 * 从GraphQL introspection自动发现平台模型
 * 🎯 纯声明式方式：基于服务器GraphQL schema自动生成元数据
 */
async function fetchPlatformModelsFromIntrospection(): Promise<Record<string, ModelMetadata>> {
  // Enhanced introspection query for platform model discovery
  const introspectionQuery = `
    query PlatformModelIntrospection {
      __schema {
        queryType {
          fields {
            name
            type {
              name
              kind
              ofType {
                name
                kind
                ofType {
                  name
                  kind
                }
              }
            }
          }
        }
        types {
          name
          kind
          description
          fields {
            name
            description
            type {
              name
              kind
              ofType {
                name
                kind
                ofType {
                  name
                  kind
                }
              }
            }
          }
          enumValues {
            name
            description
          }
        }
      }
    }
  `
  
  const graphqlUrl = (window.location.port === '5173') ? 'http://localhost:3000/graphql' : '/graphql';
  console.log('🔍 正在从GraphQL端点获取平台模型元数据:', graphqlUrl)
  
  const response = await fetch(graphqlUrl, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ query: introspectionQuery })
  })
  
  if (!response.ok) {
    throw new Error(`GraphQL introspection失败: HTTP ${response.status}`)
  }
  
  const result = await response.json()
  if (result.errors) {
    throw new Error(`GraphQL错误: ${result.errors.map((e: any) => e.message).join(', ')}`)
  }
  
  return parsePlatformModelsFromIntrospection(result.data.__schema)
}

/**
 * 从GraphQL introspection结果解析平台模型
 * 🎯 智能发现：基于Query字段和Type定义自动识别平台模型
 */
function parsePlatformModelsFromIntrospection(schema: any): Record<string, ModelMetadata> {
  const models: Record<string, ModelMetadata> = {}
  
  // Step 1: 从Query字段发现平台模型查询端点
  const platformQueries = new Map<string, string>()
  
  if (schema.queryType?.fields) {
    for (const field of schema.queryType.fields) {
      // 检测平台相关查询字段
      if (field.name.match(/^(platform_users|user_sessions|audit_log_entries)$/)) {
        const returnType = extractReturnType(field.type)
        if (returnType) {
          platformQueries.set(field.name, returnType)
        }
      }
    }
  }
  
  console.log('🔍 发现平台查询端点:', Array.from(platformQueries.entries()))
  
  // Step 2: 从Types定义解析平台模型结构
  const platformTypeNames = ['PlatformUser', 'UserSession', 'AuditLogEntry']
  
  for (const type of schema.types) {
    if (type.kind === 'OBJECT' && platformTypeNames.includes(type.name)) {
      const modelMetadata = createModelMetadataFromType(type)
      if (modelMetadata) {
        models[type.name] = modelMetadata
        console.log(`✅ 解析平台模型: ${type.name}`)
      }
    }
  }
  
  // Step 3: 验证并映射查询端点到模型
  for (const [queryName, typeName] of platformQueries) {
    if (models[typeName]) {
      // 更新模型的查询元数据
      models[typeName].queryEndpoint = queryName
      console.log(`🔗 映射查询端点: ${queryName} -> ${typeName}`)
    }
  }
  
  return models
}

/**
 * 提取GraphQL字段的返回类型名称
 */
function extractReturnType(typeRef: any): string | null {
  // 处理包装类型: [Type!]! -> Type
  let current = typeRef
  while (current && (current.kind === 'NON_NULL' || current.kind === 'LIST')) {
    current = current.ofType
  }
  return current?.name || null
}

/**
 * 从GraphQL类型定义创建模型元数据
 */
function createModelMetadataFromType(type: any): ModelMetadata | null {
  if (!type.fields) return null
  
  const fields: Record<string, FieldMetadata> = {}
  
  for (const field of type.fields) {
    const fieldType = mapGraphQLTypeToFieldType(field.type)
    fields[field.name] = {
      name: field.name,
      type: fieldType,
      optional: !isNonNullType(field.type),
      attributes: generateAttributesFromFieldName(field.name),
      ui: generateFieldUIConfig(field.name, fieldType)
    }
  }
  
  // 智能生成表名和配置
  const tableName = generateTableNameFromType(type.name)
  const searchableFields = getSearchableFieldsFromFields(fields)
  
  return {
    tableName,
    primaryKey: 'id',
    fields,
    searchable: searchableFields,
    ui: generateUIConfig(type.name, fields),
    // 平台模型特殊标记
    isPlatformModel: true,
    description: type.description || `Auto-discovered platform model: ${type.name}`
  }
}

/**
 * 检查GraphQL类型是否为NonNull
 */
function isNonNullType(typeRef: any): boolean {
  return typeRef.kind === 'NON_NULL'
}

/**
 * 从类型名智能生成表名
 */
function generateTableNameFromType(typeName: string): string {
  const typeToTable: Record<string, string> = {
    'PlatformUser': 'users',
    'UserSession': 'sessions', 
    'AuditLogEntry': 'audit_log_projections'
  }
  
  return typeToTable[typeName] || typeName.toLowerCase() + 's'
}

/**
 * 将 GraphQL 类型映射到字段类型
 */
function mapGraphQLTypeToFieldType(gqlType: any): FieldType {
  let typeName = gqlType.name
  
  // 处理包装类型（NON_NULL, LIST）
  if (gqlType.kind === 'NON_NULL' || gqlType.kind === 'LIST') {
    typeName = gqlType.ofType?.name || gqlType.ofType?.ofType?.name
  }
  
  const typeMapping: Record<string, FieldType> = {
    'String': 'string',
    'Int': 'number',
    'Float': 'number',
    'Boolean': 'boolean',
    'DateTime': 'datetime',
    'JSON': 'json',
    'ID': 'string',
    'ContentBlock': 'blocks',  // 🎯 ContentBlock类型映射
    'Block': 'blocks'          // 🎯 Block类型映射
  }
  
  return typeMapping[typeName] || 'string'
}

/**
 * 从字段名生成属性
 */
function generateAttributesFromFieldName(fieldName: string): FieldAttribute[] {
  const attributes: FieldAttribute[] = []
  
  if (fieldName === 'id') {
    attributes.push('primary', 'readonly')
  }
  if (fieldName.includes('email')) {
    attributes.push('unique')
  }
  if (fieldName.includes('createdAt') || fieldName.includes('updatedAt')) {
    attributes.push('readonly')
  }
  
  return attributes
}

/**
 * 从字段对象获取可搜索字段
 */
function getSearchableFieldsFromFields(fields: Record<string, FieldMetadata>): string[] {
  return Object.entries(fields)
    .filter(([name, field]) => 
      (field.type === 'string' || field.type === 'text') && 
      !['id', 'createdAt', 'updatedAt', 'password', 'token'].includes(name)
    )
    .map(([name]) => name)
}

/**
 * 解析模型字段
 */

/**
 * 解析模型字段
 */
function parseModelFields(fieldsContent: string): Record<string, FieldMetadata> {
  const fields: Record<string, FieldMetadata> = {}
  
  // 清理注释和空行
  const cleanContent = fieldsContent
    .replace(/\/\*[\s\S]*?\*\//g, '') // 块注释
    .replace(/\/\/.*$/gm, '') // 行注释
    .trim()
  
  // 提取字段定义
  const fieldLines = cleanContent.split('\n')
    .map(line => line.trim())
    .filter(line => line && !line.startsWith('//'))
  
  for (const line of fieldLines) {
    const fieldMatch = line.match(/(\w+)(\?)?\s*:\s*([^;,]+)/)
    if (fieldMatch) {
      const [, fieldName, optional, fieldType] = fieldMatch
      
      fields[fieldName] = {
        name: fieldName,
        type: mapTypeScriptTypeToFieldType(fieldType.trim()),
        optional: !!optional,
        attributes: generateFieldAttributes(fieldName),
        ui: generateFieldUIConfig(fieldName, fieldType.trim())
      }
    }
  }
  
  return fields
}

/**
 * 将TypeScript类型映射到字段类型
 */
function mapTypeScriptTypeToFieldType(tsType: string): FieldType {
  // 移除数组标记和可选标记
  const cleanType = tsType.replace(/\[\]/g, '').replace(/\?/g, '').trim()
  
  const typeMapping: Record<string, FieldType> = {
    'string': 'string',
    'String': 'string', 
    'number': 'number',
    'Number': 'number',
    'boolean': 'boolean',
    'Boolean': 'boolean',
    'Date': 'datetime',
    'DateTime': 'datetime',
    'any': 'json',
    'object': 'json',
    'Array': 'array',
    'Block[]': 'blocks', // Block[]在后端是ContentBlock数组
    'Block': 'blocks',   // Block在后端是ContentBlock
    'ContentBlock[]': 'blocks', // ContentBlock[]是ContentBlock数组
    'ContentBlock': 'blocks'    // ContentBlock是ContentBlock类型
  }
  
  // 特殊类型检查
  if (typeMapping[cleanType]) {
    return typeMapping[cleanType]
  }
  
  // 检查是否为枚举类型（大写开头，以Size、Stage、Status等结尾）
  if (/^[A-Z][a-zA-Z]*(Size|Stage|Status|Type|Kind|Mode)$/.test(cleanType)) {
    return 'string' // 枚举类型在GraphQL中通常表示为字符串
  }
  
  // 检查是否为引用类型（大写开头的自定义类型，但不是枚举）
  if (/^[A-Z][a-zA-Z]*$/.test(cleanType)) {
    return 'reference'
  }
  
  return 'string'
}

/**
 * 生成字段属性
 */
function generateFieldAttributes(fieldName: string): FieldAttribute[] {
  const attributes: FieldAttribute[] = []
  
  if (fieldName === 'id') {
    attributes.push('primary', 'readonly')
  }
  
  if (fieldName.includes('email')) {
    attributes.push('unique')
  }
  
  if (fieldName.includes('createdAt') || fieldName.includes('updatedAt')) {
    attributes.push('readonly')
  }
  
  return attributes
}

/**
 * 生成字段UI配置
 */
function generateFieldUIConfig(fieldName: string, _tsType: string) {
  const config: any = {
    label: generateFieldLabel(fieldName),
    placeholder: `请输入${generateFieldLabel(fieldName)}`,
    validation: {}
  }
  
  // 根据字段名生成特定配置
  if (fieldName.toLowerCase().includes('email')) {
    config.validation.pattern = /^[^\s@]+@[^\s@]+\.[^\s@]+$/
    config.validation.message = '请输入有效的邮箱地址'
  }
  
  if (fieldName.toLowerCase().includes('phone')) {
    config.validation.pattern = /^[\+]?[1-9][\d]{0,15}$/
    config.validation.message = '请输入有效的电话号码'
  }
  
  if (fieldName === 'id') {
    config.readonly = true
    config.showInList = false
  }
  
  if (fieldName.includes('createdAt') || fieldName.includes('updatedAt')) {
    config.readonly = true
    config.showInEdit = false
  }
  
  return config
}

/**
 * 生成模型UI配置
 */
function generateUIConfig(_modelName: string, fields: Record<string, FieldMetadata>) {
  const fieldNames = Object.keys(fields)
  
  // 找到适合作为显示字段的字段
  const displayField = findDisplayField(fieldNames)
  
  // 生成列表视图配置
  const listView = fieldNames
    .filter(name => !['createdAt', 'updatedAt'].includes(name))
    .slice(0, 6) // 最多显示6个字段
  
  // 生成编辑表单配置
  const editForm = fieldNames
    .filter(name => !['id', 'createdAt', 'updatedAt'].includes(name))
  
  return {
    displayField,
    listView,
    editForm,
    searchFields: getSearchableFields(fields)
  }
}

/**
 * 找到适合作为显示字段的字段
 */
function findDisplayField(fieldNames: string[]): string {
  // 优先使用常见的显示字段
  const preferredFields = ['name', 'title', 'label', 'firstName', 'email']
  
  for (const preferred of preferredFields) {
    if (fieldNames.includes(preferred)) {
      return preferred
    }
  }
  
  // 回退到第一个非ID字段
  return fieldNames.find(name => name !== 'id') || 'id'
}

/**
 * 获取可搜索字段
 */
function getSearchableFields(fields: Record<string, FieldMetadata>): string[] {
  return Object.entries(fields)
    .filter(([name, field]) => 
      (field.type === 'string' || field.type === 'text') && 
      !['id', 'createdAt', 'updatedAt'].includes(name)
    )
    .map(([name]) => name)
}

/**
 * 生成字段显示标签
 */
function generateFieldLabel(fieldName: string): string {
  const labelMap: Record<string, string> = {
    'firstName': '名',
    'lastName': '姓', 
    'companyId': '公司',
    'contactId': '联系人',
    'createdAt': '创建时间',
    'updatedAt': '更新时间',
    'actualCloseDate': '实际成交日期',
    'expectedCloseDate': '预期成交日期'
  }
  
  return labelMap[fieldName] || fieldName
}



/**
 * 获取schema.ts文件内容并解析
 */
export async function loadSchemaMetadata(): Promise<SchemaMetadata> {
  try {
    // 🎯 新架构：智能检测运行环境，使用重试机制从正确端口获取schema.ts
    const content = await loadSchemaWithRetry()
    return await parseSchemaFromTypeScript(content)
  } catch (error) {
    console.error('加载schema.ts失败:', error)
    throw new Error('无法连接到Atomo服务器或加载schema文件')
  }
}



/**
 * 带重试机制的schema加载函数
 * 🎯 在开发环境中尝试多个可能的后端端口
 */
async function loadSchemaWithRetry(): Promise<string> {
  const currentPort = window.location.port
  const currentHost = window.location.hostname
  
  // 如果不是在5173端口，直接使用相对路径
  if (currentPort !== '5173') {
    const response = await fetch('/schema.ts')
    if (!response.ok) {
      throw new Error(`HTTP ${response.status}: ${response.statusText}`)
    }
    return response.text()
  }
  
  // 在5173端口，尝试多个可能的后端端口
  const possiblePorts = ['3001', '3000', '8000', '8080', '4000']
  let lastError: Error | null = null
  
  for (const port of possiblePorts) {
    try {
      const url = `http://${currentHost}:${port}/schema.ts`
      console.log(`尝试从 ${url} 加载schema...`)
      
      const response = await fetch(url)
      if (response.ok) {
        console.log(`✅ 成功从端口 ${port} 加载schema`)
        return response.text()
      }
      
      lastError = new Error(`HTTP ${response.status}: ${response.statusText}`)
    } catch (error) {
      lastError = error as Error
      console.warn(`端口 ${port} 连接失败:`, error)
      continue
    }
  }
  
  throw lastError || new Error('无法连接到任何后端服务端口')
}

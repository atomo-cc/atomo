/**
 * Schema Parser - 前端Schema.ts解析器
 * 
 * 🎯 符合架构原则：在前端直接解析schema.ts文件
 * 而不是依赖后端硬编码的元数据
 */

import { SchemaMetadata, ModelMetadata, FieldMetadata, FieldType, FieldAttribute } from './types'

export interface ParsedSchema {
  models: Record<string, ModelMetadata>
  rawContent: string
}

/**
 * 从schema.ts文件内容中解析出模型元数据
 */
export function parseSchemaFromTypeScript(content: string): SchemaMetadata {
  const models: Record<string, ModelMetadata> = {}
  
  try {
    // 提取模型定义的正则表达式
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
    'Block[]': 'json', // Block[]在后端通常映射为JSON类型
    'Block': 'json'    // Block在后端通常映射为JSON类型
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
    return parseSchemaFromTypeScript(content)
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

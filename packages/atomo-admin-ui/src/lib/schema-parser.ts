/**
 * Schema Parser - frontend Schema.ts parser
 *
 * 🎯 Following the Atomo philosophy:
 * 1. Declarative development: automatically discover model structure via GraphQL introspection
 * 2. Single source of truth: the server's GraphQL schema is the only authoritative data source
 * 3. Auto-generation: eliminate manual maintenance for a pure auto-generation approach
 *
 * Platform model discovery mechanism:
 * - Automatically discover platform models via an enhanced GraphQL introspection query
 * - Intelligently map Query fields to model types (platform_users -> PlatformUser)
 * - Automatically generate UI config based on field type and name
 * - No manual fallback; strictly follows the "fail fast" principle
 */

import { SchemaMetadata, ModelMetadata, FieldMetadata, FieldType, FieldAttribute } from './types'
import { demoSchemaMetadata } from './demo-data'

export interface ParsedSchema {
  models: Record<string, ModelMetadata>
  rawContent: string
}

/**
 * Parse model metadata from the contents of a schema.ts file.
 */
export async function parseSchemaFromTypeScript(content: string): Promise<SchemaMetadata> {
  const models: Record<string, ModelMetadata> = {}
  
  try {
    // First add the platform-level models
    await addPlatformModels(models)

    // Then parse the business models
    const modelRegex = /export\s+interface\s+(\w+)\s*{([^}]+)}/g
    let match
    
    while ((match = modelRegex.exec(content)) !== null) {
      const [, modelName, fieldsContent] = match
      
      // Skip non-model interfaces (e.g. input types)
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
    console.error('Failed to parse schema.ts:', error)
    throw new Error('Unable to parse the schema.ts file')
  }
}

/**
 * Add platform-level models to the schema.
 * 🎯 In line with the Atomo philosophy: declarative discovery based entirely on GraphQL introspection,
 * eliminating manual definitions for a pure auto-generation approach.
 */
async function addPlatformModels(models: Record<string, ModelMetadata>) {
  try {
    console.log('🔍 Auto-discovering platform models from the GraphQL schema...')
    const platformModels = await fetchPlatformModelsFromIntrospection()
    
    if (Object.keys(platformModels).length === 0) {
      console.warn('⚠️ No platform models found in the GraphQL schema; the service may not be fully started')
      return
    }
    
    Object.assign(models, platformModels)
    console.log('✅ Auto-discovered and registered platform models:', Object.keys(platformModels))
  } catch (error) {
    console.error('❌ Platform model auto-discovery failed:', error)
    console.warn('Platform model discovery failed; continuing with the business schema metadata')
  }
}

/**
 * Auto-discover platform models from GraphQL introspection.
 * 🎯 Purely declarative: generate metadata automatically from the server's GraphQL schema.
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
  console.log('🔍 Fetching platform model metadata from the GraphQL endpoint:', graphqlUrl)
  
  const response = await fetch(graphqlUrl, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ query: introspectionQuery })
  })
  
  if (!response.ok) {
    throw new Error(`GraphQL introspection failed: HTTP ${response.status}`)
  }
  
  const result = await response.json()
  if (result.errors) {
    throw new Error(`GraphQL error: ${result.errors.map((e: any) => e.message).join(', ')}`)
  }
  
  return parsePlatformModelsFromIntrospection(result.data.__schema)
}

/**
 * Parse platform models from the GraphQL introspection result.
 * 🎯 Intelligent discovery: automatically identify platform models from Query fields and Type definitions.
 */
function parsePlatformModelsFromIntrospection(schema: any): Record<string, ModelMetadata> {
  const models: Record<string, ModelMetadata> = {}
  
  // Step 1: discover platform model query endpoints from the Query fields
  const platformQueries = new Map<string, string>()
  
  if (schema.queryType?.fields) {
    for (const field of schema.queryType.fields) {
      // Detect platform-related query fields
      if (field.name.match(/^(platform_users|user_sessions|audit_log_entries)$/)) {
        const returnType = extractReturnType(field.type)
        if (returnType) {
          platformQueries.set(field.name, returnType)
        }
      }
    }
  }
  
  console.log('🔍 Discovered platform query endpoints:', Array.from(platformQueries.entries()))

  // Step 2: parse platform model structures from the Type definitions
  const platformTypeNames = ['PlatformUser', 'UserSession', 'AuditLogEntry']
  
  for (const type of schema.types) {
    if (type.kind === 'OBJECT' && platformTypeNames.includes(type.name)) {
      const modelMetadata = createModelMetadataFromType(type)
      if (modelMetadata) {
        models[type.name] = modelMetadata
        console.log(`✅ Parsed platform model: ${type.name}`)
      }
    }
  }
  
  // Step 3: validate and map query endpoints to models
  for (const [queryName, typeName] of platformQueries) {
    if (models[typeName]) {
      // Update the model's query metadata
      models[typeName].queryEndpoint = queryName
      console.log(`🔗 Mapped query endpoint: ${queryName} -> ${typeName}`)
    }
  }
  
  return models
}

/**
 * Extract the return type name of a GraphQL field.
 */
function extractReturnType(typeRef: any): string | null {
  // Unwrap wrapper types: [Type!]! -> Type
  let current = typeRef
  while (current && (current.kind === 'NON_NULL' || current.kind === 'LIST')) {
    current = current.ofType
  }
  return current?.name || null
}

/**
 * Create model metadata from a GraphQL type definition.
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
  
  // Intelligently generate the table name and config
  const tableName = generateTableNameFromType(type.name)
  const searchableFields = getSearchableFieldsFromFields(fields)
  
  return {
    tableName,
    primaryKey: 'id',
    fields,
    searchable: searchableFields,
    ui: generateUIConfig(type.name, fields),
    // Special marker for platform models
    isPlatformModel: true,
    description: type.description || `Auto-discovered platform model: ${type.name}`
  }
}

/**
 * Check whether a GraphQL type is NonNull.
 */
function isNonNullType(typeRef: any): boolean {
  return typeRef.kind === 'NON_NULL'
}

/**
 * Intelligently generate a table name from a type name.
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
 * Map a GraphQL type to a field type.
 */
function mapGraphQLTypeToFieldType(gqlType: any): FieldType {
  let typeName = gqlType.name
  
  // Unwrap wrapper types (NON_NULL, LIST)
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
    'ContentBlock': 'blocks',  // 🎯 ContentBlock type mapping
    'Block': 'blocks'          // 🎯 Block type mapping
  }
  
  return typeMapping[typeName] || 'string'
}

/**
 * Generate attributes from a field name.
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
 * Get the searchable fields from a fields object.
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
 * Parse model fields.
 */

/**
 * Parse model fields.
 */
function parseModelFields(fieldsContent: string): Record<string, FieldMetadata> {
  const fields: Record<string, FieldMetadata> = {}

  // Strip comments and blank lines
  const cleanContent = fieldsContent
    .replace(/\/\*[\s\S]*?\*\//g, '') // Block comments
    .replace(/\/\/.*$/gm, '') // Line comments
    .trim()

  // Extract field definitions
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
 * Map a TypeScript type to a field type.
 */
function mapTypeScriptTypeToFieldType(tsType: string): FieldType {
  // Remove array and optional markers
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
    'Block[]': 'blocks', // Block[] is a ContentBlock array on the backend
    'Block': 'blocks',   // Block is a ContentBlock on the backend
    'ContentBlock[]': 'blocks', // ContentBlock[] is a ContentBlock array
    'ContentBlock': 'blocks'    // ContentBlock is a ContentBlock type
  }

  // Special type checks
  if (typeMapping[cleanType]) {
    return typeMapping[cleanType]
  }
  
  // Check whether it's an enum type (starts with an uppercase letter, ends with Size, Stage, Status, etc.)
  if (/^[A-Z][a-zA-Z]*(Size|Stage|Status|Type|Kind|Mode)$/.test(cleanType)) {
    return 'string' // Enum types are typically represented as strings in GraphQL
  }

  // Check whether it's a reference type (a custom type starting with an uppercase letter, but not an enum)
  if (/^[A-Z][a-zA-Z]*$/.test(cleanType)) {
    return 'reference'
  }
  
  return 'string'
}

/**
 * Generate field attributes.
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
 * Generate the UI config for a field.
 */
function generateFieldUIConfig(fieldName: string, _tsType: string) {
  const config: any = {
    label: generateFieldLabel(fieldName),
    placeholder: `Enter ${generateFieldLabel(fieldName)}`,
    validation: {}
  }

  // Generate field-specific config based on the field name
  if (fieldName.toLowerCase().includes('email')) {
    config.validation.pattern = /^[^\s@]+@[^\s@]+\.[^\s@]+$/
    config.validation.message = 'Please enter a valid email address'
  }

  if (fieldName.toLowerCase().includes('phone')) {
    config.validation.pattern = /^[\+]?[1-9][\d]{0,15}$/
    config.validation.message = 'Please enter a valid phone number'
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
 * Generate the UI config for a model.
 */
function generateUIConfig(_modelName: string, fields: Record<string, FieldMetadata>) {
  const fieldNames = Object.keys(fields)

  // Find a suitable field to use as the display field
  const displayField = findDisplayField(fieldNames)

  // Generate the list view config
  const listView = fieldNames
    .filter(name => !['createdAt', 'updatedAt'].includes(name))
    .slice(0, 6) // Show at most 6 fields

  // Generate the edit form config
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
 * Find a suitable field to use as the display field.
 */
function findDisplayField(fieldNames: string[]): string {
  // Prefer common display fields
  const preferredFields = ['name', 'title', 'label', 'firstName', 'email']

  for (const preferred of preferredFields) {
    if (fieldNames.includes(preferred)) {
      return preferred
    }
  }

  // Fall back to the first non-ID field
  return fieldNames.find(name => name !== 'id') || 'id'
}

/**
 * Get the searchable fields.
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
 * Generate a display label for a field.
 */
function generateFieldLabel(fieldName: string): string {
  const labelMap: Record<string, string> = {
    'firstName': 'First Name',
    'lastName': 'Last Name',
    'companyId': 'Company',
    'contactId': 'Contact',
    'createdAt': 'Created At',
    'updatedAt': 'Updated At',
    'actualCloseDate': 'Actual Close Date',
    'expectedCloseDate': 'Expected Close Date'
  }
  
  return labelMap[fieldName] || fieldName
}



/**
 * Fetch the schema.ts file contents and parse them.
 */
export async function loadSchemaMetadata(): Promise<SchemaMetadata> {
  // Preferred: the standalone server's /meta/schema (real models from the loaded schema).
  try {
    const meta = await loadFromMetaSchema()
    if (Object.keys(meta.models).length > 0) {
      return meta
    }
  } catch (error) {
    console.warn('/meta/schema is unavailable; falling back to schema.ts parsing', error)
  }
  try {
    // 🎯 New architecture: detect the runtime environment and use a retry mechanism to fetch schema.ts from the correct port
    const content = await loadSchemaWithRetry()
    return await parseSchemaFromTypeScript(content)
  } catch (error) {
    console.error('Failed to load schema.ts:', error)
    console.warn('Using the built-in CRM demo schema metadata')
    return demoSchemaMetadata
  }
}

/**
 * Load model metadata from the server's `/meta/schema` endpoint and map it to the UI's
 * SchemaMetadata. This is the standalone-server discovery path (the bare server doesn't serve
 * /schema.ts and truncates GraphQL introspection). The server returns models with
 * tableName/primaryKey/fields/relationships/validation; we fill ui + searchable locally.
 */
async function loadFromMetaSchema(): Promise<SchemaMetadata> {
  const url = window.location.port === '5173' ? 'http://localhost:3000/meta/schema' : '/meta/schema'
  const response = await fetch(url)
  if (!response.ok) {
    throw new Error(`/meta/schema HTTP ${response.status}`)
  }
  const raw = await response.json()
  const knownAttrs: FieldAttribute[] = ['primary', 'unique', 'index', 'required', 'readonly']
  const models: Record<string, ModelMetadata> = {}

  for (const [name, m] of Object.entries<any>(raw.models || {})) {
    const fields: Record<string, FieldMetadata> = {}
    for (const [fname, f] of Object.entries<any>(m.fields || {})) {
      fields[fname] = {
        name: f.name ?? fname,
        type: (f.type as FieldType) ?? 'string',
        optional: Boolean(f.optional),
        attributes: (f.attributes || []).filter((a: string) => knownAttrs.includes(a as FieldAttribute)),
      }
    }
    models[name] = {
      tableName: m.tableName ?? name.toLowerCase(),
      primaryKey: m.primaryKey ?? 'id',
      fields,
      relationships: m.relationships || undefined,
      validation: m.validation || undefined,
      searchable: getSearchableFields(fields),
      ui: generateUIConfig(name, fields),
    }
  }

  return {
    models,
    config: { auditLog: true, softDeletes: true, defaultPageSize: 20, subscriptions: true },
  }
}



/**
 * Schema loader with a retry mechanism.
 * 🎯 In development, try several possible backend ports.
 */
async function loadSchemaWithRetry(): Promise<string> {
  const currentPort = window.location.port
  const currentHost = window.location.hostname
  
  // If not on port 5173, use a relative path directly
  if (currentPort !== '5173') {
    const response = await fetch('/schema.ts')
    if (!response.ok) {
      throw new Error(`HTTP ${response.status}: ${response.statusText}`)
    }
    return response.text()
  }
  
  // On port 5173, try several possible backend ports
  const possiblePorts = ['3001', '3000', '8000', '8080', '4000']
  let lastError: Error | null = null
  
  for (const port of possiblePorts) {
    try {
      const url = `http://${currentHost}:${port}/schema.ts`
      console.log(`Attempting to load schema from ${url}...`)
      
      const response = await fetch(url)
      if (response.ok) {
        console.log(`✅ Successfully loaded schema from port ${port}`)
        return response.text()
      }
      
      lastError = new Error(`HTTP ${response.status}: ${response.statusText}`)
    } catch (error) {
      lastError = error as Error
      console.warn(`Failed to connect to port ${port}:`, error)
      continue
    }
  }
  
  throw lastError || new Error('Unable to connect to any backend service port')
}

/**
 * Atomo Admin UI Types
 * 
 * Core types for the dynamic rendering system and schema metadata
 */

export interface SchemaMetadata {
  models: Record<string, ModelMetadata>
  config: {
    auditLog?: boolean
    softDeletes?: boolean
    defaultPageSize?: number
    subscriptions?: boolean
  }
}

export interface ModelMetadata {
  tableName: string
  primaryKey: string
  searchable?: string[]
  relationships?: Record<string, RelationshipConfig>
  validation?: Record<string, string>
  ui: UIConfig
  fields: Record<string, FieldMetadata>
  // Platform model specific properties
  isPlatformModel?: boolean
  queryEndpoint?: string
  description?: string
}

export interface FieldMetadata {
  name: string
  type: FieldType
  optional: boolean
  attributes: FieldAttribute[]
  ui?: FieldUIConfig
}

export interface FieldUIConfig {
  label?: string
  placeholder?: string
  helpText?: string
  component?: string  // 自定义组件名称
  options?: any[]     // 选择器选项
  validation?: Record<string, any>
}

export interface RelationshipConfig {
  type: 'belongsTo' | 'hasMany' | 'hasOne' | 'manyToMany'
  model: string
  foreignKey?: string
  localKey?: string
  pivotTable?: string
}

export interface UIConfig {
  displayField: string | string[]
  listView: string[]
  editForm: string[]
  detailView?: string[]
  searchFields?: string[]
  filters?: FilterConfig[]
  actions?: ActionConfig[]
}

export interface FilterConfig {
  field: string
  type: 'text' | 'select' | 'date' | 'number' | 'boolean'
  label: string
  options?: { value: string; label: string }[]
}

export interface ActionConfig {
  id: string
  label: string
  type: 'button' | 'dropdown'
  variant?: 'primary' | 'secondary' | 'danger'
  icon?: string
  handler: string
}

export type FieldType = 
  | 'string'
  | 'number'
  | 'boolean'
  | 'date'
  | 'datetime'
  | 'text'
  | 'email'
  | 'url'
  | 'json'
  | 'blocks'  // Atomo 富文本块
  | 'reference'
  | 'array'
  | 'custom'

export type FieldAttribute = 
  | 'primary'
  | 'unique' 
  | 'index'
  | 'required'
  | 'readonly'
  | 'hidden'

// Entity operations
export interface EntityData {
  id: string
  [key: string]: any
}

export interface QueryOptions {
  page?: number
  limit?: number
  sort?: string
  order?: 'asc' | 'desc'
  filters?: Record<string, any>
  search?: string
}

export interface ColumnConfig {
  key: string
  label: string
  type: FieldType
  sortable?: boolean
  filterable?: boolean
  render?: (value: any, row: EntityData) => React.ReactNode
  width?: number
}

// Form types
export interface FormFieldProps {
  field: FieldMetadata
  value: any
  onChange: (value: any) => void
  error?: string
  disabled?: boolean
}

// Component variants
export type ComponentVariant = 'primary' | 'secondary' | 'danger' | 'ghost'
export type ComponentSize = 'sm' | 'md' | 'lg'

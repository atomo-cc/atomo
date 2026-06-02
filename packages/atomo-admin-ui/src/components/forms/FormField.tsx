/**
 * Form Field - 动态表单字段组件
 * 
 * 根据字段类型自动选择合适的输入组件
 */


import { FieldMetadata, SchemaMetadata, ModelMetadata } from '../../lib/types'
import { Input } from '../ui/Input'
import { Textarea } from '../ui/Textarea'

import { Switch } from '../ui/Switch'
import { DatePicker } from '../ui/DatePicker'
import { ReferenceSelect } from './ReferenceSelect'
import { TagInput } from './TagInput'
import { BlocksEditor } from './BlocksEditor'
import { JsonEditor } from './JsonEditor'
import { MediaUploader } from '../upload/MediaUploader'
import { WasmPlugin, WasmPluginConfig } from '../plugins/WasmPluginSystem'
import { getFieldLabel } from '../../lib/utils'

interface FormFieldProps {
  field: FieldMetadata
  value: any
  onChange: (value: any) => void
  error?: string
  disabled?: boolean
  modelMetadata: ModelMetadata
  schema: SchemaMetadata
}

export function FormField({
  field,
  value,
  onChange,
  error,
  disabled = false,
  modelMetadata,
  schema
}: FormFieldProps) {
  const fieldConfig = field.ui || {}
  const label = getFieldLabel(field.name, fieldConfig.label)
  const placeholder = fieldConfig.placeholder
  const helpText = fieldConfig.helpText

  // 根据字段类型渲染不同的输入组件
  const renderInput = () => {
    switch (field.type) {
      case 'string':
        if (field.name.toLowerCase().includes('email')) {
                  return (
          <Input
            type="email"
            value={value || ''}
            onChange={(e) => onChange(e.target.value)}
            placeholder={placeholder}
            disabled={disabled}
          />
        )
        }
        
        if (field.name.toLowerCase().includes('url')) {
          return (
            <Input
              type="url"
              value={value || ''}
              onChange={(e) => onChange(e.target.value)}
              placeholder={placeholder}
              disabled={disabled}
            />
          )
        }
        
        return (
          <Input
            value={value || ''}
            onChange={(e) => onChange(e.target.value)}
            placeholder={placeholder}
            disabled={disabled}
          />
        )

      case 'text':
        return (
          <Textarea
            value={value || ''}
            onChange={(e) => onChange(e.target.value)}
            placeholder={placeholder}
            disabled={disabled}
          />
        )

      case 'number':
        return (
          <Input
            type="number"
            value={value || ''}
            onChange={(e) => onChange(Number(e.target.value))}
            placeholder={placeholder}
            disabled={disabled}
          />
        )

      case 'boolean':
        return (
          <Switch
            checked={Boolean(value)}
            onCheckedChange={onChange}
            disabled={disabled}
          />
        )

      case 'date':
        return (
          <DatePicker
            value={value}
            onChange={onChange}
            disabled={disabled}
          />
        )

      case 'datetime':
        return (
          <DatePicker
            value={value}
            onChange={onChange}
            showTime
            disabled={disabled}
          />
        )

      case 'reference':
        // 引用字段需要获取关联模型信息
        const relationshipKey = field.name.replace(/Id$/, '')
        const relationship = modelMetadata.relationships?.[relationshipKey]
        
        if (!relationship) {
          return (
            <Input
              value={value || ''}
              onChange={(e) => onChange(e.target.value)}
              placeholder={`${field.name} (关系未定义)`}
              disabled={disabled}
            />
          )
        }
        
        return (
          <ReferenceSelect
            value={value}
            onChange={onChange}
            relationship={relationship}
            schema={schema}
            disabled={disabled}
          />
        )

      case 'array':
        // 处理数组字段，如标签
        if (field.name.toLowerCase().includes('tag')) {
          return (
            <TagInput
              value={value || []}
              onChange={onChange}
              disabled={disabled}
            />
          )
        }
        
        return (
          <Textarea
            value={Array.isArray(value) ? value.join('\n') : ''}
            onChange={(e) => onChange(e.target.value.split('\n').filter(Boolean))}
            placeholder="每行一个项目"
            disabled={disabled}
          />
        )

      case 'blocks':
        // 🎯 Atomo 的富文本块系统 - 智能数据格式转换
        // GraphQL返回的ContentBlock数据需要转换为FlowCanvas期望的格式
        const normalizeBlocksValue = (rawValue: any) => {
          // 空值处理
          if (!rawValue) {
            return { nodes: [], connections: [] }
          }
          
          // 如果已经是正确的格式（包含nodes和connections）
          if (rawValue.nodes && Array.isArray(rawValue.nodes)) {
            return {
              nodes: rawValue.nodes || [],
              connections: rawValue.connections || []
            }
          }
          
          // 如果是数组格式的ContentBlock数据，转换为FlowCanvas格式
          if (Array.isArray(rawValue)) {
            return {
              nodes: rawValue.map((block: any, index: number) => ({
                id: block.metadata?.id || `block-${index}`,
                type: (block.type || 'text') as any,
                position: { x: 50, y: 50 + index * 100 },
                size: { width: 200, height: 100 },
                data: {
                  content: block.content || '',
                  properties: {
                    ...block,
                    order: block.metadata?.order || index
                  }
                }
              })),
              connections: []
            }
          }
          
          // 如果是单个ContentBlock对象
          if (typeof rawValue === 'object') {
            return {
              nodes: [{
                id: rawValue.metadata?.id || 'block-0',
                type: (rawValue.type || 'text') as any,
                position: { x: 50, y: 50 },
                size: { width: 200, height: 100 },
                data: {
                  content: rawValue.content || '',
                  properties: {
                    ...rawValue,
                    order: rawValue.metadata?.order || 0
                  }
                }
              }],
              connections: []
            }
          }
          
          // 默认返回空结构
          console.warn('无法识别的ContentBlock数据格式:', rawValue)
          return { nodes: [], connections: [] }
        }
        
        return (
          <BlocksEditor
            value={normalizeBlocksValue(value)}
            onChange={(newValue) => {
              // 将FlowCanvas格式转换回ContentBlock格式以保存
              const blocks = newValue.nodes.map(node => ({
                type: node.type,
                content: node.data.content || '',
                metadata: {
                  id: node.id,
                  order: node.data.properties?.order || 0
                }
              }))
              onChange(blocks)
            }}
            disabled={disabled}
          />
        )

      case 'json':
        return (
          <JsonEditor
            value={value}
            onChange={onChange}
            disabled={disabled}
            placeholder={placeholder || "请输入有效的JSON"}
          />
        )

      case 'file':
        // Auto-rendered for File-typed schema fields.
        return (
          <MediaUploader
            value={value || []}
            onChange={onChange}
            disabled={disabled}
            accept="image/*,video/*,audio/*,.pdf,.zip"
            maxFiles={fieldConfig.options?.[0]?.maxFiles ?? 10}
            multiple={true}
            showPreview={true}
          />
        )

      case 'custom':
        // 自定义字段类型，可以通过 fieldConfig.component 指定组件
        if (fieldConfig.component === 'media-uploader') {
          return (
            <MediaUploader
              value={value || []}
              onChange={onChange}
              disabled={disabled}
              accept="image/*,video/*,audio/*,.pdf,.doc,.docx"
              maxFiles={10}
              maxFileSize={10 * 1024 * 1024}
              multiple={true}
              showPreview={true}
            />
          )
        }
        
        if (fieldConfig.component && fieldConfig.component.startsWith('wasm:')) {
          // WASM 插件组件
          const pluginId = fieldConfig.component.replace('wasm:', '')
          const pluginConfig: WasmPluginConfig = {
            id: pluginId,
            name: `Field Plugin: ${field.name}`,
            version: '1.0.0',
            isDevelopment: (import.meta as any).env?.DEV || false,
            jsUrl: (import.meta as any).env?.DEV 
              ? `/plugins/${pluginId}/index.js` 
              : undefined,
            wasmUrl: !(import.meta as any).env?.DEV 
              ? `/plugins/${pluginId}/index.wasm` 
              : undefined
          }
          
          return (
            <WasmPlugin
              config={pluginConfig}
              props={{
                field: field.name,
                value: value,
                disabled: disabled,
                placeholder: placeholder
              }}
              onEvent={(event, data) => {
                if (event === 'change') {
                  onChange(data.value)
                }
              }}
            />
          )
        }
        
        if (fieldConfig.component) {
          // 其他自定义组件
          return (
            <div className="p-4 border border-dashed border-gray-300 rounded-md text-center text-gray-500">
              自定义组件: {fieldConfig.component}
              <br />
              <small>请确保组件已正确注册</small>
            </div>
          )
        }
        
        return (
          <Input
            value={value || ''}
            onChange={(e) => onChange(e.target.value)}
            placeholder={placeholder || '自定义字段'}
            disabled={disabled}
          />
        )

      default:
        return (
          <Input
            value={value || ''}
            onChange={(e) => onChange(e.target.value)}
            placeholder={placeholder}
            disabled={disabled}
          />
        )
    }
  }

  return (
    <div className="space-y-2">
      <label className="text-sm font-medium text-gray-700">
        {label}
        {!field.optional && <span className="text-danger-500 ml-1">*</span>}
      </label>
      
      {renderInput()}
      
      {helpText && !error && (
        <p className="text-sm text-gray-500">{helpText}</p>
      )}
      
      {error && (
        <p className="text-sm text-danger-600">{error}</p>
      )}
    </div>
  )
}

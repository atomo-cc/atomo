/**
 * Form Field - 动态表单字段组件
 * 
 * 根据字段类型自动选择合适的输入组件
 */

import React from 'react'
import { FieldMetadata, SchemaMetadata, ModelMetadata } from '../../lib/types'
import { Input } from '../ui/Input'
import { Textarea } from '../ui/Textarea'
import { Select } from '../ui/Select'
import { Switch } from '../ui/Switch'
import { DatePicker } from '../ui/DatePicker'
import { ReferenceSelect } from './ReferenceSelect'
import { TagInput } from './TagInput'
import { BlocksEditor } from './BlocksEditor'
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
              error={error}
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
              error={error}
            />
          )
        }
        
        return (
          <Input
            value={value || ''}
            onChange={(e) => onChange(e.target.value)}
            placeholder={placeholder}
            disabled={disabled}
            error={error}
          />
        )

      case 'text':
        return (
          <Textarea
            value={value || ''}
            onChange={(e) => onChange(e.target.value)}
            placeholder={placeholder}
            disabled={disabled}
            error={error}
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
            error={error}
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
            error={error}
          />
        )

      case 'datetime':
        return (
          <DatePicker
            value={value}
            onChange={onChange}
            showTime
            disabled={disabled}
            error={error}
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
              error={error}
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
            error={error}
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
              error={error}
            />
          )
        }
        
        return (
          <Textarea
            value={Array.isArray(value) ? value.join('\n') : ''}
            onChange={(e) => onChange(e.target.value.split('\n').filter(Boolean))}
            placeholder="每行一个项目"
            disabled={disabled}
            error={error}
          />
        )

      case 'blocks':
        // Atomo 的富文本块系统
        return (
          <BlocksEditor
            value={value || []}
            onChange={onChange}
            disabled={disabled}
            error={error}
          />
        )

      case 'json':
        return (
          <Textarea
            value={value ? JSON.stringify(value, null, 2) : ''}
            onChange={(e) => {
              try {
                const parsed = JSON.parse(e.target.value)
                onChange(parsed)
              } catch {
                // 暂时不处理解析错误，让用户继续编辑
              }
            }}
            placeholder="JSON 格式"
            disabled={disabled}
            error={error}
            className="font-mono"
          />
        )

      case 'custom':
        // 自定义字段类型，可以通过 fieldConfig.component 指定组件
        if (fieldConfig.component) {
          // TODO: 动态加载自定义组件或 WASM 插件
          return (
            <div className="p-4 border border-dashed border-gray-300 rounded-md text-center text-gray-500">
              自定义组件: {fieldConfig.component}
              <br />
              <small>WASM 插件系统正在开发中</small>
            </div>
          )
        }
        
        return (
          <Input
            value={value || ''}
            onChange={(e) => onChange(e.target.value)}
            placeholder={placeholder || '自定义字段'}
            disabled={disabled}
            error={error}
          />
        )

      default:
        return (
          <Input
            value={value || ''}
            onChange={(e) => onChange(e.target.value)}
            placeholder={placeholder}
            disabled={disabled}
            error={error}
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

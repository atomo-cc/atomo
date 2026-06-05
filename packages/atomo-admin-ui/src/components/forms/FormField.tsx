/**
 * Form Field - dynamic form field component
 *
 * Automatically selects the appropriate input component based on the field type
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

  // Render a different input component depending on the field type
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
        // Reference fields need to look up the related model information
        const relationshipKey = field.name.replace(/Id$/, '')
        const relationship = modelMetadata.relationships?.[relationshipKey]
        
        if (!relationship) {
          return (
            <Input
              value={value || ''}
              onChange={(e) => onChange(e.target.value)}
              placeholder={`${field.name} (relationship not defined)`}
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
        // Handle array fields, such as tags
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
            placeholder="One item per line"
            disabled={disabled}
          />
        )

      case 'blocks':
        // 🎯 Atomo's rich-text block system - smart data-format conversion
        // ContentBlock data returned by GraphQL must be converted to the format FlowCanvas expects
        const normalizeBlocksValue = (rawValue: any) => {
          // Handle empty values
          if (!rawValue) {
            return { nodes: [], connections: [] }
          }

          // If it is already in the correct format (contains nodes and connections)
          if (rawValue.nodes && Array.isArray(rawValue.nodes)) {
            return {
              nodes: rawValue.nodes || [],
              connections: rawValue.connections || []
            }
          }
          
          // If it is ContentBlock data in array form, convert it to the FlowCanvas format
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
          
          // If it is a single ContentBlock object
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
          
          // Return an empty structure by default
          console.warn('Unrecognized ContentBlock data format:', rawValue)
          return { nodes: [], connections: [] }
        }
        
        return (
          <BlocksEditor
            value={normalizeBlocksValue(value)}
            onChange={(newValue) => {
              // Convert the FlowCanvas format back to ContentBlock format for saving
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
            placeholder={placeholder || "Enter valid JSON"}
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
        // Custom field type; the component can be specified via fieldConfig.component
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
          // WASM plugin component
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
          // Other custom components
          return (
            <div className="p-4 border border-dashed border-gray-300 rounded-md text-center text-gray-500">
              Custom component: {fieldConfig.component}
              <br />
              <small>Make sure the component is registered correctly</small>
            </div>
          )
        }
        
        return (
          <Input
            value={value || ''}
            onChange={(e) => onChange(e.target.value)}
            placeholder={placeholder || 'Custom field'}
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

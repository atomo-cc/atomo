/**
 * Dynamic Form - 动态表单引擎
 * 
 * 根据 Schema 元数据自动生成表单，支持：
 * - 各种字段类型的自动渲染
 * - 表单验证
 * - 关联数据选择
 * - 富文本编辑
 */

import { useEffect } from 'react'
import { useForm } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'


import { SchemaMetadata, ModelMetadata, EntityData } from '../../lib/types'
import { Button } from '../ui/Button'
import { FormField } from './FormField'
import { generateValidationSchema } from '../../lib/validation'

interface DynamicFormProps {
  modelName: string
  modelMetadata: ModelMetadata
  schema: SchemaMetadata
  initialData?: EntityData
  onSubmit: (data: any) => void | Promise<void>
  mode: 'view' | 'edit' | 'create'
  loading?: boolean
}

export function DynamicForm({
  modelName: _modelName,
  modelMetadata,
  schema,
  initialData,
  onSubmit,
  mode,
  loading = false
}: DynamicFormProps) {
  const isReadonly = mode === 'view'
  const isCreate = mode === 'create'
  
  // 清理初始数据，将null值转换为空字符串
  const cleanInitialData = (data: any) => {
    if (!data) return {}
    
    const cleaned: any = {}
    Object.entries(data).forEach(([key, value]) => {
      if (value === null) {
        // 根据字段类型处理null值
        const field = modelMetadata.fields[key]
        if (field) {
          switch (field.type) {
            case 'string':
            case 'text':
            case 'email':
            case 'url':
              cleaned[key] = ''
              break
            case 'array':
              cleaned[key] = []
              break
            case 'json':
              cleaned[key] = null // JSON字段保持null
              break
            default:
              cleaned[key] = null
          }
        } else {
          cleaned[key] = null
        }
      } else {
        cleaned[key] = value
      }
    })
    
    return cleaned
  }

  // 生成验证 schema
  const validationSchema = generateValidationSchema(modelMetadata)
  
  // 表单设置
  const form = useForm({
    resolver: zodResolver(validationSchema),
    defaultValues: cleanInitialData(initialData),
    mode: 'onChange',
    reValidateMode: 'onChange',
    shouldFocusError: true
  })

  // 当初始数据改变时重置表单
  useEffect(() => {
    if (initialData) {
      form.reset(cleanInitialData(initialData))
    }
  }, [initialData, form])

  // 强制表单重新验证（解决isValid状态同步问题）
  useEffect(() => {
    // 在组件挂载后立即触发验证
    const timer = setTimeout(() => {
      form.trigger()
    }, 100)
    
    return () => clearTimeout(timer)
  }, [form])

  // 表单字段配置（来自 schema 的 editForm 或 detailView）
  const formFields = isReadonly 
    ? (modelMetadata.ui.detailView || modelMetadata.ui.editForm)
    : modelMetadata.ui.editForm

  return (
    <form onSubmit={form.handleSubmit(onSubmit)} className="space-y-6">
      {/* 表单字段 */}
      <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
        {formFields.map((fieldName) => {
          const field = modelMetadata.fields[fieldName]
          if (!field) return null

          return (
            <FormField
              key={fieldName}
              field={field}
              value={form.watch(fieldName)}
              onChange={(value) => form.setValue(fieldName, value)}
              error={form.formState.errors[fieldName]?.message as string}
              disabled={isReadonly}
              modelMetadata={modelMetadata}
              schema={schema}
            />
          )
        })}
      </div>

      {/* 提交按钮 */}
      {!isReadonly && (
        <div className="flex justify-end gap-3 pt-6 border-t border-gray-200">
          <Button
            type="button"
            variant="secondary"
            onClick={() => form.reset(cleanInitialData(initialData))}
            disabled={loading}
          >
            重置
          </Button>
          
          <Button
            type="submit"
            loading={loading}
            disabled={isCreate ? loading : (!form.formState.isValid || loading)}
          >
            {isCreate ? '创建' : '保存'}
          </Button>
        </div>
      )}

      {/* 调试信息（开发模式） */}
      {(import.meta as any).env?.DEV && (
        <details className="mt-8 p-4 bg-gray-50 rounded-md">
          <summary className="cursor-pointer text-sm font-medium text-gray-700">
            调试信息
          </summary>
          <div className="mt-2 space-y-2">
            <div className="text-xs">
              <span className="font-medium">表单状态: </span>
              <span className={`px-2 py-1 rounded text-white text-xs ${
                form.formState.isValid ? 'bg-green-600' : 'bg-red-600'
              }`}>
                {form.formState.isValid ? '有效' : '无效'}
              </span>
              <span className="ml-2 text-gray-500">
                (已提交: {form.formState.isSubmitted ? '是' : '否'}, 
                 已修改: {form.formState.isDirty ? '是' : '否'})
              </span>
            </div>
            
            {Object.keys(form.formState.errors).length > 0 && (
              <div className="text-xs">
                <span className="font-medium text-red-600">错误字段: </span>
                {Object.keys(form.formState.errors).join(', ')}
              </div>
            )}
            
            <details className="text-xs">
              <summary className="cursor-pointer text-gray-700">完整调试数据</summary>
              <pre className="mt-1 text-xs text-gray-600 overflow-auto bg-gray-100 p-2 rounded">
                {JSON.stringify({
                  formData: form.watch(),
                  errors: form.formState.errors,
                  formState: {
                    isValid: form.formState.isValid,
                    isDirty: form.formState.isDirty,
                    isSubmitted: form.formState.isSubmitted,
                    isSubmitting: form.formState.isSubmitting,
                    touchedFields: form.formState.touchedFields,
                    dirtyFields: form.formState.dirtyFields
                  }
                }, null, 2)}
              </pre>
            </details>
          </div>
        </details>
      )}
    </form>
  )
}

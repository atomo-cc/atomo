/**
 * Dynamic Form - 动态表单引擎
 * 
 * 根据 Schema 元数据自动生成表单，支持：
 * - 各种字段类型的自动渲染
 * - 表单验证
 * - 关联数据选择
 * - 富文本编辑
 */

import React, { useEffect } from 'react'
import { useForm } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { z } from 'zod'

import { SchemaMetadata, ModelMetadata, EntityData, FieldMetadata } from '../../lib/types'
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
  modelName,
  modelMetadata,
  schema,
  initialData,
  onSubmit,
  mode,
  loading = false
}: DynamicFormProps) {
  const isReadonly = mode === 'view'
  const isCreate = mode === 'create'
  
  // 生成验证 schema
  const validationSchema = generateValidationSchema(modelMetadata)
  
  // 表单设置
  const form = useForm({
    resolver: zodResolver(validationSchema),
    defaultValues: initialData || {},
    mode: 'onChange'
  })

  // 当初始数据改变时重置表单
  useEffect(() => {
    if (initialData) {
      form.reset(initialData)
    }
  }, [initialData, form])

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
            onClick={() => form.reset()}
            disabled={loading}
          >
            重置
          </Button>
          
          <Button
            type="submit"
            loading={loading}
            disabled={!form.formState.isValid}
          >
            {isCreate ? '创建' : '保存'}
          </Button>
        </div>
      )}

      {/* 调试信息（开发模式） */}
      {process.env.NODE_ENV === 'development' && (
        <details className="mt-8 p-4 bg-gray-50 rounded-md">
          <summary className="cursor-pointer text-sm font-medium text-gray-700">
            调试信息
          </summary>
          <pre className="mt-2 text-xs text-gray-600 overflow-auto">
            {JSON.stringify({
              formData: form.watch(),
              errors: form.formState.errors,
              isValid: form.formState.isValid,
            }, null, 2)}
          </pre>
        </details>
      )}
    </form>
  )
}

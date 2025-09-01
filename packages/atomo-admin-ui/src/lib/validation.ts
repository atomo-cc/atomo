/**
 * Validation Schema Generator
 * 
 * 根据 ModelMetadata 生成 Zod 验证 schema
 */

import { z } from 'zod'
import { ModelMetadata, FieldMetadata } from './types'

export function generateValidationSchema(modelMetadata: ModelMetadata): z.ZodSchema {
  const shape: Record<string, z.ZodType> = {}

  // 遍历所有字段生成验证规则
  Object.entries(modelMetadata.fields).forEach(([fieldName, field]) => {
    let fieldSchema = generateFieldSchema(field)
    
    // 处理可选字段
    if (field.optional) {
      fieldSchema = fieldSchema.optional()
    }
    
    shape[fieldName] = fieldSchema
  })

  return z.object(shape)
}

function generateFieldSchema(field: FieldMetadata): z.ZodType {
  switch (field.type) {
    case 'string':
    case 'text':
    case 'email':
    case 'url':
      let stringSchema = z.string()
      
      // 应用验证规则
      if (field.ui?.validation) {
        const validation = field.ui.validation
        
        if (validation.minLength) {
          stringSchema = stringSchema.min(validation.minLength)
        }
        if (validation.maxLength) {
          stringSchema = stringSchema.max(validation.maxLength)
        }
        if (validation.pattern) {
          stringSchema = stringSchema.regex(new RegExp(validation.pattern))
        }
      }
      
      // 特殊字段类型验证
      if (field.name.toLowerCase().includes('email')) {
        stringSchema = stringSchema.email('请输入有效的邮箱地址')
      }
      if (field.name.toLowerCase().includes('url')) {
        stringSchema = stringSchema.url('请输入有效的URL')
      }
      
      return stringSchema

    case 'number':
      let numberSchema = z.number()
      
      if (field.ui?.validation) {
        const validation = field.ui.validation
        
        if (validation.min !== undefined) {
          numberSchema = numberSchema.min(validation.min)
        }
        if (validation.max !== undefined) {
          numberSchema = numberSchema.max(validation.max)
        }
      }
      
      return numberSchema

    case 'boolean':
      return z.boolean()

    case 'date':
    case 'datetime':
      return z.date().or(z.string().transform((str) => new Date(str)))

    case 'reference':
      return z.string().uuid('请选择有效的关联项目')

    case 'array':
      return z.array(z.any()).default([])

    case 'blocks':
      return z.array(z.object({
        type: z.string(),
        data: z.any()
      })).default([])

    case 'json':
      return z.any()

    case 'custom':
      return z.any()

    default:
      return z.string()
  }
}

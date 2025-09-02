/**
 * Validation Schema Generator
 * 
 * 根据模型元数据生成 Zod 验证模式
 */

import { z } from 'zod'
import { ModelMetadata, FieldMetadata } from './types'

export function generateValidationSchema(modelMetadata: ModelMetadata): z.ZodSchema {
  const schemaFields: Record<string, z.ZodTypeAny> = {}

  Object.entries(modelMetadata.fields).forEach(([fieldName, field]) => {
    schemaFields[fieldName] = generateFieldSchema(field)
  })

  return z.object(schemaFields)
}

function generateFieldSchema(field: FieldMetadata): z.ZodTypeAny {
  let schema: z.ZodTypeAny

  // 根据字段类型创建基础 schema
  switch (field.type) {
    case 'string':
    case 'text':
    case 'email':
    case 'url':
      schema = z.string()
      
      // 邮箱验证
      if (field.name.toLowerCase().includes('email') || field.type === 'email') {
        schema = z.string().email('请输入有效的邮箱地址')
      }
      
      // URL验证
      if (field.name.toLowerCase().includes('url') || field.type === 'url') {
        schema = z.string().url('请输入有效的URL')
      }
      
      // 长度验证
      if (field.ui?.validation?.minLength) {
        schema = (schema as z.ZodString).min(field.ui.validation.minLength, 
          `最少需要 ${field.ui.validation.minLength} 个字符`)
      }
      
      if (field.ui?.validation?.maxLength) {
        schema = (schema as z.ZodString).max(field.ui.validation.maxLength, 
          `最多允许 ${field.ui.validation.maxLength} 个字符`)
      }
      
      // 模式验证
      if (field.ui?.validation?.pattern) {
        const regex = new RegExp(field.ui.validation.pattern)
        schema = (schema as z.ZodString).regex(regex, '格式不正确')
      }
      
      break

    case 'number':
      schema = z.number()
      
      if (field.ui?.validation?.min !== undefined) {
        schema = (schema as z.ZodNumber).min(field.ui.validation.min, 
          `最小值为 ${field.ui.validation.min}`)
      }
      
      if (field.ui?.validation?.max !== undefined) {
        schema = (schema as z.ZodNumber).max(field.ui.validation.max, 
          `最大值为 ${field.ui.validation.max}`)
      }
      
      break

    case 'boolean':
      schema = z.boolean()
      break

    case 'date':
    case 'datetime':
      schema = z.union([z.string(), z.date()]).transform((val) => {
        if (typeof val === 'string') {
          const date = new Date(val)
          if (isNaN(date.getTime())) {
            throw new Error('无效的日期格式')
          }
          return date
        }
        return val
      })
      break

    case 'array':
      schema = z.array(z.unknown())
      
      if (field.ui?.validation?.minItems) {
        schema = (schema as z.ZodArray<any>).min(field.ui.validation.minItems, 
          `至少需要 ${field.ui.validation.minItems} 个项目`)
      }
      
      if (field.ui?.validation?.maxItems) {
        schema = (schema as z.ZodArray<any>).max(field.ui.validation.maxItems, 
          `最多允许 ${field.ui.validation.maxItems} 个项目`)
      }
      
      break

    case 'reference':
      schema = z.string()
      break

    case 'json':
    case 'blocks':
      schema = z.unknown()
      break

    case 'custom':
    default:
      schema = z.unknown()
      break
  }

  // 处理可选字段
  if (field.optional) {
    schema = schema.optional()
  }

  return schema
}

/**
 * 客户端验证
 */
export function validateField(value: any, field: FieldMetadata): string | null {
  try {
    const schema = generateFieldSchema(field)
    schema.parse(value)
    return null
  } catch (error) {
    if (error instanceof z.ZodError) {
      return error.errors[0]?.message || '验证失败'
    }
    return '验证失败'
  }
}

/**
 * 验证整个表单
 */
export function validateForm(data: Record<string, any>, modelMetadata: ModelMetadata): Record<string, string> {
  const errors: Record<string, string> = {}
  
  Object.entries(modelMetadata.fields).forEach(([fieldName, field]) => {
    const error = validateField(data[fieldName], field)
    if (error) {
      errors[fieldName] = error
    }
  })

  return errors
}
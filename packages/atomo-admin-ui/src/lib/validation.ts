/**
 * Validation Schema Generator
 *
 * Generate a Zod validation schema from model metadata.
 */

import { z } from 'zod'
import { ModelMetadata, FieldMetadata } from './types'

export function generateValidationSchema(modelMetadata: ModelMetadata): z.ZodSchema {
  const schemaFields: Record<string, z.ZodTypeAny> = {}

  Object.entries(modelMetadata.fields).forEach(([fieldName, field]) => {
    // Skip system fields; they shouldn't be part of form validation
    if (['id', 'createdAt', 'updatedAt', 'created_at', 'updated_at'].includes(fieldName)) {
      return
    }
    
    schemaFields[fieldName] = generateFieldSchema(field)
  })

  return z.object(schemaFields)
}

function generateFieldSchema(field: FieldMetadata): z.ZodTypeAny {
  let schema: z.ZodTypeAny

  // Create the base schema based on the field type
  switch (field.type) {
    case 'string':
    case 'text':
    case 'email':
    case 'url':
      // Create a string schema that can handle null values
      schema = z.union([z.string(), z.null()]).transform((val) => val === null ? '' : val)

      // Email validation
      if (field.name.toLowerCase().includes('email') || field.type === 'email') {
        schema = z.union([z.string().email('Please enter a valid email address'), z.null()]).transform((val) => val === null ? '' : val)
      }

      // URL validation
      if (field.name.toLowerCase().includes('url') || field.type === 'url') {
        schema = z.union([z.string().url('Please enter a valid URL'), z.null()]).transform((val) => val === null ? '' : val)
      }

      // For an already-transformed schema, check for an empty string first
      const stringSchema = schema as z.ZodEffects<any, string, any>

      // Length validation
      if (field.ui?.validation?.minLength) {
        schema = stringSchema.refine((val) => !val || val.length >= field.ui!.validation!.minLength!,
          `Must be at least ${field.ui!.validation!.minLength} characters`)
      }

      if (field.ui?.validation?.maxLength) {
        schema = stringSchema.refine((val) => !val || val.length <= field.ui!.validation!.maxLength!,
          `Must be at most ${field.ui!.validation!.maxLength} characters`)
      }

      // Pattern validation
      if (field.ui?.validation?.pattern) {
        const regex = new RegExp(field.ui.validation.pattern)
        schema = stringSchema.refine((val) => !val || regex.test(val), 'Invalid format')
      }
      
      break

    case 'number':
      schema = z.number()
      
      if (field.ui?.validation?.min !== undefined) {
        schema = (schema as z.ZodNumber).min(field.ui.validation.min,
          `Minimum value is ${field.ui.validation.min}`)
      }
      
      if (field.ui?.validation?.max !== undefined) {
        schema = (schema as z.ZodNumber).max(field.ui.validation.max,
          `Maximum value is ${field.ui.validation.max}`)
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
            throw new Error('Invalid date format')
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
          `At least ${field.ui.validation.minItems} items are required`)
      }
      
      if (field.ui?.validation?.maxItems) {
        schema = (schema as z.ZodArray<any>).max(field.ui.validation.maxItems,
          `At most ${field.ui.validation.maxItems} items are allowed`)
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

  // Handle optional fields and required-field validation
  if (field.optional) {
    // Optional fields allow empty values
    schema = schema.optional()
  } else {
    // Special handling for required fields
    switch (field.type) {
      case 'json':
      case 'array':
        // JSON and array fields can be empty even when required (an empty array or object)
        schema = schema.optional()
        break
      case 'string':
      case 'text':
      case 'email':
      case 'url':
        // String fields: allow certain special fields to be empty
        if (field.name === 'notes' || field.name.includes('description') ||
            field.name === 'phone' || field.name === 'website' ||
            field.name === 'address' || field.name === 'industry') {
          schema = schema.optional()
        } else {
          // Required string fields: cannot be an empty string
          schema = (schema as z.ZodEffects<any, string, any>).refine(
            (val) => val && val.trim().length > 0,
            'This field is required'
          )
        }
        break
      default:
        // Required fields of other types are left as-is
        break
    }
  }

  return schema
}

/**
 * Client-side validation.
 */
export function validateField(value: any, field: FieldMetadata): string | null {
  try {
    const schema = generateFieldSchema(field)
    schema.parse(value)
    return null
  } catch (error) {
    if (error instanceof z.ZodError) {
      return error.errors[0]?.message || 'Validation failed'
    }
    return 'Validation failed'
  }
}

/**
 * Validate the entire form.
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
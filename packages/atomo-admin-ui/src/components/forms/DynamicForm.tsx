/**
 * Dynamic Form - dynamic form engine
 *
 * Automatically generates a form from schema metadata. Supports:
 * - Automatic rendering of various field types
 * - Form validation
 * - Related-data selection
 * - Rich-text editing
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
  
  // Clean up the initial data, converting null values to empty strings
  const cleanInitialData = (data: any) => {
    if (!data) return {}

    const cleaned: any = {}
    Object.entries(data).forEach(([key, value]) => {
      if (value === null) {
        // Handle null values based on the field type
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
              cleaned[key] = null // Keep JSON fields as null
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

  // Generate the validation schema
  const validationSchema = generateValidationSchema(modelMetadata)

  // Form setup
  const form = useForm({
    resolver: zodResolver(validationSchema),
    defaultValues: cleanInitialData(initialData),
    mode: 'onChange',
    reValidateMode: 'onChange',
    shouldFocusError: true
  })

  // Reset the form when the initial data changes
  useEffect(() => {
    if (initialData) {
      form.reset(cleanInitialData(initialData))
    }
  }, [initialData, form])

  // Force the form to re-validate (fixes an isValid state sync issue)
  useEffect(() => {
    // Trigger validation immediately after the component mounts
    const timer = setTimeout(() => {
      form.trigger()
    }, 100)

    return () => clearTimeout(timer)
  }, [form])

  // Form field configuration (from the schema's editForm or detailView)
  const formFields = isReadonly 
    ? (modelMetadata.ui.detailView || modelMetadata.ui.editForm)
    : modelMetadata.ui.editForm

  return (
    <form onSubmit={form.handleSubmit(onSubmit)} className="space-y-6">
      {/* Form fields */}
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

      {/* Submit buttons */}
      {!isReadonly && (
        <div className="flex justify-end gap-3 pt-6 border-t border-gray-200">
          <Button
            type="button"
            variant="secondary"
            onClick={() => form.reset(cleanInitialData(initialData))}
            disabled={loading}
          >
            Reset
          </Button>
          
          <Button
            type="submit"
            loading={loading}
            disabled={isCreate ? loading : (!form.formState.isValid || loading)}
          >
            {isCreate ? 'Create' : 'Save'}
          </Button>
        </div>
      )}

      {/* Debug info (development mode) */}
      {(import.meta as any).env?.DEV && (
        <details className="mt-8 p-4 bg-gray-50 rounded-md">
          <summary className="cursor-pointer text-sm font-medium text-gray-700">
            Debug info
          </summary>
          <div className="mt-2 space-y-2">
            <div className="text-xs">
              <span className="font-medium">Form state: </span>
              <span className={`px-2 py-1 rounded text-white text-xs ${
                form.formState.isValid ? 'bg-green-600' : 'bg-red-600'
              }`}>
                {form.formState.isValid ? 'Valid' : 'Invalid'}
              </span>
              <span className="ml-2 text-gray-500">
                (Submitted: {form.formState.isSubmitted ? 'Yes' : 'No'},
                 Modified: {form.formState.isDirty ? 'Yes' : 'No'})
              </span>
            </div>
            
            {Object.keys(form.formState.errors).length > 0 && (
              <div className="text-xs">
                <span className="font-medium text-red-600">Fields with errors: </span>
                {Object.keys(form.formState.errors).join(', ')}
              </div>
            )}
            
            <details className="text-xs">
              <summary className="cursor-pointer text-gray-700">Full debug data</summary>
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

/**
 * Entity Detail View
 *
 * Supports viewing, editing, and creating records with dynamically generated forms.
 */

import React, { useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { useNavigate } from 'react-router-dom'
import { useForm } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { z } from 'zod'
import { 
  ArrowLeft, 
  Save, 
  Edit2, 
  Trash2, 
  Eye,
  Clock,
  User
} from 'lucide-react'

import { SchemaMetadata, ModelMetadata, EntityData } from '../../lib/types'
import { apiClient } from '../../lib/api'
import { Card, CardHeader, CardTitle, CardContent } from '../ui/Card'
import { Button } from '../ui/Button'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '../ui/Tabs'
import { toast } from '../ui/Toast'
import { DynamicForm } from '../forms/DynamicForm'
import { formatDate, getFieldLabel } from '../../lib/utils'
import { canPerform } from '../../lib/permissions'

interface EntityDetailViewProps {
  modelName: string
  entityId?: string // undefined indicates create mode
  modelMetadata: ModelMetadata
  schema: SchemaMetadata
  mode: 'detail' | 'edit' | 'create'
}

export function EntityDetailView({ 
  modelName, 
  entityId, 
  modelMetadata, 
  schema, 
  mode: initialMode 
}: EntityDetailViewProps) {
  const navigate = useNavigate()
  const queryClient = useQueryClient()
  const [mode, setMode] = useState(initialMode)
  
  // Fetch entity data (only when not in create mode)
  const {
    data: entity, 
    isLoading, 
    error 
  } = useQuery({
    queryKey: ['entity', modelName, entityId],
    queryFn: () => apiClient.getEntity(modelName, entityId!),
    enabled: !!entityId && mode !== 'create',
  })

  // Update entity
  const updateMutation = useMutation({
    mutationFn: (data: Partial<EntityData>) => 
      apiClient.updateEntity(modelName, entityId!, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['entity', modelName, entityId] })
      queryClient.invalidateQueries({ queryKey: ['entities', modelName] })
      setMode('detail')
    },
  })

  // Create entity
  const createMutation = useMutation({
    mutationFn: (data: Partial<EntityData>) => 
      apiClient.createEntity(modelName, data),
    onSuccess: (newEntity) => {
      queryClient.invalidateQueries({ queryKey: ['entities', modelName] })
      navigate(`/entities/${modelName}`)
    },
  })

  // Delete entity
  const deleteMutation = useMutation({
    mutationFn: () => apiClient.deleteEntity(modelName, entityId!),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['entities', modelName] })
      navigate(`/entities/${modelName}`)
    },
  })

  // Cosmetic role gating — the server enforces access regardless.
  const role = apiClient.currentUser?.role
  const mayUpdate = canPerform(modelMetadata, 'update', role)
  const mayDelete = canPerform(modelMetadata, 'delete', role)

  // Form submission
  const handleSave = async (formData: any) => {
    try {
      if (mode === 'create') {
        await createMutation.mutateAsync(formData)
        toast.success('Created')
      } else {
        await updateMutation.mutateAsync(formData)
        toast.success('Saved')
      }
    } catch (error) {
      console.error('Save failed:', error)
      toast.error('Save failed, please try again')
    }
  }

  // Delete confirmation
  const handleDelete = () => {
    if (confirm(`Are you sure you want to delete this ${getFieldLabel(modelName)}?`)) {
      deleteMutation.mutate()
    }
  }

  // Error state
  if (error) {
    return (
      <Card className="m-6">
        <CardContent className="py-8 text-center">
          <h3 className="text-lg font-semibold text-gray-900 mb-2">Failed to Load</h3>
          <p className="text-gray-600 mb-4">Unable to load {getFieldLabel(modelName)} data</p>
          <Button onClick={() => navigate(`/entities/${modelName}`)}>
            Back to List
          </Button>
        </CardContent>
      </Card>
    )
  }

  const isCreate = mode === 'create'
  const isEdit = mode === 'edit'
  const isDetail = mode === 'detail'

  return (
    <div className="p-6 space-y-6">
      {/* Page header and actions */}
      <div className="flex justify-between items-center">
        <div className="flex items-center gap-4">
          <Button
            variant="ghost"
            size="sm"
            onClick={() => navigate(`/entities/${modelName}`)}
          >
            <ArrowLeft className="h-4 w-4 mr-2" />
            Back to List
          </Button>

          <div>
            <h1 className="text-3xl font-bold text-gray-900">
              {isCreate ? `New ${getFieldLabel(modelName)}` :
               isEdit ? `Edit ${getFieldLabel(modelName)}` :
               getFieldLabel(modelName)}
            </h1>
            {entity && (
              <p className="text-gray-600 mt-1">
                ID: {entity.id}
              </p>
            )}
          </div>
        </div>
        
        <div className="flex gap-3">
          {modelName === 'Contact' && entity && isDetail && (
            <Button
              variant="secondary"
              onClick={() => navigate(`/contacts/${entity.id}/timeline`)}
            >
              View Timeline
            </Button>
          )}
          {isDetail && (
            <>
              {mayUpdate && (
                <Button
                  variant="secondary"
                  onClick={() => setMode('edit')}
                >
                  <Edit2 className="h-4 w-4 mr-2" />
                  Edit
                </Button>
              )}

              {mayDelete && (
                <Button
                  variant="danger"
                  onClick={handleDelete}
                  disabled={deleteMutation.isPending}
                >
                  <Trash2 className="h-4 w-4 mr-2" />
                  Delete
                </Button>
              )}
            </>
          )}
          
          {isEdit && (
            <Button
              variant="secondary"
              onClick={() => setMode('detail')}
            >
              <Eye className="h-4 w-4 mr-2" />
              View
            </Button>
          )}
        </div>
      </div>

      {/* Main content */}
      {isLoading ? (
        <Card>
          <CardContent className="py-8 text-center">
            <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-primary-600 mx-auto"></div>
            <p className="mt-4 text-gray-600">Loading...</p>
          </CardContent>
        </Card>
      ) : (
        <Tabs defaultValue="details" className="space-y-6">
          <TabsList>
            <TabsTrigger value="details">Details</TabsTrigger>
            {entity && (
              <>
                <TabsTrigger value="history">History</TabsTrigger>
                <TabsTrigger value="relations">Related Data</TabsTrigger>
              </>
            )}
          </TabsList>

          {/* Details tab */}
          <TabsContent value="details">
            <Card>
              <CardContent className="p-6">
                <DynamicForm
                  modelName={modelName}
                  modelMetadata={modelMetadata}
                  schema={schema}
                  initialData={entity}
                  onSubmit={handleSave}
                  mode={isDetail ? 'view' : isEdit ? 'edit' : 'create'}
                  loading={updateMutation.isPending || createMutation.isPending}
                />
              </CardContent>
            </Card>
          </TabsContent>

          {/* History tab */}
          {entity && (
            <TabsContent value="history">
              <Card>
                <CardHeader>
                  <CardTitle className="flex items-center gap-2">
                    <Clock className="h-5 w-5" />
                    Change History
                  </CardTitle>
                </CardHeader>
                <CardContent>
                  <div className="space-y-4">
                    <div className="flex items-center gap-3 p-3 bg-gray-50 rounded-md">
                      <User className="h-4 w-4 text-gray-500" />
                      <div className="flex-1">
                        <div className="text-sm font-medium">Record Created</div>
                        <div className="text-xs text-gray-500">
                          {formatDate(entity.createdAt, 'time')}
                        </div>
                      </div>
                    </div>
                    
                    {entity.updatedAt !== entity.createdAt && (
                      <div className="flex items-center gap-3 p-3 bg-gray-50 rounded-md">
                        <Edit2 className="h-4 w-4 text-gray-500" />
                        <div className="flex-1">
                          <div className="text-sm font-medium">Last Updated</div>
                          <div className="text-xs text-gray-500">
                            {formatDate(entity.updatedAt, 'time')}
                          </div>
                        </div>
                      </div>
                    )}
                  </div>
                </CardContent>
              </Card>
            </TabsContent>
          )}

          {/* Related data tab */}
          {entity && (
            <TabsContent value="relations">
              <Card>
                <CardHeader>
                  <CardTitle>Related Data</CardTitle>
                </CardHeader>
                <CardContent>
                  <p className="text-gray-600">Related data is under development...</p>
                </CardContent>
              </Card>
            </TabsContent>
          )}
        </Tabs>
      )}
    </div>
  )
}

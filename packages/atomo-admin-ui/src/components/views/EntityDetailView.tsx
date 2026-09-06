/**
 * Entity Detail View — Dashin Record Inspection, Creation, and Editing
 */

import React, { useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { useNavigate } from 'react-router-dom'
import { 
  ArrowLeft, 
  Save, 
  Edit2, 
  Trash2, 
  Eye,
  Clock,
  User,
  AlertCircle
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
    onSuccess: () => {
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

  // Role-based permissions
  const role = apiClient.currentUser?.role
  const mayUpdate = canPerform(modelMetadata, 'update', role)
  const mayDelete = canPerform(modelMetadata, 'delete', role)

  // Form submission
  const handleSave = async (formData: any) => {
    try {
      if (mode === 'create') {
        await createMutation.mutateAsync(formData)
        toast.success('Record created successfully')
      } else {
        await updateMutation.mutateAsync(formData)
        toast.success('Record updated successfully')
      }
    } catch (err: any) {
      console.error('Save failed:', err)
      toast.error(err?.message || 'Save failed, please check your input')
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
      <div className="p-6">
        <Card>
          <CardContent className="py-12 text-center">
            <AlertCircle className="h-8 w-8 text-rose-500 mx-auto mb-3" />
            <h3 className="text-base font-semibold text-foreground mb-1">Failed to Load Record</h3>
            <p className="text-xs text-icon-muted mb-4">Unable to retrieve {getFieldLabel(modelName)} details from server.</p>
            <Button size="sm" variant="secondary" onClick={() => navigate(`/entities/${modelName}`)}>
              Back to Table
            </Button>
          </CardContent>
        </Card>
      </div>
    )
  }

  const isCreate = mode === 'create'
  const isEdit = mode === 'edit'
  const isDetail = mode === 'detail'

  return (
    <div className="p-6 space-y-6 max-w-5xl">
      {/* Page header and actions */}
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
        <div className="flex items-center gap-3">
          <Button
            variant="ghost"
            size="sm"
            onClick={() => navigate(`/entities/${modelName}`)}
            className="h-8 w-8 p-0"
            title="Back to list"
          >
            <ArrowLeft className="h-4 w-4" />
          </Button>

          <div>
            <h1 className="text-2xl font-bold tracking-tight text-foreground">
              {isCreate ? `New ${getFieldLabel(modelName)}` :
               isEdit ? `Edit ${getFieldLabel(modelName)}` :
               getFieldLabel(modelName)}
            </h1>
            {entity && (
              <p className="font-mono text-xs text-icon-muted mt-0.5">
                ID: {entity.id}
              </p>
            )}
          </div>
        </div>
        
        <div className="flex items-center gap-2">
          {modelName === 'Contact' && entity && isDetail && (
            <Button
              variant="secondary"
              size="sm"
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
                  size="sm"
                  onClick={() => setMode('edit')}
                >
                  <Edit2 className="h-3.5 w-3.5 mr-1.5" />
                  Edit
                </Button>
              )}

              {mayDelete && (
                <Button
                  variant="danger"
                  size="sm"
                  onClick={handleDelete}
                  disabled={deleteMutation.isPending}
                >
                  <Trash2 className="h-3.5 w-3.5 mr-1.5" />
                  Delete
                </Button>
              )}
            </>
          )}
          
          {isEdit && (
            <Button
              variant="secondary"
              size="sm"
              onClick={() => setMode('detail')}
            >
              <Eye className="h-3.5 w-3.5 mr-1.5" />
              View Mode
            </Button>
          )}
        </div>
      </div>

      {/* Main content */}
      {isLoading ? (
        <Card>
          <CardContent className="py-12 text-center">
            <div className="w-8 h-8 rounded-full border-2 border-primary border-t-transparent animate-spin mx-auto mb-3" />
            <p className="text-xs text-icon-muted">Loading record data…</p>
          </CardContent>
        </Card>
      ) : (
        <Tabs defaultValue="details" className="space-y-4">
          <TabsList>
            <TabsTrigger value="details">Details & Fields</TabsTrigger>
            {entity && (
              <>
                <TabsTrigger value="history">Audit History</TabsTrigger>
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
                <CardHeader className="py-4 border-b border-bn-border/60">
                  <CardTitle className="flex items-center gap-2">
                    <Clock className="h-4 w-4 text-primary" />
                    Lifecycle & History
                  </CardTitle>
                </CardHeader>
                <CardContent className="p-5 space-y-3">
                  <div className="flex items-center gap-3 p-3 bg-content-bg rounded-bn border border-bn-border">
                    <div className="w-7 h-7 rounded-full bg-emerald-500/10 flex items-center justify-center text-emerald-600">
                      <User className="h-3.5 w-3.5" />
                    </div>
                    <div className="flex-1">
                      <div className="text-xs font-semibold text-foreground">Record Created</div>
                      <div className="text-[11px] text-icon-muted">
                        {formatDate(entity.createdAt, 'time')}
                      </div>
                    </div>
                  </div>
                  
                  {entity.updatedAt && entity.updatedAt !== entity.createdAt && (
                    <div className="flex items-center gap-3 p-3 bg-content-bg rounded-bn border border-bn-border">
                      <div className="w-7 h-7 rounded-full bg-blue-500/10 flex items-center justify-center text-blue-600">
                        <Edit2 className="h-3.5 w-3.5" />
                      </div>
                      <div className="flex-1">
                        <div className="text-xs font-semibold text-foreground">Last Modified</div>
                        <div className="text-[11px] text-icon-muted">
                          {formatDate(entity.updatedAt, 'time')}
                        </div>
                      </div>
                    </div>
                  )}
                </CardContent>
              </Card>
            </TabsContent>
          )}

          {/* Related data tab */}
          {entity && (
            <TabsContent value="relations">
              <Card>
                <CardHeader className="py-4 border-b border-bn-border/60">
                  <CardTitle>Cross-Entity Relations</CardTitle>
                </CardHeader>
                <CardContent className="p-6 text-center text-xs text-icon-muted">
                  Use the RelatedPreview side drawer in the list table to drill into interconnected models.
                </CardContent>
              </Card>
            </TabsContent>
          )}
        </Tabs>
      )}
    </div>
  )
}

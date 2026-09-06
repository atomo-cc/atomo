/**
 * Reference Select - related-data selector
 *
 * Used to select data from a related model. Supports search and pagination.
 */

import { useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { Search, X } from 'lucide-react'

import { RelationshipConfig, SchemaMetadata, EntityData } from '../../lib/types'
import { apiClient } from '../../lib/api'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '../ui/Select'
import { Input } from '../ui/Input'
import { Spinner } from '../ui/Spinner'
import { cn, getFieldLabel } from '../../lib/utils'

interface ReferenceSelectProps {
  value?: string
  onChange: (value: string | undefined) => void
  relationship: RelationshipConfig
  schema: SchemaMetadata
  disabled?: boolean
  error?: string
  placeholder?: string
}

export function ReferenceSelect({
  value,
  onChange,
  relationship,
  schema,
  disabled = false,
  error,
  placeholder
}: ReferenceSelectProps) {
  const [search, setSearch] = useState('')
  const [isOpen, setIsOpen] = useState(false)
  
  const relatedModel = schema.models[relationship.model]
  if (!relatedModel) {
    return (
      <div className="p-2 text-xs text-icon-muted border border-bn-border rounded-bn bg-content-bg">
        Related model {relationship.model} does not exist
      </div>
    )
  }

  // Fetch the related data
  const { data: options, isLoading } = useQuery({
    queryKey: ['reference-options', relationship.model, search],
    queryFn: () => apiClient.listEntities(relationship.model, {
      search,
      limit: 50
    }),
    enabled: isOpen, // Only load while the dropdown is open
  })

  // Fetch the details of the currently selected item
  const { data: selectedItem } = useQuery({
    queryKey: ['reference-item', relationship.model, value],
    queryFn: () => apiClient.getEntity(relationship.model, value!),
    enabled: !!value,
  })

  const displayField = relatedModel.ui.displayField
  const displayFields = Array.isArray(displayField) ? displayField : [displayField]

  // Format the display text
  const formatDisplayText = (item: EntityData) => {
    return displayFields
      .map(field => item[field])
      .filter(Boolean)
      .join(' - ')
  }

  const handleSelect = (selectedValue: string) => {
    onChange(selectedValue === value ? undefined : selectedValue)
    setIsOpen(false)
  }

  const handleClear = () => {
    onChange(undefined)
    setIsOpen(false)
  }

  return (
    <div className="space-y-1.5">
      <Select
        value={value || ''}
        onValueChange={handleSelect}
        open={isOpen}
        onOpenChange={setIsOpen}
        disabled={disabled}
      >
        <SelectTrigger className={cn(error && 'border-danger')}>
          <SelectValue 
            placeholder={placeholder || `Select ${getFieldLabel(relationship.model)}`}
          >
            {selectedItem ? formatDisplayText(selectedItem) : ''}
          </SelectValue>
        </SelectTrigger>
        
        <SelectContent>
          {/* Search box */}
          <div className="p-2 border-b border-bn-border">
            <div className="relative">
              <Search className="absolute left-2.5 top-1/2 transform -translate-y-1/2 h-3.5 w-3.5 text-icon-muted" />
              <Input
                placeholder="Search related records..."
                value={search}
                onChange={(e) => setSearch(e.target.value)}
                className="pl-8 h-8 text-xs"
              />
            </div>
          </div>

          {/* Clear option */}
          {value && (
            <SelectItem value="" onSelect={handleClear}>
              <div className="flex items-center gap-2 text-icon-muted">
                <X className="h-3.5 w-3.5" />
                Clear selection
              </div>
            </SelectItem>
          )}

          {/* Loading state */}
          {isLoading && (
            <div className="p-4 text-center">
              <Spinner size="sm" />
              <p className="text-xs text-icon-muted mt-2">Loading...</p>
            </div>
          )}

          {/* Option list */}
          {options?.data && options.data.length > 0 && (
            <>
              {options.data.map((item) => (
                <SelectItem key={item.id} value={item.id}>
                  <div className="flex flex-col">
                    <span>{formatDisplayText(item)}</span>
                    <span className="text-[10px] text-icon-muted font-mono">ID: {item.id}</span>
                  </div>
                </SelectItem>
              ))}
            </>
          )}

          {/* Empty state */}
          {options?.data && options.data.length === 0 && (
            <div className="p-4 text-center text-icon-muted text-xs">
              {search ? 'No matches found' : 'No records available'}
            </div>
          )}

          {/* More results hint */}
          {options?.data && options.data.length === 50 && (
            <div className="p-2 text-[11px] text-icon-muted text-center border-t border-bn-border">
              Showing first 50 results; type to search
            </div>
          )}
        </SelectContent>
      </Select>

      {error && (
        <p className="text-xs text-danger font-medium">{error}</p>
      )}
    </div>
  )
}
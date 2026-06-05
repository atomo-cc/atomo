/**
 * Media Uploader - Media file upload component
 *
 * Supports drag-and-drop upload, preview, progress display, and more
 */

import { useState, useRef, useCallback } from 'react'
import { 
  Upload, 
  X, 
  File, 
  Image as ImageIcon, 
  Video, 
  Music,
  FileText,
  RotateCcw
} from 'lucide-react'

import { Button } from '../ui/Button'
import { Card, CardContent } from '../ui/Card'
import { Badge } from '../ui/Badge'
import { cn, formatFileSize } from '../../lib/utils'
import { apiClient } from '../../lib/api'

export interface UploadedFile {
  id: string
  name: string
  size: number
  type: string
  url?: string
  status: 'uploading' | 'success' | 'error'
  progress?: number
  error?: string
}

interface MediaUploaderProps {
  value: UploadedFile[]
  onChange: (files: UploadedFile[]) => void
  accept?: string
  maxFiles?: number
  maxFileSize?: number // bytes
  multiple?: boolean
  disabled?: boolean
  showPreview?: boolean
  uploadEndpoint?: string
}

export function MediaUploader({
  value = [],
  onChange,
  accept = '*/*',
  maxFiles = 10,
  maxFileSize = 10 * 1024 * 1024, // 10MB
  multiple = true,
  disabled = false,
  showPreview = true,
  uploadEndpoint = '/api/upload'
}: MediaUploaderProps) {
  const [dragOver, setDragOver] = useState(false)
  const fileInputRef = useRef<HTMLInputElement>(null)
  // Keep the raw File per item id so retry can actually re-upload (was faked before).
  const filesRef = useRef<Record<string, File>>({})

  // Detect file type
  const getFileType = (file: File) => {
    if (file.type.startsWith('image/')) return 'image'
    if (file.type.startsWith('video/')) return 'video'
    if (file.type.startsWith('audio/')) return 'audio'
    if (file.type.includes('pdf') || file.type.includes('document')) return 'document'
    return 'file'
  }



  // Validate file
  const validateFile = (file: File): string | null => {
    if (file.size > maxFileSize) {
      return `File size exceeds the limit (${formatFileSize(maxFileSize)})`
    }

    if (accept !== '*/*') {
      const acceptedTypes = accept.split(',').map(type => type.trim())
      const isAccepted = acceptedTypes.some(type => {
        if (type.startsWith('.')) {
          return file.name.toLowerCase().endsWith(type.toLowerCase())
        }
        return file.type.match(new RegExp(type.replace('*', '.*')))
      })
      
      if (!isAccepted) {
        return 'File type is not supported'
      }
    }

    return null
  }

  // Upload file
  const uploadFile = async (file: File): Promise<UploadedFile> => {
    const fileId = `file_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`
    filesRef.current[fileId] = file

    const uploadedFile: UploadedFile = {
      id: fileId,
      name: file.name,
      size: file.size,
      type: getFileType(file),
      status: 'uploading',
      progress: 0
    }

    try {
      const { url } = await apiClient.uploadMedia(file, (progress) => {
        onChange(value.map(f => f.id === fileId ? { ...f, progress } : f))
      })
      return { ...uploadedFile, status: 'success', url, progress: 100 }
    } catch (error) {
      throw {
        ...uploadedFile,
        status: 'error',
        error: error instanceof Error ? error.message : 'Upload failed'
      } as UploadedFile
    }
  }

  // Handle file selection
  const handleFiles = useCallback(async (files: FileList) => {
    if (disabled) return

    const fileArray = Array.from(files)
    const remainingSlots = maxFiles - value.length
    const filesToProcess = multiple 
      ? fileArray.slice(0, remainingSlots)
      : fileArray.slice(0, 1)

    for (const file of filesToProcess) {
      const error = validateFile(file)
      if (error) {
        // Add error file
        const errorFile: UploadedFile = {
          id: `error_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`,
          name: file.name,
          size: file.size,
          type: getFileType(file),
          status: 'error',
          error
        }
        onChange([...value, errorFile])
        continue
      }

      // Add to upload queue
      const uploadingFile: UploadedFile = {
        id: `uploading_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`,
        name: file.name,
        size: file.size,
        type: getFileType(file),
        status: 'uploading',
        progress: 0
      }
      
      onChange([...value, uploadingFile])

      try {
        const uploadedFile = await uploadFile(file)
        onChange(value.map(f => f.id === uploadingFile.id ? uploadedFile : f))
      } catch (errorFile) {
        onChange(value.map(f => f.id === uploadingFile.id ? (errorFile as UploadedFile) : f))
      }
    }
  }, [value, onChange, disabled, multiple, maxFiles, maxFileSize, accept, uploadEndpoint])

  // Drag-and-drop handling
  const handleDrop = useCallback((e: React.DragEvent) => {
    e.preventDefault()
    setDragOver(false)
    
    if (disabled) return
    
    const { files } = e.dataTransfer
    if (files.length > 0) {
      handleFiles(files)
    }
  }, [handleFiles, disabled])

  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault()
    if (!disabled) {
      setDragOver(true)
    }
  }, [disabled])

  const handleDragLeave = useCallback((e: React.DragEvent) => {
    e.preventDefault()
    setDragOver(false)
  }, [])

  // Remove file
  const removeFile = (fileId: string) => {
    onChange(value.filter(f => f.id !== fileId))
  }

  // Retry upload
  const retryUpload = async (file: UploadedFile) => {
    const updatedFile = { ...file, status: 'uploading' as const, progress: 0, error: undefined }
    onChange(value.map(f => f.id === file.id ? updatedFile : f))

    const original = filesRef.current[file.id]
    if (!original) {
      onChange(value.map(f => f.id === file.id ? { ...f, status: 'error' as const, error: 'Cannot retry (file is no longer available)' } : f))
      return
    }
    try {
      const { url } = await apiClient.uploadMedia(original)
      onChange(value.map(f => f.id === file.id ? { ...f, status: 'success' as const, progress: 100, url } : f))
    } catch (error) {
      onChange(value.map(f => f.id === file.id ? { ...f, status: 'error' as const, error: error instanceof Error ? error.message : 'Retry failed' } : f))
    }
  }

  const canUploadMore = value.length < maxFiles

  return (
    <div className="space-y-4">
      {/* Upload area */}
      {canUploadMore && !disabled && (
        <div
          data-testid="media-uploader-dropzone"
          className={cn(
            'border-2 border-dashed rounded-lg p-6 text-center transition-colors cursor-pointer',
            dragOver 
              ? 'border-primary-500 bg-primary-50' 
              : 'border-gray-300 hover:border-gray-400',
            disabled && 'opacity-50 cursor-not-allowed'
          )}
          onDrop={handleDrop}
          onDragOver={handleDragOver}
          onDragLeave={handleDragLeave}
          onClick={() => !disabled && fileInputRef.current?.click()}
        >
          <Upload className="h-8 w-8 mx-auto mb-4 text-gray-400" />
          <p className="text-sm text-gray-600 mb-2">
            Drag and drop files here, or click to upload
          </p>
          <p className="text-xs text-gray-500">
            Supports {accept === '*/*' ? 'all file types' : accept},
            up to {formatFileSize(maxFileSize)},
            max {maxFiles} files
          </p>
          
          <input
            ref={fileInputRef}
            type="file"
            accept={accept}
            multiple={multiple}
            onChange={(e) => e.target.files && handleFiles(e.target.files)}
            className="hidden"
          />
        </div>
      )}

      {/* File list */}
      {value.length > 0 && (
        <div className="space-y-2">
          {value.map((file) => (
            <FileItem
              key={file.id}
              file={file}
              onRemove={() => removeFile(file.id)}
              onRetry={() => retryUpload(file)}
              showPreview={showPreview}
            />
          ))}
        </div>
      )}

      {/* Status info */}
      {value.length > 0 && (
        <div className="text-xs text-gray-500">
          {value.length} / {maxFiles} files
          {value.length >= maxFiles && ' (limit reached)'}
        </div>
      )}
    </div>
  )
}

// File item component
interface FileItemProps {
  file: UploadedFile
  onRemove: () => void
  onRetry: () => void
  showPreview: boolean
}

function FileItem({ file, onRemove, onRetry, showPreview }: FileItemProps) {
  // File icon
  const getFileIcon = (fileType: string) => {
    switch (fileType) {
      case 'image': return ImageIcon
      case 'video': return Video
      case 'audio': return Music
      case 'document': return FileText
      default: return File
    }
  }
  
  const Icon = getFileIcon(file.type)
  const isImage = file.type === 'image' && file.url

  return (
    <Card>
      <CardContent className="p-3">
        <div className="flex items-center gap-3">
          {/* File icon / preview */}
          <div className="flex-shrink-0">
            {showPreview && isImage ? (
              <img 
                src={file.url} 
                alt={file.name}
                className="w-10 h-10 object-cover rounded"
              />
            ) : (
              <div className="w-10 h-10 bg-gray-100 rounded flex items-center justify-center">
                <Icon className="h-5 w-5 text-gray-500" />
              </div>
            )}
          </div>

          {/* File info */}
          <div className="flex-1 min-w-0">
            <div className="flex items-center gap-2">
              <p className="text-sm font-medium text-gray-900 truncate">
                {file.name}
              </p>
              <Badge 
                variant={
                  file.status === 'success' ? 'success' :
                  file.status === 'error' ? 'danger' : 'secondary'
                }
                className="text-xs"
              >
                {file.status === 'uploading' ? 'Uploading' :
                 file.status === 'success' ? 'Done' : 'Failed'}
              </Badge>
            </div>
            
            <p className="text-xs text-gray-500">
              {formatFileSize(file.size)}
            </p>

                      {/* Progress bar */}
          {file.status === 'uploading' && file.progress !== undefined && (
            <div className="mt-2">
              <div className="w-full bg-gray-200 rounded-full h-1">
                <div 
                  className="bg-primary-600 h-1 rounded-full transition-all duration-300"
                  style={{ width: `${file.progress}%` }}
                />
              </div>
              <p className="text-xs text-gray-500 mt-1">{file.progress}%</p>
            </div>
          )}

            {/* Error message */}
            {file.status === 'error' && file.error && (
              <p className="text-xs text-danger-600 mt-1">{file.error}</p>
            )}
          </div>

          {/* Action buttons */}
          <div className="flex items-center gap-1">
            {file.status === 'error' && (
              <Button
                variant="ghost"
                size="sm"
                onClick={onRetry}
                title="Retry"
              >
                <RotateCcw className="h-4 w-4" />
              </Button>
            )}
            
            <Button
              variant="ghost"
              size="sm"
              onClick={onRemove}
              title="Remove"
            >
              <X className="h-4 w-4" />
            </Button>
          </div>
        </div>
      </CardContent>
    </Card>
  )
}

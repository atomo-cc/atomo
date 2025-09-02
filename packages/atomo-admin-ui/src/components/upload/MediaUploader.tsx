/**
 * Media Uploader - 高级媒体上传组件
 * 
 * 支持拖拽上传、多文件选择、进度显示和预览功能
 * 完全集成到 Atomo 的表单系统中
 */

import React, { useState, useRef, useCallback, useEffect } from 'react'
import { 
  Upload, 
  X, 
  Image, 
  File, 
  Video, 
  Music,
  FileText,
  Check,
  AlertCircle,
  RotateCcw
} from 'lucide-react'

import { Button } from '../ui/Button'
import { Progress } from '../ui/Progress'
import { Card, CardContent } from '../ui/Card'
import { Badge } from '../ui/Badge'
import { cn, formatFileSize } from '../../lib/utils'

export interface UploadedFile {
  id: string
  file: File
  name: string
  size: number
  type: string
  url?: string
  thumbnailUrl?: string
  uploadProgress: number
  status: 'pending' | 'uploading' | 'completed' | 'error'
  error?: string
}

interface MediaUploaderProps {
  value?: UploadedFile[]
  onChange: (files: UploadedFile[]) => void
  accept?: string
  maxFiles?: number
  maxFileSize?: number // in bytes
  disabled?: boolean
  multiple?: boolean
  showPreview?: boolean
  uploadEndpoint?: string
  className?: string
}

const ACCEPTED_FILE_TYPES = {
  image: ['image/jpeg', 'image/png', 'image/gif', 'image/webp', 'image/svg+xml'],
  video: ['video/mp4', 'video/webm', 'video/ogg'],
  audio: ['audio/mp3', 'audio/wav', 'audio/ogg'],
  document: ['application/pdf', 'text/plain', 'application/msword', 'application/vnd.openxmlformats-officedocument.wordprocessingml.document']
}

const getFileIcon = (type: string) => {
  if (ACCEPTED_FILE_TYPES.image.includes(type)) return Image
  if (ACCEPTED_FILE_TYPES.video.includes(type)) return Video
  if (ACCEPTED_FILE_TYPES.audio.includes(type)) return Music
  if (ACCEPTED_FILE_TYPES.document.includes(type)) return FileText
  return File
}

const getFileCategory = (type: string): 'image' | 'video' | 'audio' | 'document' | 'other' => {
  if (ACCEPTED_FILE_TYPES.image.includes(type)) return 'image'
  if (ACCEPTED_FILE_TYPES.video.includes(type)) return 'video'
  if (ACCEPTED_FILE_TYPES.audio.includes(type)) return 'audio'
  if (ACCEPTED_FILE_TYPES.document.includes(type)) return 'document'
  return 'other'
}

export function MediaUploader({
  value = [],
  onChange,
  accept = 'image/*,video/*,audio/*,.pdf,.doc,.docx,.txt',
  maxFiles = 10,
  maxFileSize = 10 * 1024 * 1024, // 10MB
  disabled = false,
  multiple = true,
  showPreview = true,
  uploadEndpoint = '/api/upload',
  className
}: MediaUploaderProps) {
  const [files, setFiles] = useState<UploadedFile[]>(value)
  const [isDragOver, setIsDragOver] = useState(false)
  const fileInputRef = useRef<HTMLInputElement>(null)

  // 同步外部值变化
  useEffect(() => {
    setFiles(value)
  }, [value])

  // 文件变化时通知外部
  useEffect(() => {
    onChange(files)
  }, [files, onChange])

  const processFiles = useCallback(async (fileList: FileList) => {
    const newFiles: UploadedFile[] = []
    
    for (let i = 0; i < fileList.length; i++) {
      const file = fileList[i]
      
      // 检查文件数量限制
      if (files.length + newFiles.length >= maxFiles) {
        break
      }

      // 检查文件大小
      if (file.size > maxFileSize) {
        const errorFile: UploadedFile = {
          id: `file_${Date.now()}_${i}`,
          file,
          name: file.name,
          size: file.size,
          type: file.type,
          uploadProgress: 0,
          status: 'error',
          error: `文件大小超过限制 (${formatFileSize(maxFileSize)})`
        }
        newFiles.push(errorFile)
        continue
      }

      // 创建预览URL
      let url: string | undefined
      let thumbnailUrl: string | undefined
      
      if (getFileCategory(file.type) === 'image') {
        url = URL.createObjectURL(file)
        thumbnailUrl = url
      }

      const uploadFile: UploadedFile = {
        id: `file_${Date.now()}_${i}`,
        file,
        name: file.name,
        size: file.size,
        type: file.type,
        url,
        thumbnailUrl,
        uploadProgress: 0,
        status: 'pending'
      }

      newFiles.push(uploadFile)
    }

    setFiles(prev => [...prev, ...newFiles])

    // 开始上传
    newFiles.forEach(uploadFile => {
      if (uploadFile.status !== 'error') {
        startUpload(uploadFile)
      }
    })
  }, [files.length, maxFiles, maxFileSize])

  const startUpload = async (uploadFile: UploadedFile) => {
    // 更新状态为上传中
    setFiles(prev => prev.map(f => 
      f.id === uploadFile.id 
        ? { ...f, status: 'uploading' as const }
        : f
    ))

    try {
      const formData = new FormData()
      formData.append('file', uploadFile.file)

      // 模拟上传进度
      const xhr = new XMLHttpRequest()
      
      xhr.upload.addEventListener('progress', (e) => {
        if (e.lengthComputable) {
          const progress = Math.round((e.loaded / e.total) * 100)
          setFiles(prev => prev.map(f => 
            f.id === uploadFile.id 
              ? { ...f, uploadProgress: progress }
              : f
          ))
        }
      })

      xhr.addEventListener('load', () => {
        if (xhr.status === 200) {
          const response = JSON.parse(xhr.responseText)
          setFiles(prev => prev.map(f => 
            f.id === uploadFile.id 
              ? { 
                  ...f, 
                  status: 'completed' as const, 
                  uploadProgress: 100,
                  url: response.url,
                  thumbnailUrl: response.thumbnailUrl || response.url
                }
              : f
          ))
        } else {
          throw new Error(`上传失败: ${xhr.statusText}`)
        }
      })

      xhr.addEventListener('error', () => {
        throw new Error('网络错误，上传失败')
      })

      xhr.open('POST', uploadEndpoint)
      xhr.send(formData)

    } catch (error) {
      setFiles(prev => prev.map(f => 
        f.id === uploadFile.id 
          ? { 
              ...f, 
              status: 'error' as const,
              error: error instanceof Error ? error.message : '上传失败'
            }
          : f
      ))
    }
  }

  const removeFile = (fileId: string) => {
    setFiles(prev => {
      const fileToRemove = prev.find(f => f.id === fileId)
      if (fileToRemove?.url && fileToRemove.url.startsWith('blob:')) {
        URL.revokeObjectURL(fileToRemove.url)
      }
      return prev.filter(f => f.id !== fileId)
    })
  }

  const retryUpload = (fileId: string) => {
    const file = files.find(f => f.id === fileId)
    if (file) {
      startUpload(file)
    }
  }

  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault()
    e.stopPropagation()
    setIsDragOver(true)
  }, [])

  const handleDragLeave = useCallback((e: React.DragEvent) => {
    e.preventDefault()
    e.stopPropagation()
    setIsDragOver(false)
  }, [])

  const handleDrop = useCallback((e: React.DragEvent) => {
    e.preventDefault()
    e.stopPropagation()
    setIsDragOver(false)

    if (disabled) return

    const droppedFiles = e.dataTransfer.files
    if (droppedFiles.length > 0) {
      processFiles(droppedFiles)
    }
  }, [disabled, processFiles])

  const handleFileSelect = (e: React.ChangeEvent<HTMLInputElement>) => {
    if (e.target.files && e.target.files.length > 0) {
      processFiles(e.target.files)
    }
    // 清空input值，允许重复选择同一文件
    e.target.value = ''
  }

  const openFileDialog = () => {
    fileInputRef.current?.click()
  }

  return (
    <div className={cn('space-y-4', className)}>
      {/* 上传区域 */}
      <div
        className={cn(
          'border-2 border-dashed rounded-lg p-6 text-center cursor-pointer transition-colors',
          'hover:border-primary-400 hover:bg-primary-50',
          isDragOver && 'border-primary-500 bg-primary-100',
          disabled && 'opacity-50 cursor-not-allowed bg-gray-50',
          files.length >= maxFiles && 'opacity-50 cursor-not-allowed'
        )}
        onDragOver={handleDragOver}
        onDragLeave={handleDragLeave}
        onDrop={handleDrop}
        onClick={disabled || files.length >= maxFiles ? undefined : openFileDialog}
      >
        <Upload className="mx-auto h-12 w-12 text-gray-400 mb-4" />
        
        <div className="space-y-2">
          <p className="text-lg font-medium text-gray-900">
            {files.length >= maxFiles 
              ? `已达到最大文件数量 (${maxFiles})`
              : '拖拽文件到此处或点击上传'
            }
          </p>
          
          <p className="text-sm text-gray-500">
            支持 {accept.split(',').map(type => type.trim()).join(', ')}
          </p>
          
          <p className="text-xs text-gray-400">
            最大文件大小: {formatFileSize(maxFileSize)} | 
            最多 {maxFiles} 个文件 | 
            已上传: {files.filter(f => f.status === 'completed').length}/{files.length}
          </p>
        </div>
      </div>

      {/* 隐藏的文件输入 */}
      <input
        ref={fileInputRef}
        type="file"
        accept={accept}
        multiple={multiple}
        onChange={handleFileSelect}
        className="hidden"
        disabled={disabled}
      />

      {/* 文件列表 */}
      {files.length > 0 && (
        <div className="space-y-3">
          <h3 className="text-sm font-medium text-gray-900">
            已选择的文件 ({files.length})
          </h3>
          
          <div className={cn(
            'grid gap-3',
            showPreview ? 'grid-cols-1 md:grid-cols-2 lg:grid-cols-3' : 'grid-cols-1'
          )}>
            {files.map((file) => (
              <FileItem
                key={file.id}
                file={file}
                showPreview={showPreview}
                onRemove={() => removeFile(file.id)}
                onRetry={() => retryUpload(file.id)}
              />
            ))}
          </div>
        </div>
      )}

      {/* 统计信息 */}
      {files.length > 0 && (
        <div className="flex justify-between items-center text-sm text-gray-600 pt-2 border-t">
          <span>
            总大小: {formatFileSize(files.reduce((acc, f) => acc + f.size, 0))}
          </span>
          <span>
            完成: {files.filter(f => f.status === 'completed').length} | 
            失败: {files.filter(f => f.status === 'error').length}
          </span>
        </div>
      )}
    </div>
  )
}

interface FileItemProps {
  file: UploadedFile
  showPreview: boolean
  onRemove: () => void
  onRetry: () => void
}

function FileItem({ file, showPreview, onRemove, onRetry }: FileItemProps) {
  const IconComponent = getFileIcon(file.type)
  const category = getFileCategory(file.type)

  const getStatusColor = () => {
    switch (file.status) {
      case 'completed': return 'success'
      case 'error': return 'danger'
      case 'uploading': return 'warning'
      default: return 'secondary'
    }
  }

  const getStatusIcon = () => {
    switch (file.status) {
      case 'completed': return <Check className="h-4 w-4" />
      case 'error': return <AlertCircle className="h-4 w-4" />
      case 'uploading': return <Upload className="h-4 w-4 animate-pulse" />
      default: return <IconComponent className="h-4 w-4" />
    }
  }

  return (
    <Card className="overflow-hidden">
      <CardContent className="p-3">
        <div className="flex items-start gap-3">
          {/* 预览或图标 */}
          <div className="flex-shrink-0">
            {showPreview && category === 'image' && file.thumbnailUrl ? (
              <img
                src={file.thumbnailUrl}
                alt={file.name}
                className="w-16 h-16 object-cover rounded border"
              />
            ) : (
              <div className="w-16 h-16 flex items-center justify-center bg-gray-100 rounded border">
                <IconComponent className="h-8 w-8 text-gray-500" />
              </div>
            )}
          </div>

          {/* 文件信息 */}
          <div className="flex-1 min-w-0">
            <div className="flex items-start justify-between">
              <div className="flex-1 min-w-0">
                <p className="text-sm font-medium text-gray-900 truncate" title={file.name}>
                  {file.name}
                </p>
                <p className="text-xs text-gray-500">
                  {formatFileSize(file.size)}
                </p>
              </div>

              <div className="flex items-center gap-1 ml-2">
                <Badge variant={getStatusColor() as any} className="text-xs">
                  {getStatusIcon()}
                  <span className="ml-1">
                    {file.status === 'completed' && '完成'}
                    {file.status === 'error' && '失败'}
                    {file.status === 'uploading' && '上传中'}
                    {file.status === 'pending' && '等待'}
                  </span>
                </Badge>

                {file.status === 'error' && (
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={onRetry}
                    className="h-6 w-6 p-0"
                    title="重试上传"
                  >
                    <RotateCcw className="h-3 w-3" />
                  </Button>
                )}

                <Button
                  variant="ghost"
                  size="sm"
                  onClick={onRemove}
                  className="h-6 w-6 p-0 text-red-600 hover:text-red-700"
                  title="删除文件"
                >
                  <X className="h-3 w-3" />
                </Button>
              </div>
            </div>

            {/* 上传进度 */}
            {file.status === 'uploading' && (
              <div className="mt-2">
                <Progress value={file.uploadProgress} className="h-1" />
                <p className="text-xs text-gray-500 mt-1">
                  {file.uploadProgress}%
                </p>
              </div>
            )}

            {/* 错误信息 */}
            {file.status === 'error' && file.error && (
              <p className="text-xs text-red-600 mt-1" title={file.error}>
                {file.error}
              </p>
            )}
          </div>
        </div>
      </CardContent>
    </Card>
  )
}

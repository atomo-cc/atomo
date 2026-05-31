/**
 * JSON Editor - 增强的JSON字段编辑器
 * 
 * 功能：
 * - 语法高亮
 * - 错误提示
 * - 格式化
 * - 预览模式
 */

import React, { useState, useEffect } from 'react'
import { Button } from '../ui/Button'
import { Textarea } from '../ui/Textarea'
import { Badge } from '../ui/Badge'
import { 
  Eye, 
  EyeOff, 
  CheckCircle, 
  AlertCircle, 
  RotateCcw,
  Copy,
  FileText
} from 'lucide-react'

interface JsonEditorProps {
  value: any
  onChange: (value: any) => void
  disabled?: boolean
  error?: string
  placeholder?: string
  className?: string
}

export function JsonEditor({
  value,
  onChange,
  disabled = false,
  error,
  placeholder = '请输入有效的JSON',
  className = ''
}: JsonEditorProps): React.JSX.Element {
  const [jsonText, setJsonText] = useState('')
  const [isValid, setIsValid] = useState(true)
  const [validationError, setValidationError] = useState('')
  const [showPreview, setShowPreview] = useState(false)
  const [isFormatted, setIsFormatted] = useState(false)

  // 初始化和同步外部值
  useEffect(() => {
    if (value !== undefined && value !== null) {
      try {
        const formatted = typeof value === 'string' ? value : JSON.stringify(value, null, 2)
        setJsonText(formatted)
        setIsValid(true)
        setValidationError('')
        setIsFormatted(formatted.includes('\n'))
      } catch (err) {
        setJsonText(String(value))
        setIsValid(false)
        setValidationError('无效的JSON格式')
      }
    } else {
      setJsonText('')
      setIsValid(true)
      setValidationError('')
    }
  }, [value])

  // JSON文本变化处理
  const handleTextChange = (newText: string) => {
    setJsonText(newText)
    
    if (!newText.trim()) {
      setIsValid(true)
      setValidationError('')
      onChange(null)
      return
    }

    try {
      const parsed = JSON.parse(newText)
      setIsValid(true)
      setValidationError('')
      onChange(parsed)
    } catch (err) {
      setIsValid(false)
      const errorMessage = err instanceof Error ? err.message : '无效的JSON格式'
      setValidationError(errorMessage)
      // 不立即调用onChange，让用户继续编辑
    }
  }

  // 格式化JSON
  const formatJson = () => {
    if (!isValid) return
    
    try {
      const parsed = JSON.parse(jsonText)
      const formatted = JSON.stringify(parsed, null, 2)
      setJsonText(formatted)
      setIsFormatted(true)
    } catch (err) {
      // 已经在handleTextChange中处理了错误
    }
  }

  // 压缩JSON
  const compactJson = () => {
    if (!isValid) return
    
    try {
      const parsed = JSON.parse(jsonText)
      const compacted = JSON.stringify(parsed)
      setJsonText(compacted)
      setIsFormatted(false)
    } catch (err) {
      // 已经在handleTextChange中处理了错误
    }
  }

  // 重置
  const reset = () => {
    setJsonText('')
    setIsValid(true)
    setValidationError('')
    onChange(null)
  }

  // 复制到剪贴板
  const copyToClipboard = async () => {
    try {
      await navigator.clipboard.writeText(jsonText)
    } catch (err) {
      console.error('复制失败:', err)
    }
  }

  // 获取JSON预览信息
  const getPreviewInfo = () => {
    if (!isValid || !jsonText.trim()) return null
    
    try {
      const parsed = JSON.parse(jsonText)
      if (Array.isArray(parsed)) {
        return {
          type: '数组',
          length: parsed.length,
          preview: `[${parsed.length} 项]`
        }
      } else if (typeof parsed === 'object' && parsed !== null) {
        const keys = Object.keys(parsed)
        return {
          type: '对象',
          length: keys.length,
          preview: `{${keys.length} 个字段: ${keys.slice(0, 3).join(', ')}${keys.length > 3 ? '...' : ''}}}`
        }
      } else {
        return {
          type: typeof parsed,
          length: 1,
          preview: String(parsed)
        }
      }
    } catch {
      return null
    }
  }

  const previewInfo = getPreviewInfo()

  return (
    <div className={`space-y-3 ${className}`}>
      {/* 工具栏 */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <Badge variant={isValid ? 'success' : 'danger'} className="text-xs">
            {isValid ? (
              <>
                <CheckCircle className="w-3 h-3 mr-1" />
                有效
              </>
            ) : (
              <>
                <AlertCircle className="w-3 h-3 mr-1" />
                错误
              </>
            )}
          </Badge>
          
          {previewInfo && (
            <Badge variant="secondary" className="text-xs">
              <FileText className="w-3 h-3 mr-1" />
              {previewInfo.preview}
            </Badge>
          )}
        </div>

        <div className="flex items-center gap-1">
          <Button
            type="button"
            variant="ghost"
            size="sm"
            onClick={() => setShowPreview(!showPreview)}
            disabled={disabled || !isValid}
            title={showPreview ? "隐藏预览" : "显示预览"}
          >
            {showPreview ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />}
          </Button>
          
          <Button
            type="button"
            variant="ghost"
            size="sm"
            onClick={isFormatted ? compactJson : formatJson}
            disabled={disabled || !isValid || !jsonText.trim()}
            title={isFormatted ? "压缩" : "格式化"}
          >
            <FileText className="w-4 h-4" />
          </Button>
          
          <Button
            type="button"
            variant="ghost"
            size="sm"
            onClick={copyToClipboard}
            disabled={disabled || !jsonText.trim()}
            title="复制"
          >
            <Copy className="w-4 h-4" />
          </Button>
          
          <Button
            type="button"
            variant="ghost"
            size="sm"
            onClick={reset}
            disabled={disabled}
            title="重置"
          >
            <RotateCcw className="w-4 h-4" />
          </Button>
        </div>
      </div>

      {/* JSON编辑器 */}
      <div className="relative">
        <Textarea
          value={jsonText}
          onChange={(e) => handleTextChange(e.target.value)}
          placeholder={placeholder}
          disabled={disabled}
          error={validationError || error}
          className="font-mono text-sm min-h-[120px] resize-y"
          style={{
            background: isValid ? undefined : '#fef2f2'
          }}
        />
        
        {/* 错误提示 */}
        {validationError && (
          <div className="absolute top-2 right-2">
            <Badge variant="danger" className="text-xs">
              <AlertCircle className="w-3 h-3 mr-1" />
              JSON错误
            </Badge>
          </div>
        )}
      </div>

      {/* 预览面板 */}
      {showPreview && isValid && jsonText.trim() && (
        <div className="border border-gray-200 rounded-md p-3 bg-gray-50">
          <div className="text-sm font-medium text-gray-700 mb-2">
            预览 ({previewInfo?.type})
          </div>
          <pre className="text-xs text-gray-600 overflow-auto max-h-40 whitespace-pre-wrap">
            {JSON.stringify(JSON.parse(jsonText), null, 2)}
          </pre>
        </div>
      )}

      {/* 错误详情 */}
      {validationError && (
        <div className="text-sm text-red-600 bg-red-50 border border-red-200 rounded-md p-3">
          <div className="font-medium mb-1">JSON语法错误:</div>
          <div className="font-mono">{validationError}</div>
          <div className="mt-2 text-xs text-red-500">
            请检查括号、引号和逗号是否正确匹配
          </div>
        </div>
      )}
    </div>
  )
}

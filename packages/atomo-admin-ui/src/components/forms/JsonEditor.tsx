/**
 * JSON Editor - enhanced JSON field editor
 *
 * Features:
 * - Syntax highlighting
 * - Error reporting
 * - Formatting
 * - Preview mode
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
  placeholder = 'Enter valid JSON',
  className = ''
}: JsonEditorProps): React.JSX.Element {
  const [jsonText, setJsonText] = useState('')
  const [isValid, setIsValid] = useState(true)
  const [validationError, setValidationError] = useState('')
  const [showPreview, setShowPreview] = useState(false)
  const [isFormatted, setIsFormatted] = useState(false)

  // Initialize and sync with the external value
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
        setValidationError('Invalid JSON format')
      }
    } else {
      setJsonText('')
      setIsValid(true)
      setValidationError('')
    }
  }, [value])

  // Handle changes to the JSON text
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
      const errorMessage = err instanceof Error ? err.message : 'Invalid JSON format'
      setValidationError(errorMessage)
      // Don't call onChange immediately, so the user can keep editing
    }
  }

  // Format the JSON
  const formatJson = () => {
    if (!isValid) return
    
    try {
      const parsed = JSON.parse(jsonText)
      const formatted = JSON.stringify(parsed, null, 2)
      setJsonText(formatted)
      setIsFormatted(true)
    } catch (err) {
      // The error is already handled in handleTextChange
    }
  }

  // Minify the JSON
  const compactJson = () => {
    if (!isValid) return
    
    try {
      const parsed = JSON.parse(jsonText)
      const compacted = JSON.stringify(parsed)
      setJsonText(compacted)
      setIsFormatted(false)
    } catch (err) {
      // The error is already handled in handleTextChange
    }
  }

  // Reset
  const reset = () => {
    setJsonText('')
    setIsValid(true)
    setValidationError('')
    onChange(null)
  }

  // Copy to clipboard
  const copyToClipboard = async () => {
    try {
      await navigator.clipboard.writeText(jsonText)
    } catch (err) {
      console.error('Copy failed:', err)
    }
  }

  // Get JSON preview info
  const getPreviewInfo = () => {
    if (!isValid || !jsonText.trim()) return null
    
    try {
      const parsed = JSON.parse(jsonText)
      if (Array.isArray(parsed)) {
        return {
          type: 'Array',
          length: parsed.length,
          preview: `[${parsed.length} items]`
        }
      } else if (typeof parsed === 'object' && parsed !== null) {
        const keys = Object.keys(parsed)
        return {
          type: 'Object',
          length: keys.length,
          preview: `{${keys.length} fields: ${keys.slice(0, 3).join(', ')}${keys.length > 3 ? '...' : ''}}}`
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
      {/* Toolbar */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <Badge variant={isValid ? 'success' : 'danger'} className="text-xs">
            {isValid ? (
              <>
                <CheckCircle className="w-3 h-3 mr-1" />
                Valid
              </>
            ) : (
              <>
                <AlertCircle className="w-3 h-3 mr-1" />
                Error
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
            title={showPreview ? "Hide preview" : "Show preview"}
          >
            {showPreview ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />}
          </Button>
          
          <Button
            type="button"
            variant="ghost"
            size="sm"
            onClick={isFormatted ? compactJson : formatJson}
            disabled={disabled || !isValid || !jsonText.trim()}
            title={isFormatted ? "Minify" : "Format"}
          >
            <FileText className="w-4 h-4" />
          </Button>
          
          <Button
            type="button"
            variant="ghost"
            size="sm"
            onClick={copyToClipboard}
            disabled={disabled || !jsonText.trim()}
            title="Copy"
          >
            <Copy className="w-4 h-4" />
          </Button>
          
          <Button
            type="button"
            variant="ghost"
            size="sm"
            onClick={reset}
            disabled={disabled}
            title="Reset"
          >
            <RotateCcw className="w-4 h-4" />
          </Button>
        </div>
      </div>

      {/* JSON editor */}
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
        
        {/* Error indicator */}
        {validationError && (
          <div className="absolute top-2 right-2">
            <Badge variant="danger" className="text-xs">
              <AlertCircle className="w-3 h-3 mr-1" />
              JSON error
            </Badge>
          </div>
        )}
      </div>

      {/* Preview panel */}
      {showPreview && isValid && jsonText.trim() && (
        <div className="border border-bn-border rounded-bn p-3 bg-content-bg">
          <div className="text-sm font-medium text-foreground mb-2">
            Preview ({previewInfo?.type})
          </div>
          <pre className="text-xs text-icon-muted font-mono overflow-auto max-h-40 whitespace-pre-wrap">
            {JSON.stringify(JSON.parse(jsonText), null, 2)}
          </pre>
        </div>
      )}

      {/* Error details */}
      {validationError && (
        <div className="text-sm text-rose-600 dark:text-rose-400 bg-rose-500/10 border border-rose-500/20 rounded-bn p-3">
          <div className="font-medium mb-1">JSON syntax error:</div>
          <div className="font-mono text-xs">{validationError}</div>
          <div className="mt-2 text-xs opacity-80">
            Check that brackets, quotes, and commas are correctly matched
          </div>
        </div>
      )}
    </div>
  )
}

/**
 * Magic Wand - AI魔法棒组件
 * 
 * 为文本编辑器提供AI增强功能，包括：
 * - 内容生成
 * - 文本润色
 * - 风格转换
 * - 智能建议
 */

import React, { useState, useRef, useEffect } from 'react'
import { 
  Wand2, 
  Sparkles, 
  RefreshCw, 
  Check, 
  X, 
  MessageSquare,
  PenTool,
  Zap,
  Lightbulb,
  Copy,
  RotateCcw
} from 'lucide-react'

import { Button } from '../ui/Button'
import { Card, CardContent } from '../ui/Card'
import { Badge } from '../ui/Badge'
import { useAIAssistant, AIResponse } from '../../lib/ai-assistant'
import { cn } from '../../lib/utils'

interface MagicWandProps {
  selectedText?: string
  onTextReplace?: (text: string) => void
  onTextInsert?: (text: string) => void
  position?: { x: number; y: number }
  visible: boolean
  onClose: () => void
}

export function MagicWand({
  selectedText = '',
  onTextReplace,
  onTextInsert,
  position,
  visible,
  onClose
}: MagicWandProps) {
  const { assistant, capabilities, isLoading } = useAIAssistant()
  const [activeAction, setActiveAction] = useState<string | null>(null)
  const [aiResponse, setAiResponse] = useState<AIResponse | null>(null)
  const [customPrompt, setCustomPrompt] = useState('')
  const [showCustomPrompt, setShowCustomPrompt] = useState(false)
  const wrapperRef = useRef<HTMLDivElement>(null)

  // 点击外部关闭
  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (wrapperRef.current && !wrapperRef.current.contains(event.target as Node)) {
        onClose()
      }
    }

    if (visible) {
      document.addEventListener('mousedown', handleClickOutside)
      return () => document.removeEventListener('mousedown', handleClickOutside)
    }
  }, [visible, onClose])

  const magicActions = [
    {
      id: 'improve',
      name: '润色文本',
      icon: <PenTool className="h-4 w-4" />,
      description: '优化语言表达和逻辑结构',
      needsSelection: true,
      prompt: '请优化这段文本，使其更加清晰、专业且易读'
    },
    {
      id: 'generate',
      name: '续写内容',
      icon: <MessageSquare className="h-4 w-4" />,
      description: '基于上下文生成后续内容',
      needsSelection: false,
      prompt: '请基于上述内容，继续生成相关的文本'
    },
    {
      id: 'summarize',
      name: '总结要点',
      icon: <Lightbulb className="h-4 w-4" />,
      description: '提取关键信息和要点',
      needsSelection: true,
      prompt: '请总结这段文本的关键要点'
    },
    {
      id: 'expand',
      name: '扩展内容',
      icon: <Zap className="h-4 w-4" />,
      description: '添加更多细节和说明',
      needsSelection: true,
      prompt: '请扩展这段文本，添加更多细节和解释'
    },
    {
      id: 'translate',
      name: '翻译文本',
      icon: <RefreshCw className="h-4 w-4" />,
      description: '翻译为其他语言',
      needsSelection: true,
      prompt: '请将这段文本翻译为英文'
    },
    {
      id: 'custom',
      name: '自定义',
      icon: <Wand2 className="h-4 w-4" />,
      description: '使用自定义提示',
      needsSelection: false,
      prompt: ''
    }
  ]

  const handleMagicAction = async (action: typeof magicActions[0]) => {
    if (action.needsSelection && !selectedText) {
      alert('请先选择文本')
      return
    }

    if (action.id === 'custom') {
      setShowCustomPrompt(true)
      return
    }

    setActiveAction(action.id)
    setAiResponse(null)

    try {
      let response: AIResponse

      if (action.id === 'improve' && selectedText) {
        response = await assistant.improveText(selectedText)
      } else {
        response = await assistant.generateText(
          action.prompt,
          selectedText || '当前上下文'
        )
      }

      setAiResponse(response)
    } catch (error) {
      console.error('AI处理失败:', error)
      alert('AI处理失败，请稍后重试')
      setActiveAction(null)
    }
  }

  const handleCustomPrompt = async () => {
    if (!customPrompt.trim()) return

    setActiveAction('custom')
    setAiResponse(null)
    setShowCustomPrompt(false)

    try {
      const response = await assistant.generateText(
        customPrompt,
        selectedText || '当前上下文'
      )
      setAiResponse(response)
    } catch (error) {
      console.error('AI处理失败:', error)
      alert('AI处理失败，请稍后重试')
      setActiveAction(null)
    }
  }

  const handleAcceptSuggestion = () => {
    if (!aiResponse) return

    if (selectedText && onTextReplace) {
      onTextReplace(aiResponse.content)
    } else if (onTextInsert) {
      onTextInsert(aiResponse.content)
    }

    setAiResponse(null)
    setActiveAction(null)
    onClose()
  }

  const handleRejectSuggestion = () => {
    setAiResponse(null)
    setActiveAction(null)
  }

  const handleRetry = async () => {
    if (!activeAction) return

    const action = magicActions.find(a => a.id === activeAction)
    if (action) {
      await handleMagicAction(action)
    }
  }

  const copyToClipboard = () => {
    if (aiResponse) {
      navigator.clipboard.writeText(aiResponse.content)
    }
  }

  if (!visible) return null

  return (
    <div
      ref={wrapperRef}
      className="fixed z-50 bg-white border border-gray-200 rounded-lg shadow-lg"
      style={{
        left: position?.x || '50%',
        top: position?.y || '50%',
        transform: position ? 'none' : 'translate(-50%, -50%)',
        maxWidth: '400px',
        minWidth: '300px'
      }}
    >
      {/* AI响应显示 */}
      {aiResponse && (
        <div className="p-4 border-b border-gray-200">
          <div className="flex items-center justify-between mb-3">
            <div className="flex items-center gap-2">
              <Sparkles className="h-4 w-4 text-purple-600" />
              <span className="font-medium text-sm">AI建议</span>
              <Badge variant="secondary" className="text-xs">
                {Math.round(aiResponse.confidence * 100)}%
              </Badge>
            </div>
            <div className="flex items-center gap-1">
              <Button variant="ghost" size="sm" onClick={copyToClipboard}>
                <Copy className="h-3 w-3" />
              </Button>
              <Button variant="ghost" size="sm" onClick={handleRetry}>
                <RotateCcw className="h-3 w-3" />
              </Button>
            </div>
          </div>

          <div className="bg-gray-50 rounded-md p-3 mb-3 text-sm">
            {aiResponse.content}
          </div>

          <div className="flex items-center gap-2">
            <Button size="sm" onClick={handleAcceptSuggestion}>
              <Check className="h-3 w-3 mr-1" />
              采用
            </Button>
            <Button variant="secondary" size="sm" onClick={handleRejectSuggestion}>
              <X className="h-3 w-3 mr-1" />
              拒绝
            </Button>
            {aiResponse.alternatives && aiResponse.alternatives.length > 0 && (
              <span className="text-xs text-gray-500">
                +{aiResponse.alternatives.length} 个备选方案
              </span>
            )}
          </div>
        </div>
      )}

      {/* 自定义提示输入 */}
      {showCustomPrompt && (
        <div className="p-4 border-b border-gray-200">
          <div className="flex items-center gap-2 mb-3">
            <Wand2 className="h-4 w-4 text-purple-600" />
            <span className="font-medium text-sm">自定义提示</span>
          </div>
          
          <textarea
            value={customPrompt}
            onChange={(e) => setCustomPrompt(e.target.value)}
            placeholder="输入您的自定义提示..."
            className="w-full h-20 p-2 text-sm border border-gray-300 rounded-md resize-none"
            autoFocus
          />
          
          <div className="flex items-center gap-2 mt-3">
            <Button size="sm" onClick={handleCustomPrompt} disabled={!customPrompt.trim()}>
              <Zap className="h-3 w-3 mr-1" />
              执行
            </Button>
            <Button variant="secondary" size="sm" onClick={() => setShowCustomPrompt(false)}>
              取消
            </Button>
          </div>
        </div>
      )}

      {/* 魔法操作列表 */}
      {!aiResponse && !showCustomPrompt && (
        <div className="p-4">
          <div className="flex items-center gap-2 mb-3">
            <Wand2 className="h-4 w-4 text-purple-600" />
            <span className="font-medium text-sm">AI魔法棒</span>
            {selectedText && (
              <Badge variant="secondary" className="text-xs">
                已选择 {selectedText.length} 字符
              </Badge>
            )}
          </div>

          <div className="space-y-1">
            {magicActions.map((action) => (
              <button
                key={action.id}
                onClick={() => handleMagicAction(action)}
                disabled={isLoading || (action.needsSelection && !selectedText)}
                className={cn(
                  "w-full flex items-center gap-3 p-2 text-left rounded-md transition-colors",
                  "hover:bg-gray-50 disabled:opacity-50 disabled:cursor-not-allowed",
                  activeAction === action.id && "bg-purple-50 border border-purple-200"
                )}
              >
                <div className="flex-shrink-0">
                  {activeAction === action.id && isLoading ? (
                    <div className="animate-spin rounded-full h-4 w-4 border-2 border-purple-600 border-t-transparent" />
                  ) : (
                    action.icon
                  )}
                </div>
                <div className="flex-1 min-w-0">
                  <div className="font-medium text-sm text-gray-900">
                    {action.name}
                  </div>
                  <div className="text-xs text-gray-500 truncate">
                    {action.description}
                  </div>
                </div>
              </button>
            ))}
          </div>

          {capabilities.length === 0 && (
            <div className="text-center py-4 text-gray-500 text-sm">
              AI功能暂不可用
            </div>
          )}
        </div>
      )}

      {/* 关闭按钮 */}
      <div className="absolute top-2 right-2">
        <Button variant="ghost" size="sm" onClick={onClose}>
          <X className="h-3 w-3" />
        </Button>
      </div>
    </div>
  )
}

/**
 * AI增强的文本区域组件
 */
interface AIEnhancedTextareaProps {
  value: string
  onChange: (value: string) => void
  placeholder?: string
  className?: string
  disabled?: boolean
}

export function AIEnhancedTextarea({
  value,
  onChange,
  placeholder,
  className,
  disabled
}: AIEnhancedTextareaProps) {
  const [showMagicWand, setShowMagicWand] = useState(false)
  const [selectedText, setSelectedText] = useState('')
  const [magicWandPosition, setMagicWandPosition] = useState<{ x: number; y: number } | undefined>()
  const textareaRef = useRef<HTMLTextAreaElement>(null)

  const handleTextSelection = () => {
    if (!textareaRef.current) return

    const textarea = textareaRef.current
    const start = textarea.selectionStart
    const end = textarea.selectionEnd
    const selected = value.substring(start, end)

    setSelectedText(selected)

    if (selected.length > 0) {
      // 计算选择位置
      const rect = textarea.getBoundingClientRect()
      setMagicWandPosition({
        x: rect.left + 20,
        y: rect.top + 20
      })
    }
  }

  const handleMagicWandTrigger = (e: React.KeyboardEvent) => {
    // Ctrl/Cmd + M 打开魔法棒
    if ((e.ctrlKey || e.metaKey) && e.key === 'm') {
      e.preventDefault()
      handleTextSelection()
      setShowMagicWand(true)
    }
  }

  const handleTextReplace = (newText: string) => {
    if (!textareaRef.current) return

    const textarea = textareaRef.current
    const start = textarea.selectionStart
    const end = textarea.selectionEnd
    
    const newValue = value.substring(0, start) + newText + value.substring(end)
    onChange(newValue)

    // 设置新的光标位置
    setTimeout(() => {
      textarea.focus()
      textarea.setSelectionRange(start + newText.length, start + newText.length)
    }, 0)
  }

  const handleTextInsert = (newText: string) => {
    if (!textareaRef.current) return

    const textarea = textareaRef.current
    const start = textarea.selectionStart
    
    const newValue = value.substring(0, start) + newText + value.substring(start)
    onChange(newValue)

    // 设置新的光标位置
    setTimeout(() => {
      textarea.focus()
      textarea.setSelectionRange(start + newText.length, start + newText.length)
    }, 0)
  }

  return (
    <div className="relative">
      <textarea
        ref={textareaRef}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        onSelect={handleTextSelection}
        onKeyDown={handleMagicWandTrigger}
        placeholder={placeholder}
        disabled={disabled}
        className={cn(
          "w-full min-h-32 p-3 border border-gray-300 rounded-md resize-y",
          "focus:ring-2 focus:ring-primary-500 focus:border-primary-500",
          "disabled:opacity-50 disabled:cursor-not-allowed",
          className
        )}
      />
      
      {/* AI魔法棒按钮 */}
      <Button
        variant="ghost"
        size="sm"
        onClick={() => {
          handleTextSelection()
          setShowMagicWand(true)
        }}
        className="absolute bottom-2 right-2 opacity-70 hover:opacity-100"
        title="AI魔法棒 (Ctrl+M)"
      >
        <Wand2 className="h-4 w-4 text-purple-600" />
      </Button>

      {/* 魔法棒组件 */}
      <MagicWand
        selectedText={selectedText}
        onTextReplace={handleTextReplace}
        onTextInsert={handleTextInsert}
        position={magicWandPosition}
        visible={showMagicWand}
        onClose={() => setShowMagicWand(false)}
      />
    </div>
  )
}

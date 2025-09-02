/**
 * Tag Input - 标签输入组件
 * 
 * 支持添加、删除标签，输入提示等功能
 */

import { useState, useRef, KeyboardEvent } from 'react'
import { X, Plus } from 'lucide-react'
import { cn } from '../../lib/utils'
import { Badge } from '../ui/Badge'

interface TagInputProps {
  value: string[]
  onChange: (tags: string[]) => void
  disabled?: boolean
  error?: string
  placeholder?: string
  suggestions?: string[]
  maxTags?: number
  allowDuplicates?: boolean
}

export function TagInput({
  value = [],
  onChange,
  disabled = false,
  error,
  placeholder = '输入标签并按回车',
  suggestions = [],
  maxTags,
  allowDuplicates = false
}: TagInputProps) {
  const [inputValue, setInputValue] = useState('')
  const [showSuggestions, setShowSuggestions] = useState(false)
  const inputRef = useRef<HTMLInputElement>(null)

  // 过滤建议
  const filteredSuggestions = suggestions.filter(
    suggestion => 
      suggestion.toLowerCase().includes(inputValue.toLowerCase()) &&
      (allowDuplicates || !value.includes(suggestion))
  )

  // 添加标签
  const addTag = (tag: string) => {
    const trimmedTag = tag.trim()
    if (!trimmedTag) return

    if (!allowDuplicates && value.includes(trimmedTag)) {
      setInputValue('')
      return
    }

    if (maxTags && value.length >= maxTags) {
      return
    }

    onChange([...value, trimmedTag])
    setInputValue('')
    setShowSuggestions(false)
  }

  // 删除标签
  const removeTag = (indexToRemove: number) => {
    onChange(value.filter((_, index) => index !== indexToRemove))
  }

  // 键盘事件处理
  const handleKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    switch (e.key) {
      case 'Enter':
        e.preventDefault()
        if (inputValue) {
          addTag(inputValue)
        }
        break
        
      case 'Backspace':
        if (!inputValue && value.length > 0) {
          removeTag(value.length - 1)
        }
        break
        
      case 'Escape':
        setShowSuggestions(false)
        break
        
      case 'ArrowDown':
      case 'ArrowUp':
        // TODO: 实现建议列表导航
        break
    }
  }

  // 输入变化处理
  const handleInputChange = (newValue: string) => {
    setInputValue(newValue)
    setShowSuggestions(newValue.length > 0 && filteredSuggestions.length > 0)
  }

  // 点击建议项
  const handleSuggestionClick = (suggestion: string) => {
    addTag(suggestion)
    inputRef.current?.focus()
  }

  const canAddMore = !maxTags || value.length < maxTags

  return (
    <div className="space-y-2">
      {/* 标签容器 */}
      <div
        className={cn(
          'min-h-[2.5rem] w-full rounded-md border border-gray-300 bg-white px-3 py-2',
          'focus-within:ring-2 focus-within:ring-primary-500 focus-within:border-transparent',
          disabled && 'opacity-50 cursor-not-allowed',
          error && 'border-danger-500 focus-within:ring-danger-500',
          'flex flex-wrap gap-2 items-center'
        )}
        onClick={() => !disabled && inputRef.current?.focus()}
      >
        {/* 已有标签 */}
        {value.map((tag, index) => (
          <Badge
            key={index}
            variant="secondary"
            className="flex items-center gap-1 pl-2 pr-1"
          >
            <span>{tag}</span>
            {!disabled && (
              <button
                type="button"
                onClick={(e) => {
                  e.stopPropagation()
                  removeTag(index)
                }}
                className="hover:bg-gray-200 rounded-full p-0.5 ml-1"
              >
                <X className="h-3 w-3" />
              </button>
            )}
          </Badge>
        ))}

        {/* 输入框 */}
        {canAddMore && !disabled && (
          <input
            ref={inputRef}
            type="text"
            value={inputValue}
            onChange={(e) => handleInputChange(e.target.value)}
            onKeyDown={handleKeyDown}
            onFocus={() => setShowSuggestions(inputValue.length > 0 && filteredSuggestions.length > 0)}
            onBlur={() => setTimeout(() => setShowSuggestions(false), 200)}
            placeholder={value.length === 0 ? placeholder : ''}
            className="flex-1 min-w-[120px] outline-none bg-transparent"
          />
        )}

        {/* 添加按钮 */}
        {canAddMore && !disabled && inputValue && (
          <button
            type="button"
            onClick={() => addTag(inputValue)}
            className="p-1 hover:bg-gray-100 rounded"
          >
            <Plus className="h-4 w-4 text-gray-500" />
          </button>
        )}
      </div>

      {/* 建议列表 */}
      {showSuggestions && filteredSuggestions.length > 0 && (
        <div className="relative">
          <div className="absolute top-0 left-0 right-0 z-10 bg-white border border-gray-200 rounded-md shadow-lg max-h-40 overflow-y-auto">
            {filteredSuggestions.map((suggestion, index) => (
              <button
                key={index}
                type="button"
                className="w-full text-left px-3 py-2 hover:bg-gray-50 text-sm"
                onClick={() => handleSuggestionClick(suggestion)}
              >
                {suggestion}
              </button>
            ))}
          </div>
        </div>
      )}

      {/* 错误信息 */}
      {error && (
        <p className="text-sm text-danger-600">{error}</p>
      )}

      {/* 帮助信息 */}
      {maxTags && (
        <p className="text-xs text-gray-500">
          {value.length} / {maxTags} 个标签
          {value.length >= maxTags && ' (已达上限)'}
        </p>
      )}
    </div>
  )
}
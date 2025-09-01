/**
 * Tag Input - 标签输入组件
 * 
 * 支持添加、删除标签，快捷键操作
 */

import React, { useState, KeyboardEvent } from 'react'
import { X } from 'lucide-react'
import { cn } from '../../lib/utils'

interface TagInputProps {
  value: string[]
  onChange: (tags: string[]) => void
  disabled?: boolean
  error?: string
  placeholder?: string
  maxTags?: number
}

export function TagInput({
  value = [],
  onChange,
  disabled = false,
  error,
  placeholder = '输入标签后按回车添加',
  maxTags = 20
}: TagInputProps) {
  const [inputValue, setInputValue] = useState('')

  const addTag = (tag: string) => {
    const trimmedTag = tag.trim()
    if (!trimmedTag) return
    
    if (value.includes(trimmedTag)) {
      setInputValue('')
      return
    }
    
    if (value.length >= maxTags) {
      setInputValue('')
      return
    }
    
    onChange([...value, trimmedTag])
    setInputValue('')
  }

  const removeTag = (indexToRemove: number) => {
    onChange(value.filter((_, index) => index !== indexToRemove))
  }

  const handleKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (disabled) return
    
    switch (e.key) {
      case 'Enter':
        e.preventDefault()
        addTag(inputValue)
        break
        
      case 'Backspace':
        if (!inputValue && value.length > 0) {
          removeTag(value.length - 1)
        }
        break
        
      case ',':
      case ';':
        e.preventDefault()
        addTag(inputValue)
        break
    }
  }

  const handleBlur = () => {
    if (inputValue.trim()) {
      addTag(inputValue)
    }
  }

  return (
    <div className="space-y-2">
      {/* 标签容器 */}
      <div
        className={cn(
          'flex flex-wrap gap-2 min-h-[40px] p-2 border border-gray-300 rounded-md bg-white',
          'focus-within:ring-2 focus-within:ring-primary-500 focus-within:border-transparent',
          disabled && 'bg-gray-50 cursor-not-allowed',
          error && 'border-danger-500 focus-within:ring-danger-500'
        )}
      >
        {/* 已有标签 */}
        {value.map((tag, index) => (
          <span
            key={index}
            className="inline-flex items-center gap-1 px-2 py-1 bg-primary-100 text-primary-800 text-sm rounded-md"
          >
            {tag}
            {!disabled && (
              <button
                type="button"
                onClick={() => removeTag(index)}
                className="p-0.5 hover:bg-primary-200 rounded-full transition-colors"
              >
                <X className="h-3 w-3" />
              </button>
            )}
          </span>
        ))}

        {/* 输入框 */}
        {!disabled && value.length < maxTags && (
          <input
            type="text"
            value={inputValue}
            onChange={(e) => setInputValue(e.target.value)}
            onKeyDown={handleKeyDown}
            onBlur={handleBlur}
            placeholder={value.length === 0 ? placeholder : ''}
            className="flex-1 min-w-[120px] outline-none bg-transparent text-sm"
          />
        )}
      </div>

      {/* 帮助文本 */}
      <div className="text-xs text-gray-500">
        {!error && (
          <span>
            已添加 {value.length}/{maxTags} 个标签
            {!disabled && '，支持回车、逗号、分号分隔'}
          </span>
        )}
      </div>

      {/* 错误提示 */}
      {error && (
        <p className="text-sm text-danger-600">{error}</p>
      )}
    </div>
  )
}

/**
 * Blocks Editor - Atomo 富文本块编辑器
 * 
 * 这是 Atomo "流动画布" 的核心组件，支持组合式内容创作
 */

import React, { useState } from 'react'
import { Plus, GripVertical, Trash2, Type, Image, Code } from 'lucide-react'
import { Button } from '../ui/Button'
import { Textarea } from '../ui/Textarea'
import { Input } from '../ui/Input'

interface Block {
  id: string
  type: string
  data: any
}

interface BlocksEditorProps {
  value: Block[]
  onChange: (blocks: Block[]) => void
  disabled?: boolean
  error?: string
}

const BLOCK_TYPES = [
  { type: 'paragraph', label: '段落', icon: Type },
  { type: 'heading', label: '标题', icon: Type },
  { type: 'image', label: '图片', icon: Image },
  { type: 'code', label: '代码', icon: Code },
]

export function BlocksEditor({
  value = [],
  onChange,
  disabled = false,
  error
}: BlocksEditorProps) {
  const [showAddMenu, setShowAddMenu] = useState(false)

  const addBlock = (type: string) => {
    const newBlock: Block = {
      id: `block_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`,
      type,
      data: getDefaultBlockData(type)
    }
    
    onChange([...value, newBlock])
    setShowAddMenu(false)
  }

  const updateBlock = (blockId: string, data: any) => {
    onChange(value.map(block => 
      block.id === blockId ? { ...block, data } : block
    ))
  }

  const removeBlock = (blockId: string) => {
    onChange(value.filter(block => block.id !== blockId))
  }

  const moveBlock = (fromIndex: number, toIndex: number) => {
    const newBlocks = [...value]
    const [movedBlock] = newBlocks.splice(fromIndex, 1)
    newBlocks.splice(toIndex, 0, movedBlock)
    onChange(newBlocks)
  }

  const getDefaultBlockData = (type: string) => {
    switch (type) {
      case 'paragraph':
        return { text: '' }
      case 'heading':
        return { text: '', level: 1 }
      case 'image':
        return { url: '', alt: '', caption: '' }
      case 'code':
        return { code: '', language: 'javascript' }
      default:
        return {}
    }
  }

  const renderBlock = (block: Block, index: number) => {
    const blockType = BLOCK_TYPES.find(bt => bt.type === block.type)
    
    return (
      <div key={block.id} className="group relative border border-gray-200 rounded-md">
        {/* 块工具栏 */}
        {!disabled && (
          <div className="absolute -left-10 top-2 opacity-0 group-hover:opacity-100 transition-opacity">
            <div className="flex flex-col gap-1">
              <button
                type="button"
                className="p-1 text-gray-400 hover:text-gray-600 cursor-grab"
                title="拖拽排序"
              >
                <GripVertical className="h-4 w-4" />
              </button>
              
              <button
                type="button"
                onClick={() => removeBlock(block.id)}
                className="p-1 text-gray-400 hover:text-red-600"
                title="删除块"
              >
                <Trash2 className="h-4 w-4" />
              </button>
            </div>
          </div>
        )}

        {/* 块内容 */}
        <div className="p-4">
          <div className="flex items-center gap-2 mb-3 text-sm text-gray-600">
            {blockType?.icon && <blockType.icon className="h-4 w-4" />}
            <span>{blockType?.label || block.type}</span>
          </div>

          {renderBlockContent(block)}
        </div>
      </div>
    )
  }

  const renderBlockContent = (block: Block) => {
    switch (block.type) {
      case 'paragraph':
        return (
          <Textarea
            value={block.data.text || ''}
            onChange={(e) => updateBlock(block.id, { text: e.target.value })}
            placeholder="输入段落内容..."
            disabled={disabled}
            className="min-h-[100px]"
          />
        )

      case 'heading':
        return (
          <div className="space-y-3">
            <div className="flex gap-2 items-center">
              <label className="text-sm font-medium">级别:</label>
              <select
                value={block.data.level || 1}
                onChange={(e) => updateBlock(block.id, { 
                  ...block.data, 
                  level: parseInt(e.target.value) 
                })}
                disabled={disabled}
                className="px-2 py-1 border border-gray-300 rounded text-sm"
              >
                {[1, 2, 3, 4, 5, 6].map(level => (
                  <option key={level} value={level}>H{level}</option>
                ))}
              </select>
            </div>
            
            <Input
              value={block.data.text || ''}
              onChange={(e) => updateBlock(block.id, { 
                ...block.data, 
                text: e.target.value 
              })}
              placeholder="输入标题..."
              disabled={disabled}
            />
          </div>
        )

      case 'image':
        return (
          <div className="space-y-3">
            <Input
              value={block.data.url || ''}
              onChange={(e) => updateBlock(block.id, { 
                ...block.data, 
                url: e.target.value 
              })}
              placeholder="图片 URL..."
              disabled={disabled}
            />
            
            <Input
              value={block.data.alt || ''}
              onChange={(e) => updateBlock(block.id, { 
                ...block.data, 
                alt: e.target.value 
              })}
              placeholder="替代文本..."
              disabled={disabled}
            />
            
            <Input
              value={block.data.caption || ''}
              onChange={(e) => updateBlock(block.id, { 
                ...block.data, 
                caption: e.target.value 
              })}
              placeholder="图片说明..."
              disabled={disabled}
            />
            
            {block.data.url && (
              <img
                src={block.data.url}
                alt={block.data.alt}
                className="max-w-full h-auto rounded border"
              />
            )}
          </div>
        )

      case 'code':
        return (
          <div className="space-y-3">
            <div className="flex gap-2 items-center">
              <label className="text-sm font-medium">语言:</label>
              <select
                value={block.data.language || 'javascript'}
                onChange={(e) => updateBlock(block.id, { 
                  ...block.data, 
                  language: e.target.value 
                })}
                disabled={disabled}
                className="px-2 py-1 border border-gray-300 rounded text-sm"
              >
                <option value="javascript">JavaScript</option>
                <option value="typescript">TypeScript</option>
                <option value="python">Python</option>
                <option value="rust">Rust</option>
                <option value="sql">SQL</option>
                <option value="html">HTML</option>
                <option value="css">CSS</option>
              </select>
            </div>
            
            <Textarea
              value={block.data.code || ''}
              onChange={(e) => updateBlock(block.id, { 
                ...block.data, 
                code: e.target.value 
              })}
              placeholder="输入代码..."
              disabled={disabled}
              className="font-mono text-sm min-h-[120px]"
            />
          </div>
        )

      default:
        return (
          <div className="p-4 bg-gray-50 rounded text-center text-gray-500">
            未知块类型: {block.type}
          </div>
        )
    }
  }

  return (
    <div className="space-y-4">
      {/* 块列表 */}
      <div className="space-y-4 relative">
        {value.map((block, index) => renderBlock(block, index))}
        
        {value.length === 0 && (
          <div className="text-center py-8 text-gray-500 border-2 border-dashed border-gray-300 rounded-lg">
            暂无内容块，点击下方按钮添加
          </div>
        )}
      </div>

      {/* 添加块按钮 */}
      {!disabled && (
        <div className="relative">
          <Button
            type="button"
            variant="secondary"
            onClick={() => setShowAddMenu(!showAddMenu)}
            className="w-full"
          >
            <Plus className="h-4 w-4 mr-2" />
            添加内容块
          </Button>

          {/* 添加菜单 */}
          {showAddMenu && (
            <>
              <div
                className="fixed inset-0 z-10"
                onClick={() => setShowAddMenu(false)}
              />
              
              <div className="absolute z-20 w-full mt-1 bg-white border border-gray-200 rounded-md shadow-lg">
                {BLOCK_TYPES.map(({ type, label, icon: Icon }) => (
                  <button
                    key={type}
                    type="button"
                    onClick={() => addBlock(type)}
                    className="w-full px-4 py-3 text-left hover:bg-gray-50 flex items-center gap-3 border-b border-gray-100 last:border-b-0"
                  >
                    <Icon className="h-4 w-4 text-gray-500" />
                    <span>{label}</span>
                  </button>
                ))}
              </div>
            </>
          )}
        </div>
      )}

      {/* 错误提示 */}
      {error && (
        <p className="text-sm text-danger-600">{error}</p>
      )}

      {/* 调试信息 */}
      {process.env.NODE_ENV === 'development' && value.length > 0 && (
        <details className="mt-4 p-3 bg-gray-50 rounded text-xs">
          <summary className="cursor-pointer font-medium">块数据 (调试)</summary>
          <pre className="mt-2 overflow-auto">
            {JSON.stringify(value, null, 2)}
          </pre>
        </details>
      )}
    </div>
  )
}

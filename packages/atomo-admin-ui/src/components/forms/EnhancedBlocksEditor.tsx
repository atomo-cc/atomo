/**
 * Enhanced Flow Canvas - 增强版 Atomo 流动画布编辑器
 * 
 * 在原有基础上添加：
 * - 撤销/重做功能
 * - 键盘快捷键
 * - 高级拖拽交互
 * - 组件模板系统
 * - 导入/导出功能
 * - 多选操作
 * - 智能对齐辅助线
 */

import { useState, useRef, useCallback, useEffect, useMemo } from 'react'
import { 
  Undo2, 
  Redo2, 
  Save, 
  Download, 
  Upload,
  Keyboard,
  Layers3,
  Magnet,
  AlignLeft,
  AlignCenter,
  AlignRight,
  MoreHorizontal
} from 'lucide-react'

import { FlowCanvas, FlowNode, NodeConnection, NodeType } from './BlocksEditor'
import { Button } from '../ui/Button'
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from '../ui/DropdownMenu'
import { Tooltip } from '../ui/Tooltip'
import { cn } from '../../lib/utils'

// ==================== 增强功能接口 ====================

interface HistoryState {
  nodes: FlowNode[]
  connections: NodeConnection[]
  timestamp: number
}

interface AlignmentGuide {
  id: string
  type: 'vertical' | 'horizontal'
  position: number
  nodes: string[]
}

interface CanvasTemplate {
  id: string
  name: string
  description: string
  preview: string
  data: {
    nodes: FlowNode[]
    connections: NodeConnection[]
  }
}

interface EnhancedFlowCanvasProps {
  value: {
    nodes: FlowNode[]
    connections: NodeConnection[]
  }
  onChange: (value: { nodes: FlowNode[]; connections: NodeConnection[] }) => void
  disabled?: boolean
  error?: string
  mode?: 'edit' | 'preview'
  enableHistory?: boolean
  enableTemplates?: boolean
  enableKeyboardShortcuts?: boolean
  enableAlignment?: boolean
  maxHistorySize?: number
}

// ==================== 模板库 ====================

const canvasTemplates: CanvasTemplate[] = [
  {
    id: 'contact-form',
    name: '联系表单',
    description: '标准的联系信息收集表单',
    preview: '📝',
    data: {
      nodes: [
        {
          id: 'form-1',
          type: 'form',
          position: { x: 50, y: 50 },
          size: { width: 400, height: 500 },
          data: { label: '联系表单' }
        },
        {
          id: 'name-input',
          type: 'input',
          position: { x: 80, y: 120 },
          size: { width: 200, height: 40 },
          data: { label: '姓名', content: { placeholder: '请输入姓名' } }
        },
        {
          id: 'email-input',
          type: 'input',
          position: { x: 80, y: 180 },
          size: { width: 200, height: 40 },
          data: { label: '邮箱', content: { placeholder: '请输入邮箱' } }
        },
        {
          id: 'message-textarea',
          type: 'textarea',
          position: { x: 80, y: 240 },
          size: { width: 300, height: 120 },
          data: { label: '留言', content: { placeholder: '请输入留言内容' } }
        },
        {
          id: 'submit-button',
          type: 'button',
          position: { x: 80, y: 380 },
          size: { width: 120, height: 40 },
          data: { label: '提交按钮', content: { text: '提交' } }
        }
      ],
      connections: []
    }
  },
  {
    id: 'dashboard-layout',
    name: '仪表板布局',
    description: '标准的数据仪表板布局',
    preview: '📊',
    data: {
      nodes: [
        {
          id: 'header',
          type: 'heading',
          position: { x: 50, y: 20 },
          size: { width: 600, height: 60 },
          data: { label: '页面标题', content: { text: '数据仪表板', level: 1 } }
        },
        {
          id: 'stats-card-1',
          type: 'card',
          position: { x: 50, y: 100 },
          size: { width: 180, height: 120 },
          data: { label: '统计卡片1' }
        },
        {
          id: 'stats-card-2',
          type: 'card',
          position: { x: 250, y: 100 },
          size: { width: 180, height: 120 },
          data: { label: '统计卡片2' }
        },
        {
          id: 'stats-card-3',
          type: 'card',
          position: { x: 450, y: 100 },
          size: { width: 180, height: 120 },
          data: { label: '统计卡片3' }
        },
        {
          id: 'chart-area',
          type: 'chart',
          position: { x: 50, y: 240 },
          size: { width: 400, height: 250 },
          data: { label: '主图表' }
        },
        {
          id: 'data-table',
          type: 'table',
          position: { x: 470, y: 240 },
          size: { width: 350, height: 250 },
          data: { label: '数据表格' }
        }
      ],
      connections: []
    }
  }
]

// ==================== 键盘快捷键配置 ====================

const keyboardShortcuts = [
  { key: 'Ctrl+Z', action: 'undo', description: '撤销' },
  { key: 'Ctrl+Y', action: 'redo', description: '重做' },
  { key: 'Ctrl+S', action: 'save', description: '保存' },
  { key: 'Ctrl+A', action: 'selectAll', description: '全选' },
  { key: 'Delete', action: 'delete', description: '删除' },
  { key: 'Ctrl+C', action: 'copy', description: '复制' },
  { key: 'Ctrl+V', action: 'paste', description: '粘贴' },
  { key: 'Ctrl+D', action: 'duplicate', description: '复制' },
  { key: 'Escape', action: 'deselect', description: '取消选择' }
]

// ==================== 主组件 ====================

export function EnhancedFlowCanvas({
  value = { nodes: [], connections: [] },
  onChange,
  disabled = false,
  error,
  mode = 'edit',
  enableHistory = true,
  enableTemplates = true,
  enableKeyboardShortcuts = true,
  enableAlignment = true,
  maxHistorySize = 50
}: EnhancedFlowCanvasProps) {
  // 历史记录管理
  const [history, setHistory] = useState<HistoryState[]>([{
    nodes: value.nodes,
    connections: value.connections,
    timestamp: Date.now()
  }])
  const [historyIndex, setHistoryIndex] = useState(0)
  
  // 对齐辅助线
  const [alignmentGuides, setAlignmentGuides] = useState<AlignmentGuide[]>([])
  const [showAlignmentGuides, setShowAlignmentGuides] = useState(enableAlignment)
  
  // 剪贴板
  const [clipboard, setClipboard] = useState<FlowNode[]>([])
  
  // 选择状态
  const [selectedNodes, setSelectedNodes] = useState<string[]>([])
  const [multiSelectMode, setMultiSelectMode] = useState(false)

  // ==================== 历史记录操作 ====================

  const addToHistory = useCallback((newState: { nodes: FlowNode[]; connections: NodeConnection[] }) => {
    if (!enableHistory) return

    const newHistoryState: HistoryState = {
      ...newState,
      timestamp: Date.now()
    }

    setHistory(prev => {
      const newHistory = prev.slice(0, historyIndex + 1)
      newHistory.push(newHistoryState)
      
      // 限制历史记录大小
      if (newHistory.length > maxHistorySize) {
        return newHistory.slice(-maxHistorySize)
      }
      
      return newHistory
    })
    
    setHistoryIndex(prev => prev + 1)
  }, [enableHistory, historyIndex, maxHistorySize])

  const undo = useCallback(() => {
    if (historyIndex > 0) {
      const newIndex = historyIndex - 1
      const state = history[newIndex]
      setHistoryIndex(newIndex)
      onChange({ nodes: state.nodes, connections: state.connections })
    }
  }, [history, historyIndex, onChange])

  const redo = useCallback(() => {
    if (historyIndex < history.length - 1) {
      const newIndex = historyIndex + 1
      const state = history[newIndex]
      setHistoryIndex(newIndex)
      onChange({ nodes: state.nodes, connections: state.connections })
    }
  }, [history, historyIndex, onChange])

  // ==================== 对齐功能 ====================

  const calculateAlignmentGuides = useCallback((draggedNodes: FlowNode[], allNodes: FlowNode[]) => {
    if (!showAlignmentGuides) return []

    const guides: AlignmentGuide[] = []
    const staticNodes = allNodes.filter(n => !draggedNodes.some(d => d.id === n.id))

    draggedNodes.forEach(draggedNode => {
      staticNodes.forEach(staticNode => {
        // 垂直对齐（左边缘、中心、右边缘）
        const leftAlign = staticNode.position.x
        const centerAlign = staticNode.position.x + staticNode.size.width / 2
        const rightAlign = staticNode.position.x + staticNode.size.width

        const draggedLeft = draggedNode.position.x
        const draggedCenter = draggedNode.position.x + draggedNode.size.width / 2
        const draggedRight = draggedNode.position.x + draggedNode.size.width

        if (Math.abs(draggedLeft - leftAlign) < 5) {
          guides.push({
            id: `v-left-${staticNode.id}-${draggedNode.id}`,
            type: 'vertical',
            position: leftAlign,
            nodes: [staticNode.id, draggedNode.id]
          })
        }

        if (Math.abs(draggedCenter - centerAlign) < 5) {
          guides.push({
            id: `v-center-${staticNode.id}-${draggedNode.id}`,
            type: 'vertical',
            position: centerAlign,
            nodes: [staticNode.id, draggedNode.id]
          })
        }

        if (Math.abs(draggedRight - rightAlign) < 5) {
          guides.push({
            id: `v-right-${staticNode.id}-${draggedNode.id}`,
            type: 'vertical',
            position: rightAlign,
            nodes: [staticNode.id, draggedNode.id]
          })
        }

        // 水平对齐（顶部、中心、底部）
        const topAlign = staticNode.position.y
        const centerYAlign = staticNode.position.y + staticNode.size.height / 2
        const bottomAlign = staticNode.position.y + staticNode.size.height

        const draggedTop = draggedNode.position.y
        const draggedCenterY = draggedNode.position.y + draggedNode.size.height / 2
        const draggedBottom = draggedNode.position.y + draggedNode.size.height

        if (Math.abs(draggedTop - topAlign) < 5) {
          guides.push({
            id: `h-top-${staticNode.id}-${draggedNode.id}`,
            type: 'horizontal',
            position: topAlign,
            nodes: [staticNode.id, draggedNode.id]
          })
        }

        if (Math.abs(draggedCenterY - centerYAlign) < 5) {
          guides.push({
            id: `h-center-${staticNode.id}-${draggedNode.id}`,
            type: 'horizontal',
            position: centerYAlign,
            nodes: [staticNode.id, draggedNode.id]
          })
        }

        if (Math.abs(draggedBottom - bottomAlign) < 5) {
          guides.push({
            id: `h-bottom-${staticNode.id}-${draggedNode.id}`,
            type: 'horizontal',
            position: bottomAlign,
            nodes: [staticNode.id, draggedNode.id]
          })
        }
      })
    })

    return guides
  }, [showAlignmentGuides])

  // ==================== 批量操作 ====================

  const alignNodes = useCallback((alignment: 'left' | 'center' | 'right' | 'top' | 'middle' | 'bottom') => {
    if (selectedNodes.length < 2) return

    const nodes = value.nodes.filter(n => selectedNodes.includes(n.id))
    const updates: Partial<FlowNode>[] = []

    switch (alignment) {
      case 'left': {
        const leftMost = Math.min(...nodes.map(n => n.position.x))
        updates.push(...nodes.map(n => ({ position: { ...n.position, x: leftMost } })))
        break
      }
      case 'center': {
        const centerX = nodes.reduce((sum, n) => sum + n.position.x + n.size.width / 2, 0) / nodes.length
        updates.push(...nodes.map(n => ({ position: { ...n.position, x: centerX - n.size.width / 2 } })))
        break
      }
      case 'right': {
        const rightMost = Math.max(...nodes.map(n => n.position.x + n.size.width))
        updates.push(...nodes.map(n => ({ position: { ...n.position, x: rightMost - n.size.width } })))
        break
      }
      case 'top': {
        const topMost = Math.min(...nodes.map(n => n.position.y))
        updates.push(...nodes.map(n => ({ position: { ...n.position, y: topMost } })))
        break
      }
      case 'middle': {
        const centerY = nodes.reduce((sum, n) => sum + n.position.y + n.size.height / 2, 0) / nodes.length
        updates.push(...nodes.map(n => ({ position: { ...n.position, y: centerY - n.size.height / 2 } })))
        break
      }
      case 'bottom': {
        const bottomMost = Math.max(...nodes.map(n => n.position.y + n.size.height))
        updates.push(...nodes.map(n => ({ position: { ...n.position, y: bottomMost - n.size.height } })))
        break
      }
    }

    const newState = {
      ...value,
      nodes: value.nodes.map((n, i) => {
        const nodeIndex = nodes.findIndex(selected => selected.id === n.id)
        return nodeIndex >= 0 ? { ...n, ...updates[nodeIndex] } : n
      })
    }

    onChange(newState)
    addToHistory(newState)
  }, [selectedNodes, value, onChange, addToHistory])

  // ==================== 剪贴板操作 ====================

  const copyNodes = useCallback(() => {
    const nodesToCopy = value.nodes.filter(n => selectedNodes.includes(n.id))
    setClipboard(nodesToCopy)
  }, [selectedNodes, value.nodes])

  const pasteNodes = useCallback(() => {
    if (clipboard.length === 0) return

    const newNodes = clipboard.map(node => ({
      ...node,
      id: `node_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`,
      position: {
        x: node.position.x + 20,
        y: node.position.y + 20
      }
    }))

    const newState = {
      ...value,
      nodes: [...value.nodes, ...newNodes]
    }

    onChange(newState)
    addToHistory(newState)
    setSelectedNodes(newNodes.map(n => n.id))
  }, [clipboard, value, onChange, addToHistory])

  // ==================== 模板操作 ====================

  const applyTemplate = useCallback((template: CanvasTemplate) => {
    const newState = {
      nodes: template.data.nodes.map(node => ({
        ...node,
        id: `node_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`
      })),
      connections: template.data.connections
    }
    
    onChange(newState)
    addToHistory(newState)
  }, [onChange, addToHistory])

  // ==================== 键盘快捷键 ====================

  useEffect(() => {
    if (!enableKeyboardShortcuts) return

    const handleKeyDown = (e: KeyboardEvent) => {
      const isCtrl = e.ctrlKey || e.metaKey
      
      // 撤销/重做
      if (isCtrl && e.key === 'z' && !e.shiftKey) {
        e.preventDefault()
        undo()
      } else if (isCtrl && (e.key === 'y' || (e.key === 'z' && e.shiftKey))) {
        e.preventDefault()
        redo()
      }
      
      // 复制/粘贴
      else if (isCtrl && e.key === 'c') {
        e.preventDefault()
        copyNodes()
      } else if (isCtrl && e.key === 'v') {
        e.preventDefault()
        pasteNodes()
      }
      
      // 删除
      else if (e.key === 'Delete' || e.key === 'Backspace') {
        if (selectedNodes.length > 0) {
          e.preventDefault()
          const newState = {
            ...value,
            nodes: value.nodes.filter(n => !selectedNodes.includes(n.id)),
            connections: value.connections.filter(c => 
              !selectedNodes.includes(c.source) && !selectedNodes.includes(c.target)
            )
          }
          onChange(newState)
          addToHistory(newState)
          setSelectedNodes([])
        }
      }
      
      // 全选
      else if (isCtrl && e.key === 'a') {
        e.preventDefault()
        setSelectedNodes(value.nodes.map(n => n.id))
      }
      
      // 取消选择
      else if (e.key === 'Escape') {
        setSelectedNodes([])
      }
    }

    document.addEventListener('keydown', handleKeyDown)
    return () => document.removeEventListener('keydown', handleKeyDown)
  }, [enableKeyboardShortcuts, undo, redo, copyNodes, pasteNodes, selectedNodes, value, onChange, addToHistory])

  // ==================== 导入/导出 ====================

  const exportCanvas = useCallback(() => {
    const data = {
      version: '1.0',
      timestamp: Date.now(),
      canvas: value
    }
    
    const blob = new Blob([JSON.stringify(data, null, 2)], { type: 'application/json' })
    const url = URL.createObjectURL(blob)
    const a = document.createElement('a')
    a.href = url
    a.download = `atomo-canvas-${new Date().toISOString().split('T')[0]}.json`
    document.body.appendChild(a)
    a.click()
    document.body.removeChild(a)
    URL.revokeObjectURL(url)
  }, [value])

  const importCanvas = useCallback((file: File) => {
    const reader = new FileReader()
    reader.onload = (e) => {
      try {
        const data = JSON.parse(e.target?.result as string)
        if (data.canvas) {
          onChange(data.canvas)
          addToHistory(data.canvas)
        }
      } catch (error) {
        console.error('导入失败:', error)
      }
    }
    reader.readAsText(file)
  }, [onChange, addToHistory])

  // ==================== 增强工具栏 ====================

  const canUndo = enableHistory && historyIndex > 0
  const canRedo = enableHistory && historyIndex < history.length - 1

  const enhancedToolbar = (
    <div className="h-14 bg-white border-b border-gray-200 flex items-center justify-between px-4">
      {/* 左侧工具组 */}
      <div className="flex items-center gap-1">
        {/* 历史操作 */}
        {enableHistory && (
          <>
            <Tooltip content="撤销 (Ctrl+Z)">
              <Button
                variant="ghost"
                size="sm"
                onClick={undo}
                disabled={!canUndo || disabled}
              >
                <Undo2 className="h-4 w-4" />
              </Button>
            </Tooltip>
            <Tooltip content="重做 (Ctrl+Y)">
              <Button
                variant="ghost"
                size="sm"
                onClick={redo}
                disabled={!canRedo || disabled}
              >
                <Redo2 className="h-4 w-4" />
              </Button>
            </Tooltip>
            <div className="h-4 w-px bg-gray-300 mx-1" />
          </>
        )}

        {/* 对齐工具 */}
        {selectedNodes.length > 1 && (
          <>
            <DropdownMenu>
              <DropdownMenuTrigger asChild>
                <Button variant="ghost" size="sm">
                  <AlignLeft className="h-4 w-4" />
                </Button>
              </DropdownMenuTrigger>
              <DropdownMenuContent>
                <DropdownMenuItem onClick={() => alignNodes('left')}>
                  <AlignLeft className="h-4 w-4 mr-2" />
                  左对齐
                </DropdownMenuItem>
                <DropdownMenuItem onClick={() => alignNodes('center')}>
                  <AlignCenter className="h-4 w-4 mr-2" />
                  水平居中
                </DropdownMenuItem>
                <DropdownMenuItem onClick={() => alignNodes('right')}>
                  <AlignRight className="h-4 w-4 mr-2" />
                  右对齐
                </DropdownMenuItem>
                <DropdownMenuItem onClick={() => alignNodes('top')}>
                  上对齐
                </DropdownMenuItem>
                <DropdownMenuItem onClick={() => alignNodes('middle')}>
                  垂直居中
                </DropdownMenuItem>
                <DropdownMenuItem onClick={() => alignNodes('bottom')}>
                  下对齐
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenu>
            <div className="h-4 w-px bg-gray-300 mx-1" />
          </>
        )}

        {/* 辅助功能 */}
        <Tooltip content="对齐辅助线">
          <Button
            variant={showAlignmentGuides ? 'secondary' : 'ghost'}
            size="sm"
            onClick={() => setShowAlignmentGuides(!showAlignmentGuides)}
          >
            <Magnet className="h-4 w-4" />
          </Button>
        </Tooltip>
      </div>

      {/* 右侧工具组 */}
      <div className="flex items-center gap-1">
        {/* 模板 */}
        {enableTemplates && (
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button variant="ghost" size="sm">
                <Layers3 className="h-4 w-4" />
                模板
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent className="w-64">
              {canvasTemplates.map(template => (
                <DropdownMenuItem
                  key={template.id}
                  onClick={() => applyTemplate(template)}
                  className="flex items-start gap-3 p-3"
                >
                  <span className="text-2xl">{template.preview}</span>
                  <div>
                    <div className="font-medium">{template.name}</div>
                    <div className="text-sm text-gray-500">{template.description}</div>
                  </div>
                </DropdownMenuItem>
              ))}
            </DropdownMenuContent>
          </DropdownMenu>
        )}

        {/* 导入/导出 */}
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button variant="ghost" size="sm">
              <MoreHorizontal className="h-4 w-4" />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent>
            <DropdownMenuItem onClick={exportCanvas}>
              <Download className="h-4 w-4 mr-2" />
              导出画布
            </DropdownMenuItem>
            <DropdownMenuItem asChild>
              <label className="flex items-center cursor-pointer">
                <Upload className="h-4 w-4 mr-2" />
                导入画布
                <input
                  type="file"
                  accept=".json"
                  className="hidden"
                  onChange={(e) => {
                    const file = e.target.files?.[0]
                    if (file) importCanvas(file)
                  }}
                />
              </label>
            </DropdownMenuItem>
            <DropdownMenuItem>
              <Save className="h-4 w-4 mr-2" />
              保存为模板
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>

        {/* 快捷键帮助 */}
        {enableKeyboardShortcuts && (
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button variant="ghost" size="sm">
                <Keyboard className="h-4 w-4" />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent className="w-48">
              <div className="p-2 font-medium text-sm">键盘快捷键</div>
              {keyboardShortcuts.map(shortcut => (
                <div key={shortcut.key} className="flex justify-between items-center px-2 py-1 text-xs">
                  <span className="text-gray-600">{shortcut.description}</span>
                  <span className="font-mono bg-gray-100 px-1 rounded">{shortcut.key}</span>
                </div>
              ))}
            </DropdownMenuContent>
          </DropdownMenu>
        )}
      </div>
    </div>
  )

  // ==================== 对齐辅助线渲染 ====================

  const alignmentGuidesOverlay = showAlignmentGuides && alignmentGuides.length > 0 && (
    <div className="absolute inset-0 pointer-events-none">
      {alignmentGuides.map(guide => (
        <div
          key={guide.id}
          className={cn(
            "absolute bg-blue-500 opacity-75",
            guide.type === 'vertical' ? 'w-0.5 h-full' : 'w-full h-0.5'
          )}
          style={{
            [guide.type === 'vertical' ? 'left' : 'top']: guide.position,
            [guide.type === 'vertical' ? 'top' : 'left']: 0
          }}
        />
      ))}
    </div>
  )

  // ==================== 增强的 FlowCanvas ====================

  const handleEnhancedChange = useCallback((newValue: { nodes: FlowNode[]; connections: NodeConnection[] }) => {
    onChange(newValue)
    addToHistory(newValue)
  }, [onChange, addToHistory])

  return (
    <div className="flex flex-col h-full">
      {mode === 'edit' && enhancedToolbar}
      
      <div className="flex-1 relative">
        <FlowCanvas
          value={value}
          onChange={handleEnhancedChange}
          disabled={disabled}
          error={error}
          mode={mode}
        />
        
        {alignmentGuidesOverlay}
        
        {/* 状态指示器 */}
        {mode === 'edit' && (
          <div className="absolute bottom-4 left-4 bg-white bg-opacity-90 rounded-lg px-3 py-2 text-sm text-gray-600 shadow-sm">
            <div className="flex items-center gap-4">
              <span>节点: {value.nodes.length}</span>
              <span>已选: {selectedNodes.length}</span>
              {enableHistory && (
                <span>历史: {historyIndex + 1}/{history.length}</span>
              )}
            </div>
          </div>
        )}
      </div>
    </div>
  )
}

export default EnhancedFlowCanvas

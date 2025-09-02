/**
 * Flow Canvas - Atomo 流动画布编辑器
 * 
 * 可视化的拖拽式画布，支持自由布局、组件连接和实时预览
 * 这是 Atomo Admin UI 动态页面构建的核心组件
 */

import { useState, useRef, useCallback, useEffect } from 'react'
import { 
  // 画布操作
  ZoomIn, 
  ZoomOut, 
  Grid3X3,
  Layers,
  
  // UI组件
  Type, 
  Image, 
  Video, 
  Square,
  Circle,
  
  // 表单组件
  FileText,
  CheckSquare,
  ToggleLeft,
  Calendar,
  List,
  
  // 布局组件
  Layout,
  Columns,
  
  // 操作图标
  Trash2,
  Copy,
  Settings,
  Eye,
  MousePointer,
  Hand
} from 'lucide-react'

import { Button } from '../ui/Button'
import { Input } from '../ui/Input'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '../ui/Tabs'
import { cn } from '../../lib/utils'

// ==================== 类型定义 ====================

export interface FlowNode {
  id: string
  type: NodeType
  position: { x: number; y: number }
  size: { width: number; height: number }
  data: NodeData
  style?: React.CSSProperties
  selected?: boolean
  locked?: boolean
}

export interface NodeConnection {
  id: string
  source: string
  target: string
  type?: 'data' | 'event' | 'style'
  animated?: boolean
}

export interface NodeData {
  label?: string
  content?: any
  properties?: Record<string, any>
  events?: Record<string, any>
  styles?: Record<string, any>
}

export type NodeType = 
  // UI 基础组件
  | 'text' | 'heading' | 'button' | 'image' | 'video'
  // 表单组件
  | 'input' | 'textarea' | 'select' | 'checkbox' | 'switch' | 'datepicker'
  // 布局组件
  | 'container' | 'grid' | 'flex' | 'card' | 'tabs'
  // 数据组件
  | 'table' | 'chart' | 'list' | 'form'
  // 自定义组件
  | 'custom'

interface FlowCanvasProps {
  value: {
    nodes: FlowNode[]
    connections: NodeConnection[]
  }
  onChange: (value: { nodes: FlowNode[]; connections: NodeConnection[] }) => void
  disabled?: boolean
  error?: string
  mode?: 'edit' | 'preview'
}

// ==================== 组件库定义 ====================

const nodeLibrary = [
  {
    category: 'UI组件',
    items: [
      { type: 'text', label: '文本', icon: Type, description: '静态文本内容' },
      { type: 'heading', label: '标题', icon: Type, description: '页面标题' },
      { type: 'button', label: '按钮', icon: Square, description: '交互按钮' },
      { type: 'image', label: '图片', icon: Image, description: '图片展示' },
      { type: 'video', label: '视频', icon: Video, description: '视频播放器' },
    ]
  },
  {
    category: '表单组件',
    items: [
      { type: 'input', label: '输入框', icon: FileText, description: '文本输入' },
      { type: 'textarea', label: '多行输入', icon: FileText, description: '多行文本' },
      { type: 'select', label: '下拉选择', icon: List, description: '选项选择' },
      { type: 'checkbox', label: '复选框', icon: CheckSquare, description: '多选' },
      { type: 'switch', label: '开关', icon: ToggleLeft, description: '布尔值' },
      { type: 'datepicker', label: '日期选择', icon: Calendar, description: '日期时间' },
    ]
  },
  {
    category: '布局组件',
    items: [
      { type: 'container', label: '容器', icon: Square, description: '布局容器' },
      { type: 'grid', label: '网格', icon: Grid3X3, description: '网格布局' },
      { type: 'flex', label: '弹性布局', icon: Columns, description: 'Flexbox' },
      { type: 'card', label: '卡片', icon: Layout, description: '内容卡片' },
      { type: 'tabs', label: '标签页', icon: Layers, description: '选项卡' },
    ]
  },
  {
    category: '数据组件',
    items: [
      { type: 'table', label: '表格', icon: Grid3X3, description: '数据表格' },
      { type: 'chart', label: '图表', icon: Circle, description: '数据可视化' },
      { type: 'list', label: '列表', icon: List, description: '数据列表' },
      { type: 'form', label: '表单', icon: FileText, description: '表单容器' },
    ]
  }
]

// ==================== 主组件 ====================

export function FlowCanvas({
  value = { nodes: [], connections: [] },
  onChange,
  disabled = false,
  error,
  mode = 'edit'
}: FlowCanvasProps) {
  const canvasRef = useRef<HTMLDivElement>(null)
  const [canvasState, setCanvasState] = useState({
    zoom: 1,
    pan: { x: 0, y: 0 },
    tool: 'select' as 'select' | 'hand' | 'connect',
    showGrid: true,
    snapToGrid: true,
    gridSize: 20
  })
  
  const [selectedNodes, setSelectedNodes] = useState<string[]>([])
  const [dragState, setDragState] = useState<{
    isDragging: boolean
    startPos: { x: number; y: number }
    offset: { x: number; y: number }
  }>({
    isDragging: false,
    startPos: { x: 0, y: 0 },
    offset: { x: 0, y: 0 }
  })

  // ==================== 画布操作 ====================

  const addNode = useCallback((type: NodeType, position?: { x: number; y: number }) => {
    const newNode: FlowNode = {
      id: `node_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`,
      type,
      position: position || { x: 100, y: 100 },
      size: getDefaultNodeSize(type),
      data: getDefaultNodeData(type)
    }

    onChange({
      ...value,
      nodes: [...value.nodes, newNode]
    })
  }, [value, onChange])

  const updateNode = useCallback((nodeId: string, updates: Partial<FlowNode>) => {
    onChange({
      ...value,
      nodes: value.nodes.map(node =>
        node.id === nodeId ? { ...node, ...updates } : node
      )
    })
  }, [value, onChange])

  const deleteNode = useCallback((nodeId: string) => {
    onChange({
      nodes: value.nodes.filter(node => node.id !== nodeId),
      connections: value.connections.filter(conn => 
        conn.source !== nodeId && conn.target !== nodeId
      )
    })
  }, [value, onChange])

  const duplicateNode = useCallback((nodeId: string) => {
    const node = value.nodes.find(n => n.id === nodeId)
    if (node) {
      const newNode: FlowNode = {
        ...node,
        id: `node_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`,
        position: {
          x: node.position.x + 20,
          y: node.position.y + 20
        }
      }
      onChange({
        ...value,
        nodes: [...value.nodes, newNode]
      })
    }
  }, [value, onChange])

  // ==================== 拖拽处理 ====================

  const handleNodeMouseDown = useCallback((e: React.MouseEvent, nodeId: string) => {
    if (disabled || mode === 'preview') return
    
    e.stopPropagation()
    const rect = canvasRef.current?.getBoundingClientRect()
    if (!rect) return

    setDragState({
      isDragging: true,
      startPos: { x: e.clientX, y: e.clientY },
      offset: { x: 0, y: 0 }
    })

    if (!selectedNodes.includes(nodeId)) {
      setSelectedNodes([nodeId])
    }
  }, [disabled, mode, selectedNodes])

  const handleMouseMove = useCallback((e: MouseEvent) => {
    if (!dragState.isDragging || selectedNodes.length === 0) return

    const deltaX = e.clientX - dragState.startPos.x
    const deltaY = e.clientY - dragState.startPos.y

    selectedNodes.forEach(nodeId => {
      const node = value.nodes.find(n => n.id === nodeId)
      if (node && !node.locked) {
        let newX = node.position.x + deltaX
        let newY = node.position.y + deltaY

        // 网格对齐
        if (canvasState.snapToGrid) {
          newX = Math.round(newX / canvasState.gridSize) * canvasState.gridSize
          newY = Math.round(newY / canvasState.gridSize) * canvasState.gridSize
        }

        updateNode(nodeId, {
          position: { x: newX, y: newY }
        })
      }
    })

    setDragState(prev => ({
      ...prev,
      startPos: { x: e.clientX, y: e.clientY }
    }))
  }, [dragState.isDragging, dragState.startPos, selectedNodes, value.nodes, canvasState.snapToGrid, canvasState.gridSize, updateNode])

  const handleMouseUp = useCallback(() => {
    setDragState({
      isDragging: false,
      startPos: { x: 0, y: 0 },
      offset: { x: 0, y: 0 }
    })
  }, [])

  // 事件监听
  useEffect(() => {
    if (dragState.isDragging) {
      document.addEventListener('mousemove', handleMouseMove)
      document.addEventListener('mouseup', handleMouseUp)
      return () => {
        document.removeEventListener('mousemove', handleMouseMove)
        document.removeEventListener('mouseup', handleMouseUp)
      }
    }
  }, [dragState.isDragging, handleMouseMove, handleMouseUp])

  // ==================== 工具函数 ====================

  const getDefaultNodeSize = (type: NodeType) => {
    switch (type) {
      case 'text': return { width: 200, height: 60 }
      case 'heading': return { width: 300, height: 80 }
      case 'button': return { width: 120, height: 40 }
      case 'image': return { width: 200, height: 150 }
      case 'container': return { width: 300, height: 200 }
      case 'input': return { width: 200, height: 40 }
      case 'table': return { width: 400, height: 300 }
      default: return { width: 150, height: 100 }
    }
  }

  const getDefaultNodeData = (type: NodeType): NodeData => {
    switch (type) {
      case 'text':
        return { 
          label: '文本',
          content: { text: '这是一段文本' },
          properties: { fontSize: 14, color: '#000000' }
        }
      case 'heading':
        return { 
          label: '标题',
          content: { text: '页面标题', level: 2 },
          properties: { fontSize: 24, fontWeight: 'bold' }
        }
      case 'button':
        return { 
          label: '按钮',
          content: { text: '点击按钮' },
          properties: { variant: 'primary', size: 'medium' },
          events: { onClick: 'handleClick' }
        }
      case 'input':
        return { 
          label: '输入框',
          content: { placeholder: '请输入内容' },
          properties: { type: 'text', required: false }
        }
      default:
        return { label: `${type}组件` }
    }
  }

  // ==================== 渲染函数 ====================

  if (mode === 'preview') {
    return <CanvasPreview nodes={value.nodes} />
  }

  return (
    <div className={cn('flex h-[600px] bg-gray-50 rounded-lg overflow-hidden', error && 'border-2 border-red-500')}>
      {/* 组件库面板 */}
      <div className="w-64 bg-white border-r border-gray-200 flex flex-col">
        <div className="p-4 border-b border-gray-200">
          <h3 className="font-medium text-gray-900">组件库</h3>
        </div>
        
        <Tabs defaultValue="components" className="flex-1">
          <TabsList className="grid w-full grid-cols-3">
            <TabsTrigger value="components">组件</TabsTrigger>
            <TabsTrigger value="layers">图层</TabsTrigger>
            <TabsTrigger value="properties">属性</TabsTrigger>
          </TabsList>
          
          <TabsContent value="components" className="flex-1 overflow-auto p-2">
            <ComponentLibrary onAddNode={addNode} disabled={disabled} />
          </TabsContent>
          
          <TabsContent value="layers" className="flex-1 overflow-auto p-2">
            <LayersPanel 
              nodes={value.nodes} 
              selectedNodes={selectedNodes}
              onSelectNode={setSelectedNodes}
              onDeleteNode={deleteNode}
              onDuplicateNode={duplicateNode}
            />
          </TabsContent>
          
          <TabsContent value="properties" className="flex-1 overflow-auto p-2">
            <PropertiesPanel 
              nodes={value.nodes.filter(n => selectedNodes.includes(n.id))}
              onUpdateNode={updateNode}
            />
          </TabsContent>
        </Tabs>
      </div>

      {/* 主画布区域 */}
      <div className="flex-1 flex flex-col">
        {/* 工具栏 */}
        <div className="h-12 bg-white border-b border-gray-200 flex items-center justify-between px-4">
          <div className="flex items-center gap-2">
            <Button
              variant={canvasState.tool === 'select' ? 'primary' : 'ghost'}
              size="sm"
              onClick={() => setCanvasState(prev => ({ ...prev, tool: 'select' }))}
            >
              <MousePointer className="h-4 w-4" />
            </Button>
            <Button
              variant={canvasState.tool === 'hand' ? 'primary' : 'ghost'}
              size="sm"
              onClick={() => setCanvasState(prev => ({ ...prev, tool: 'hand' }))}
            >
              <Hand className="h-4 w-4" />
            </Button>
            <div className="h-4 w-px bg-gray-300" />
            <Button
              variant="ghost"
              size="sm"
              onClick={() => setCanvasState(prev => ({ ...prev, zoom: Math.min(prev.zoom + 0.1, 2) }))}
            >
              <ZoomIn className="h-4 w-4" />
            </Button>
            <span className="text-sm text-gray-600">{Math.round(canvasState.zoom * 100)}%</span>
            <Button
              variant="ghost"
              size="sm"
              onClick={() => setCanvasState(prev => ({ ...prev, zoom: Math.max(prev.zoom - 0.1, 0.1) }))}
            >
              <ZoomOut className="h-4 w-4" />
            </Button>
          </div>
          
          <div className="flex items-center gap-2">
            <Button
              variant={canvasState.showGrid ? 'secondary' : 'ghost'}
              size="sm"
              onClick={() => setCanvasState(prev => ({ ...prev, showGrid: !prev.showGrid }))}
            >
              <Grid3X3 className="h-4 w-4" />
            </Button>
            <Button variant="ghost" size="sm">
              <Eye className="h-4 w-4" />
              预览
            </Button>
          </div>
        </div>

        {/* 画布 */}
        <div 
          ref={canvasRef}
          className="flex-1 relative overflow-hidden cursor-crosshair"
          style={{
            backgroundImage: canvasState.showGrid 
              ? `radial-gradient(circle, #ccc 1px, transparent 1px)`
              : 'none',
            backgroundSize: canvasState.showGrid 
              ? `${canvasState.gridSize}px ${canvasState.gridSize}px`
              : 'auto'
          }}
          onClick={(e) => {
            if (e.target === e.currentTarget) {
              setSelectedNodes([])
            }
          }}
        >
          {/* 渲染节点 */}
          {value.nodes.map(node => (
            <NodeRenderer
              key={node.id}
              node={node}
              selected={selectedNodes.includes(node.id)}
              zoom={canvasState.zoom}
              onMouseDown={(e) => handleNodeMouseDown(e, node.id)}
              disabled={disabled}
            />
          ))}

          {/* 选择框 */}
          {selectedNodes.length > 0 && (
            <SelectionBox 
              nodes={value.nodes.filter(n => selectedNodes.includes(n.id))}
              zoom={canvasState.zoom}
            />
          )}
        </div>
      </div>

      {error && (
        <div className="absolute bottom-4 left-4 bg-red-100 border border-red-400 text-red-700 px-3 py-2 rounded">
          {error}
        </div>
      )}
    </div>
  )
}

// ==================== 子组件 ====================

function ComponentLibrary({ onAddNode, disabled }: { 
  onAddNode: (type: NodeType) => void
  disabled: boolean 
}) {
  return (
    <div className="space-y-4">
      {nodeLibrary.map(category => (
        <div key={category.category}>
          <h4 className="text-sm font-medium text-gray-700 mb-2">{category.category}</h4>
          <div className="grid grid-cols-2 gap-2">
            {category.items.map(item => (
              <Button
                key={item.type}
                variant="ghost"
                size="sm"
                className="h-auto p-2 flex flex-col items-center text-center"
                onClick={() => onAddNode(item.type as NodeType)}
                disabled={disabled}
                title={item.description}
              >
                <item.icon className="h-5 w-5 mb-1" />
                <span className="text-xs">{item.label}</span>
              </Button>
            ))}
          </div>
        </div>
      ))}
    </div>
  )
}

function LayersPanel({ 
  nodes, 
  selectedNodes, 
  onSelectNode, 
  onDeleteNode, 
  onDuplicateNode 
}: {
  nodes: FlowNode[]
  selectedNodes: string[]
  onSelectNode: (nodeIds: string[]) => void
  onDeleteNode: (nodeId: string) => void
  onDuplicateNode: (nodeId: string) => void
}) {
  return (
    <div className="space-y-1">
      {nodes.map(node => (
        <div
          key={node.id}
          className={cn(
            'flex items-center justify-between p-2 rounded text-sm cursor-pointer',
            selectedNodes.includes(node.id) ? 'bg-blue-100' : 'hover:bg-gray-100'
          )}
          onClick={() => onSelectNode([node.id])}
        >
          <span className="flex-1 truncate">{node.data.label || node.type}</span>
          <div className="flex items-center gap-1">
            <Button
              variant="ghost"
              size="sm"
              onClick={(e) => {
                e.stopPropagation()
                onDuplicateNode(node.id)
              }}
            >
              <Copy className="h-3 w-3" />
            </Button>
            <Button
              variant="ghost"
              size="sm"
              onClick={(e) => {
                e.stopPropagation()
                onDeleteNode(node.id)
              }}
            >
              <Trash2 className="h-3 w-3" />
            </Button>
          </div>
        </div>
      ))}
    </div>
  )
}

function PropertiesPanel({ 
  nodes, 
  onUpdateNode 
}: {
  nodes: FlowNode[]
  onUpdateNode: (nodeId: string, updates: Partial<FlowNode>) => void
}) {
  if (nodes.length === 0) {
    return (
      <div className="text-center text-gray-500 py-8">
        <Settings className="h-8 w-8 mx-auto mb-2 opacity-50" />
        <p>选择一个组件查看属性</p>
      </div>
    )
  }

  const node = nodes[0] // 暂时只支持单选

  return (
    <div className="space-y-4">
      <div>
        <label className="text-sm font-medium text-gray-700">标签</label>
        <Input
          value={node.data.label || ''}
          onChange={(e) => onUpdateNode(node.id, {
            data: { ...node.data, label: e.target.value }
          })}
          className="mt-1"
        />
      </div>

      <div>
        <label className="text-sm font-medium text-gray-700">位置</label>
        <div className="grid grid-cols-2 gap-2 mt-1">
          <Input
            type="number"
            value={node.position.x}
            onChange={(e) => onUpdateNode(node.id, {
              position: { ...node.position, x: Number(e.target.value) }
            })}
            placeholder="X"
          />
          <Input
            type="number"
            value={node.position.y}
            onChange={(e) => onUpdateNode(node.id, {
              position: { ...node.position, y: Number(e.target.value) }
            })}
            placeholder="Y"
          />
        </div>
      </div>

      <div>
        <label className="text-sm font-medium text-gray-700">尺寸</label>
        <div className="grid grid-cols-2 gap-2 mt-1">
          <Input
            type="number"
            value={node.size.width}
            onChange={(e) => onUpdateNode(node.id, {
              size: { ...node.size, width: Number(e.target.value) }
            })}
            placeholder="宽度"
          />
          <Input
            type="number"
            value={node.size.height}
            onChange={(e) => onUpdateNode(node.id, {
              size: { ...node.size, height: Number(e.target.value) }
            })}
            placeholder="高度"
          />
        </div>
      </div>
    </div>
  )
}

function NodeRenderer({ 
  node, 
  selected, 
  zoom, 
  onMouseDown, 
  disabled 
}: {
  node: FlowNode
  selected: boolean
  zoom: number
  onMouseDown: (e: React.MouseEvent) => void
  disabled: boolean
}) {
  const getNodeIcon = (type: NodeType) => {
    const iconMap: Record<NodeType, any> = {
      text: Type,
      heading: Type,
      button: Square,
      image: Image,
      video: Video,
      input: FileText,
      textarea: FileText,
      select: List,
      checkbox: CheckSquare,
      switch: ToggleLeft,
      datepicker: Calendar,
      container: Layout,
      grid: Grid3X3,
      flex: Columns,
      card: Layout,
      tabs: Layers,
      table: Grid3X3,
      chart: Circle,
      list: List,
      form: FileText,
      custom: Square
    }
    return iconMap[type] || Square
  }

  const Icon = getNodeIcon(node.type)

  return (
    <div
      className={cn(
        'absolute bg-white border-2 rounded-lg shadow-sm cursor-move transition-all',
        selected ? 'border-blue-500 shadow-lg' : 'border-gray-300',
        disabled && 'opacity-50 cursor-not-allowed'
      )}
      style={{
        left: node.position.x * zoom,
        top: node.position.y * zoom,
        width: node.size.width * zoom,
        height: node.size.height * zoom,
        transform: `scale(${zoom})`,
        transformOrigin: 'top left'
      }}
      onMouseDown={onMouseDown}
    >
      <div className="p-2 h-full flex flex-col">
        <div className="flex items-center gap-2 mb-1">
          <Icon className="h-4 w-4 text-gray-600" />
          <span className="text-sm font-medium text-gray-900 truncate">
            {node.data.label || node.type}
          </span>
        </div>
        
        <div className="flex-1 text-xs text-gray-600">
          {node.data.content?.text || `${node.type}组件`}
        </div>

        {selected && (
          <div className="absolute -top-8 right-0 flex gap-1">
            <Button variant="ghost" size="sm" className="h-6 w-6 p-0">
              <Settings className="h-3 w-3" />
            </Button>
            <Button variant="ghost" size="sm" className="h-6 w-6 p-0">
              <Trash2 className="h-3 w-3" />
            </Button>
          </div>
        )}
      </div>
    </div>
  )
}

function SelectionBox({ nodes, zoom }: { nodes: FlowNode[]; zoom: number }) {
  if (nodes.length === 0) return null

  const bounds = nodes.reduce((acc, node) => ({
    left: Math.min(acc.left, node.position.x),
    top: Math.min(acc.top, node.position.y),
    right: Math.max(acc.right, node.position.x + node.size.width),
    bottom: Math.max(acc.bottom, node.position.y + node.size.height)
  }), {
    left: Infinity,
    top: Infinity,
    right: -Infinity,
    bottom: -Infinity
  })

  return (
    <div
      className="absolute border-2 border-blue-500 bg-blue-500 bg-opacity-10 pointer-events-none"
      style={{
        left: bounds.left * zoom,
        top: bounds.top * zoom,
        width: (bounds.right - bounds.left) * zoom,
        height: (bounds.bottom - bounds.top) * zoom
      }}
    />
  )
}

function CanvasPreview({ nodes }: { nodes: FlowNode[] }) {
  return (
    <div className="w-full h-full bg-white p-4">
      <h3 className="text-lg font-medium mb-4">预览模式</h3>
      <div className="space-y-4">
        {nodes.map(node => (
          <div key={node.id} className="p-4 border rounded-lg">
            <h4 className="font-medium">{node.data.label}</h4>
            <p className="text-sm text-gray-600">{node.data.content?.text}</p>
          </div>
        ))}
      </div>
    </div>
  )
}

// 导出别名保持兼容性
export { FlowCanvas as BlocksEditor }
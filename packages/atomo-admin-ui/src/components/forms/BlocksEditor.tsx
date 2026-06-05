/**
 * Flow Canvas - Atomo flow canvas editor
 *
 * A visual drag-and-drop canvas supporting free-form layout, component connections, and live preview.
 * This is the core component for building dynamic pages in the Atomo Admin UI.
 */

import { useState, useRef, useCallback, useEffect } from 'react'
import {
  // Canvas controls
  ZoomIn,
  ZoomOut,
  Grid3X3,
  Layers,

  // UI components
  Type,
  Image,
  Video,
  Square,
  Circle,

  // Form components
  FileText,
  CheckSquare,
  ToggleLeft,
  Calendar,
  List,

  // Layout components
  Layout,
  Columns,

  // Action icons
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

// ==================== Type definitions ====================

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
  // Basic UI components
  | 'text' | 'heading' | 'button' | 'image' | 'video'
  // Form components
  | 'input' | 'textarea' | 'select' | 'checkbox' | 'switch' | 'datepicker'
  // Layout components
  | 'container' | 'grid' | 'flex' | 'card' | 'tabs'
  // Data components
  | 'table' | 'chart' | 'list' | 'form'
  // Custom components
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

// ==================== Component library definition ====================

const nodeLibrary = [
  {
    category: 'UI Components',
    items: [
      { type: 'text', label: 'Text', icon: Type, description: 'Static text content' },
      { type: 'heading', label: 'Heading', icon: Type, description: 'Page heading' },
      { type: 'button', label: 'Button', icon: Square, description: 'Interactive button' },
      { type: 'image', label: 'Image', icon: Image, description: 'Image display' },
      { type: 'video', label: 'Video', icon: Video, description: 'Video player' },
    ]
  },
  {
    category: 'Form Components',
    items: [
      { type: 'input', label: 'Input', icon: FileText, description: 'Text input' },
      { type: 'textarea', label: 'Multiline Input', icon: FileText, description: 'Multiline text' },
      { type: 'select', label: 'Dropdown', icon: List, description: 'Option selection' },
      { type: 'checkbox', label: 'Checkbox', icon: CheckSquare, description: 'Multiple selection' },
      { type: 'switch', label: 'Switch', icon: ToggleLeft, description: 'Boolean value' },
      { type: 'datepicker', label: 'Date Picker', icon: Calendar, description: 'Date and time' },
    ]
  },
  {
    category: 'Layout Components',
    items: [
      { type: 'container', label: 'Container', icon: Square, description: 'Layout container' },
      { type: 'grid', label: 'Grid', icon: Grid3X3, description: 'Grid layout' },
      { type: 'flex', label: 'Flex Layout', icon: Columns, description: 'Flexbox' },
      { type: 'card', label: 'Card', icon: Layout, description: 'Content card' },
      { type: 'tabs', label: 'Tabs', icon: Layers, description: 'Tabbed panels' },
    ]
  },
  {
    category: 'Data Components',
    items: [
      { type: 'table', label: 'Table', icon: Grid3X3, description: 'Data table' },
      { type: 'chart', label: 'Chart', icon: Circle, description: 'Data visualization' },
      { type: 'list', label: 'List', icon: List, description: 'Data list' },
      { type: 'form', label: 'Form', icon: FileText, description: 'Form container' },
    ]
  }
]

// ==================== Main component ====================

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

  // ==================== Canvas operations ====================

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

  // ==================== Drag handling ====================

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

        // Snap to grid
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

  // Event listeners
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

  // ==================== Utility functions ====================

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
          label: 'Text',
          content: { text: 'This is some text' },
          properties: { fontSize: 14, color: '#000000' }
        }
      case 'heading':
        return {
          label: 'Heading',
          content: { text: 'Page heading', level: 2 },
          properties: { fontSize: 24, fontWeight: 'bold' }
        }
      case 'button':
        return {
          label: 'Button',
          content: { text: 'Click me' },
          properties: { variant: 'primary', size: 'medium' },
          events: { onClick: 'handleClick' }
        }
      case 'input':
        return {
          label: 'Input',
          content: { placeholder: 'Enter a value' },
          properties: { type: 'text', required: false }
        }
      default:
        return { label: `${type} component` }
    }
  }

  // ==================== Render functions ====================

  if (mode === 'preview') {
    return <CanvasPreview nodes={value.nodes} />
  }

  return (
    <div className={cn('flex h-[600px] bg-gray-50 rounded-lg overflow-hidden', error && 'border-2 border-red-500')}>
      {/* Component library panel */}
      <div className="w-64 bg-white border-r border-gray-200 flex flex-col">
        <div className="p-4 border-b border-gray-200">
          <h3 className="font-medium text-gray-900">Component Library</h3>
        </div>

        <Tabs defaultValue="components" className="flex-1">
          <TabsList className="grid w-full grid-cols-3">
            <TabsTrigger value="components">Components</TabsTrigger>
            <TabsTrigger value="layers">Layers</TabsTrigger>
            <TabsTrigger value="properties">Properties</TabsTrigger>
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

      {/* Main canvas area */}
      <div className="flex-1 flex flex-col">
        {/* Toolbar */}
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
              Preview
            </Button>
          </div>
        </div>

        {/* Canvas */}
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
          {/* Render nodes */}
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

          {/* Selection box */}
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

// ==================== Subcomponents ====================

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
        <p>Select a component to view its properties</p>
      </div>
    )
  }

  const node = nodes[0] // Only single selection is supported for now

  return (
    <div className="space-y-4">
      <div>
        <label className="text-sm font-medium text-gray-700">Label</label>
        <Input
          value={node.data.label || ''}
          onChange={(e) => onUpdateNode(node.id, {
            data: { ...node.data, label: e.target.value }
          })}
          className="mt-1"
        />
      </div>

      <div>
        <label className="text-sm font-medium text-gray-700">Position</label>
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
        <label className="text-sm font-medium text-gray-700">Size</label>
        <div className="grid grid-cols-2 gap-2 mt-1">
          <Input
            type="number"
            value={node.size.width}
            onChange={(e) => onUpdateNode(node.id, {
              size: { ...node.size, width: Number(e.target.value) }
            })}
            placeholder="Width"
          />
          <Input
            type="number"
            value={node.size.height}
            onChange={(e) => onUpdateNode(node.id, {
              size: { ...node.size, height: Number(e.target.value) }
            })}
            placeholder="Height"
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
          {node.data.content?.text || `${node.type} component`}
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
      <h3 className="text-lg font-medium mb-4">Preview Mode</h3>
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

// Export alias kept for backward compatibility
export { FlowCanvas as BlocksEditor }
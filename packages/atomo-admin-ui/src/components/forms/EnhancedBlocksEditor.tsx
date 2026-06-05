/**
 * Enhanced Flow Canvas - enhanced Atomo flow canvas editor
 *
 * Adds the following on top of the base editor:
 * - Undo/redo
 * - Keyboard shortcuts
 * - Advanced drag interactions
 * - Component template system
 * - Import/export
 * - Multi-select operations
 * - Smart alignment guides
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

// ==================== Enhanced feature interfaces ====================

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

// ==================== Template library ====================

const canvasTemplates: CanvasTemplate[] = [
  {
    id: 'contact-form',
    name: 'Contact Form',
    description: 'A standard contact-information collection form',
    preview: '📝',
    data: {
      nodes: [
        {
          id: 'form-1',
          type: 'form',
          position: { x: 50, y: 50 },
          size: { width: 400, height: 500 },
          data: { label: 'Contact Form' }
        },
        {
          id: 'name-input',
          type: 'input',
          position: { x: 80, y: 120 },
          size: { width: 200, height: 40 },
          data: { label: 'Name', content: { placeholder: 'Enter your name' } }
        },
        {
          id: 'email-input',
          type: 'input',
          position: { x: 80, y: 180 },
          size: { width: 200, height: 40 },
          data: { label: 'Email', content: { placeholder: 'Enter your email' } }
        },
        {
          id: 'message-textarea',
          type: 'textarea',
          position: { x: 80, y: 240 },
          size: { width: 300, height: 120 },
          data: { label: 'Message', content: { placeholder: 'Enter your message' } }
        },
        {
          id: 'submit-button',
          type: 'button',
          position: { x: 80, y: 380 },
          size: { width: 120, height: 40 },
          data: { label: 'Submit Button', content: { text: 'Submit' } }
        }
      ],
      connections: []
    }
  },
  {
    id: 'dashboard-layout',
    name: 'Dashboard Layout',
    description: 'A standard data dashboard layout',
    preview: '📊',
    data: {
      nodes: [
        {
          id: 'header',
          type: 'heading',
          position: { x: 50, y: 20 },
          size: { width: 600, height: 60 },
          data: { label: 'Page Title', content: { text: 'Data Dashboard', level: 1 } }
        },
        {
          id: 'stats-card-1',
          type: 'card',
          position: { x: 50, y: 100 },
          size: { width: 180, height: 120 },
          data: { label: 'Stat Card 1' }
        },
        {
          id: 'stats-card-2',
          type: 'card',
          position: { x: 250, y: 100 },
          size: { width: 180, height: 120 },
          data: { label: 'Stat Card 2' }
        },
        {
          id: 'stats-card-3',
          type: 'card',
          position: { x: 450, y: 100 },
          size: { width: 180, height: 120 },
          data: { label: 'Stat Card 3' }
        },
        {
          id: 'chart-area',
          type: 'chart',
          position: { x: 50, y: 240 },
          size: { width: 400, height: 250 },
          data: { label: 'Main Chart' }
        },
        {
          id: 'data-table',
          type: 'table',
          position: { x: 470, y: 240 },
          size: { width: 350, height: 250 },
          data: { label: 'Data Table' }
        }
      ],
      connections: []
    }
  }
]

// ==================== Keyboard shortcut configuration ====================

const keyboardShortcuts = [
  { key: 'Ctrl+Z', action: 'undo', description: 'Undo' },
  { key: 'Ctrl+Y', action: 'redo', description: 'Redo' },
  { key: 'Ctrl+S', action: 'save', description: 'Save' },
  { key: 'Ctrl+A', action: 'selectAll', description: 'Select all' },
  { key: 'Delete', action: 'delete', description: 'Delete' },
  { key: 'Ctrl+C', action: 'copy', description: 'Copy' },
  { key: 'Ctrl+V', action: 'paste', description: 'Paste' },
  { key: 'Ctrl+D', action: 'duplicate', description: 'Duplicate' },
  { key: 'Escape', action: 'deselect', description: 'Deselect' }
]

// ==================== Main component ====================

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
  // History management
  const [history, setHistory] = useState<HistoryState[]>([{
    nodes: value.nodes,
    connections: value.connections,
    timestamp: Date.now()
  }])
  const [historyIndex, setHistoryIndex] = useState(0)

  // Alignment guides
  const [alignmentGuides, setAlignmentGuides] = useState<AlignmentGuide[]>([])
  const [showAlignmentGuides, setShowAlignmentGuides] = useState(enableAlignment)

  // Clipboard
  const [clipboard, setClipboard] = useState<FlowNode[]>([])

  // Selection state
  const [selectedNodes, setSelectedNodes] = useState<string[]>([])
  const [multiSelectMode, setMultiSelectMode] = useState(false)

  // ==================== History operations ====================

  const addToHistory = useCallback((newState: { nodes: FlowNode[]; connections: NodeConnection[] }) => {
    if (!enableHistory) return

    const newHistoryState: HistoryState = {
      ...newState,
      timestamp: Date.now()
    }

    setHistory(prev => {
      const newHistory = prev.slice(0, historyIndex + 1)
      newHistory.push(newHistoryState)
      
      // Limit the history size
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

  // ==================== Alignment features ====================

  const calculateAlignmentGuides = useCallback((draggedNodes: FlowNode[], allNodes: FlowNode[]) => {
    if (!showAlignmentGuides) return []

    const guides: AlignmentGuide[] = []
    const staticNodes = allNodes.filter(n => !draggedNodes.some(d => d.id === n.id))

    draggedNodes.forEach(draggedNode => {
      staticNodes.forEach(staticNode => {
        // Vertical alignment (left edge, center, right edge)
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

        // Horizontal alignment (top, center, bottom)
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

  // ==================== Bulk operations ====================

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

  // ==================== Clipboard operations ====================

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

  // ==================== Template operations ====================

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

  // ==================== Keyboard shortcuts ====================

  useEffect(() => {
    if (!enableKeyboardShortcuts) return

    const handleKeyDown = (e: KeyboardEvent) => {
      const isCtrl = e.ctrlKey || e.metaKey

      // Undo/redo
      if (isCtrl && e.key === 'z' && !e.shiftKey) {
        e.preventDefault()
        undo()
      } else if (isCtrl && (e.key === 'y' || (e.key === 'z' && e.shiftKey))) {
        e.preventDefault()
        redo()
      }
      
      // Copy/paste
      else if (isCtrl && e.key === 'c') {
        e.preventDefault()
        copyNodes()
      } else if (isCtrl && e.key === 'v') {
        e.preventDefault()
        pasteNodes()
      }
      
      // Delete
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
      
      // Select all
      else if (isCtrl && e.key === 'a') {
        e.preventDefault()
        setSelectedNodes(value.nodes.map(n => n.id))
      }

      // Deselect
      else if (e.key === 'Escape') {
        setSelectedNodes([])
      }
    }

    document.addEventListener('keydown', handleKeyDown)
    return () => document.removeEventListener('keydown', handleKeyDown)
  }, [enableKeyboardShortcuts, undo, redo, copyNodes, pasteNodes, selectedNodes, value, onChange, addToHistory])

  // ==================== Import/export ====================

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
        console.error('Import failed:', error)
      }
    }
    reader.readAsText(file)
  }, [onChange, addToHistory])

  // ==================== Enhanced toolbar ====================

  const canUndo = enableHistory && historyIndex > 0
  const canRedo = enableHistory && historyIndex < history.length - 1

  const enhancedToolbar = (
    <div className="h-14 bg-white border-b border-gray-200 flex items-center justify-between px-4">
      {/* Left tool group */}
      <div className="flex items-center gap-1">
        {/* History operations */}
        {enableHistory && (
          <>
            <Tooltip content="Undo (Ctrl+Z)">
              <Button
                variant="ghost"
                size="sm"
                onClick={undo}
                disabled={!canUndo || disabled}
              >
                <Undo2 className="h-4 w-4" />
              </Button>
            </Tooltip>
            <Tooltip content="Redo (Ctrl+Y)">
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

        {/* Alignment tools */}
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
                  Align left
                </DropdownMenuItem>
                <DropdownMenuItem onClick={() => alignNodes('center')}>
                  <AlignCenter className="h-4 w-4 mr-2" />
                  Center horizontally
                </DropdownMenuItem>
                <DropdownMenuItem onClick={() => alignNodes('right')}>
                  <AlignRight className="h-4 w-4 mr-2" />
                  Align right
                </DropdownMenuItem>
                <DropdownMenuItem onClick={() => alignNodes('top')}>
                  Align top
                </DropdownMenuItem>
                <DropdownMenuItem onClick={() => alignNodes('middle')}>
                  Center vertically
                </DropdownMenuItem>
                <DropdownMenuItem onClick={() => alignNodes('bottom')}>
                  Align bottom
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenu>
            <div className="h-4 w-px bg-gray-300 mx-1" />
          </>
        )}

        {/* Assistive features */}
        <Tooltip content="Alignment guides">
          <Button
            variant={showAlignmentGuides ? 'secondary' : 'ghost'}
            size="sm"
            onClick={() => setShowAlignmentGuides(!showAlignmentGuides)}
          >
            <Magnet className="h-4 w-4" />
          </Button>
        </Tooltip>
      </div>

      {/* Right tool group */}
      <div className="flex items-center gap-1">
        {/* Templates */}
        {enableTemplates && (
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button variant="ghost" size="sm">
                <Layers3 className="h-4 w-4" />
                Templates
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

        {/* Import/export */}
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button variant="ghost" size="sm">
              <MoreHorizontal className="h-4 w-4" />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent>
            <DropdownMenuItem onClick={exportCanvas}>
              <Download className="h-4 w-4 mr-2" />
              Export canvas
            </DropdownMenuItem>
            <DropdownMenuItem asChild>
              <label className="flex items-center cursor-pointer">
                <Upload className="h-4 w-4 mr-2" />
                Import canvas
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
              Save as template
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>

        {/* Keyboard shortcut help */}
        {enableKeyboardShortcuts && (
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button variant="ghost" size="sm">
                <Keyboard className="h-4 w-4" />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent className="w-48">
              <div className="p-2 font-medium text-sm">Keyboard Shortcuts</div>
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

  // ==================== Alignment guide rendering ====================

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

  // ==================== Enhanced FlowCanvas ====================

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
        
        {/* Status indicator */}
        {mode === 'edit' && (
          <div className="absolute bottom-4 left-4 bg-white bg-opacity-90 rounded-lg px-3 py-2 text-sm text-gray-600 shadow-sm">
            <div className="flex items-center gap-4">
              <span>Nodes: {value.nodes.length}</span>
              <span>Selected: {selectedNodes.length}</span>
              {enableHistory && (
                <span>History: {historyIndex + 1}/{history.length}</span>
              )}
            </div>
          </div>
        )}
      </div>
    </div>
  )
}

export default EnhancedFlowCanvas

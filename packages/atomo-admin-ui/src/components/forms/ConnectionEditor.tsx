/**
 * Connection Editor - connection line editor
 *
 * Used to create and manage connections between components:
 * - Data-flow connections
 * - Event-trigger connections
 * - Style-inheritance connections
 * - Visual connection lines
 */

import { useState, useRef, useCallback, useEffect } from 'react'
import { FlowNode, NodeConnection } from './BlocksEditor'
import { 
  ArrowRight, 
  Zap, 
  Palette, 
  Database,
  Settings,
  Trash2,
  Plus,
  Circle
} from 'lucide-react'
import { Button } from '../ui/Button'
import { Select } from '../ui/Select'
import { cn } from '../../lib/utils'

// ==================== Type definitions ====================

export interface ConnectionPoint {
  nodeId: string
  side: 'top' | 'right' | 'bottom' | 'left'
  index: number
  type: 'input' | 'output' | 'both'
  dataType?: 'string' | 'number' | 'boolean' | 'object' | 'array' | 'function'
}

export interface ConnectionPath {
  start: { x: number; y: number }
  end: { x: number; y: number }
  controlPoints: Array<{ x: number; y: number }>
}

export interface ActiveConnection {
  id: string
  source: ConnectionPoint
  target: ConnectionPoint
  type: 'data' | 'event' | 'style'
  path: ConnectionPath
  animated: boolean
  color: string
}

interface ConnectionEditorProps {
  nodes: FlowNode[]
  connections: NodeConnection[]
  onConnectionsChange: (connections: NodeConnection[]) => void
  disabled?: boolean
  zoom?: number
  mode?: 'connect' | 'select'
}

// ==================== Connection type configuration ====================

const connectionTypes = {
  data: {
    label: 'Data Flow',
    icon: Database,
    color: '#3b82f6',
    description: 'Passes data values'
  },
  event: {
    label: 'Event Trigger',
    icon: Zap,
    color: '#f59e0b',
    description: 'Triggers an action or event'
  },
  style: {
    label: 'Style Inheritance',
    icon: Palette,
    color: '#8b5cf6',
    description: 'Shares style properties'
  }
}

// ==================== Main component ====================

export function ConnectionEditor({
  nodes,
  connections,
  onConnectionsChange,
  disabled = false,
  zoom = 1,
  mode = 'select'
}: ConnectionEditorProps) {
  const svgRef = useRef<SVGSVGElement>(null)
  const [dragState, setDragState] = useState<{
    isDragging: boolean
    sourcePoint: ConnectionPoint | null
    currentPath: ConnectionPath | null
  }>({
    isDragging: false,
    sourcePoint: null,
    currentPath: null
  })

  const [selectedConnection, setSelectedConnection] = useState<string | null>(null)
  const [connectionType, setConnectionType] = useState<'data' | 'event' | 'style'>('data')

  // ==================== Connection point calculation ====================

  const getConnectionPoints = useCallback((node: FlowNode): ConnectionPoint[] => {
    const points: ConnectionPoint[] = []
    const { position, size } = node

    // Determine connection points based on the node type
    const getNodeConnectionConfig = (nodeType: string) => {
      switch (nodeType) {
        case 'input':
        case 'textarea':
        case 'select':
          return {
            inputs: ['top'],
            outputs: ['right', 'bottom']
          }
        case 'button':
          return {
            inputs: ['top', 'left'],
            outputs: ['right', 'bottom']
          }
        case 'form':
          return {
            inputs: ['top'],
            outputs: ['right', 'bottom', 'left']
          }
        case 'table':
        case 'chart':
          return {
            inputs: ['top', 'left'],
            outputs: ['right']
          }
        default:
          return {
            inputs: ['top', 'left'],
            outputs: ['right', 'bottom']
          }
      }
    }

    const config = getNodeConnectionConfig(node.type)

    // Add input points
    config.inputs.forEach((side, index) => {
      points.push({
        nodeId: node.id,
        side: side as 'top' | 'right' | 'bottom' | 'left',
        index,
        type: 'input'
      })
    })

    // Add output points
    config.outputs.forEach((side, index) => {
      points.push({
        nodeId: node.id,
        side: side as 'top' | 'right' | 'bottom' | 'left',
        index,
        type: 'output'
      })
    })

    return points
  }, [])

  // Get the screen coordinates of a connection point
  const getConnectionPointPosition = useCallback((node: FlowNode, point: ConnectionPoint) => {
    const { position, size } = node
    const padding = 8

    switch (point.side) {
      case 'top':
        return {
          x: position.x + (size.width / 2) + (point.index * 20 - 10),
          y: position.y - padding
        }
      case 'right':
        return {
          x: position.x + size.width + padding,
          y: position.y + (size.height / 2) + (point.index * 20 - 10)
        }
      case 'bottom':
        return {
          x: position.x + (size.width / 2) + (point.index * 20 - 10),
          y: position.y + size.height + padding
        }
      case 'left':
        return {
          x: position.x - padding,
          y: position.y + (size.height / 2) + (point.index * 20 - 10)
        }
      default:
        return { x: position.x, y: position.y }
    }
  }, [])

  // ==================== Path calculation ====================

  const calculateConnectionPath = useCallback((
    start: { x: number; y: number },
    end: { x: number; y: number },
    startSide: string,
    endSide: string
  ): ConnectionPath => {
    const dx = end.x - start.x
    const dy = end.y - start.y
    const distance = Math.sqrt(dx * dx + dy * dy)
    
    // Calculate the control points
    const controlOffset = Math.min(distance * 0.3, 100)
    
    let control1: { x: number; y: number }
    let control2: { x: number; y: number }

    // Adjust the control points based on the connection direction
    switch (startSide) {
      case 'right':
        control1 = { x: start.x + controlOffset, y: start.y }
        break
      case 'left':
        control1 = { x: start.x - controlOffset, y: start.y }
        break
      case 'bottom':
        control1 = { x: start.x, y: start.y + controlOffset }
        break
      case 'top':
        control1 = { x: start.x, y: start.y - controlOffset }
        break
      default:
        control1 = { x: start.x + controlOffset, y: start.y }
    }

    switch (endSide) {
      case 'left':
        control2 = { x: end.x - controlOffset, y: end.y }
        break
      case 'right':
        control2 = { x: end.x + controlOffset, y: end.y }
        break
      case 'top':
        control2 = { x: end.x, y: end.y - controlOffset }
        break
      case 'bottom':
        control2 = { x: end.x, y: end.y + controlOffset }
        break
      default:
        control2 = { x: end.x - controlOffset, y: end.y }
    }

    return {
      start,
      end,
      controlPoints: [control1, control2]
    }
  }, [])

  // ==================== Drag handling ====================

  const handleConnectionStart = useCallback((point: ConnectionPoint, position: { x: number; y: number }) => {
    if (disabled || mode !== 'connect') return

    setDragState({
      isDragging: true,
      sourcePoint: point,
      currentPath: {
        start: position,
        end: position,
        controlPoints: [position, position]
      }
    })
  }, [disabled, mode])

  const handleConnectionDrag = useCallback((position: { x: number; y: number }) => {
    if (!dragState.isDragging || !dragState.sourcePoint) return

    const startPos = dragState.currentPath!.start
    const path = calculateConnectionPath(
      startPos,
      position,
      dragState.sourcePoint.side,
      'left' // Default target direction
    )

    setDragState(prev => ({
      ...prev,
      currentPath: path
    }))
  }, [dragState, calculateConnectionPath])

  const handleConnectionEnd = useCallback((targetPoint?: ConnectionPoint) => {
    if (!dragState.isDragging || !dragState.sourcePoint) return

    if (targetPoint && targetPoint.nodeId !== dragState.sourcePoint.nodeId) {
      // Create a new connection
      const newConnection: NodeConnection = {
        id: `conn_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`,
        source: dragState.sourcePoint.nodeId,
        target: targetPoint.nodeId,
        type: connectionType,
        animated: connectionType === 'event'
      }

      onConnectionsChange([...connections, newConnection])
    }

    setDragState({
      isDragging: false,
      sourcePoint: null,
      currentPath: null
    })
  }, [dragState, connectionType, connections, onConnectionsChange])

  // ==================== Connection deletion ====================

  const deleteConnection = useCallback((connectionId: string) => {
    onConnectionsChange(connections.filter(conn => conn.id !== connectionId))
    setSelectedConnection(null)
  }, [connections, onConnectionsChange])

  // ==================== Render connection lines ====================

  const renderConnection = useCallback((connection: NodeConnection) => {
    const sourceNode = nodes.find(n => n.id === connection.source)
    const targetNode = nodes.find(n => n.id === connection.target)

    if (!sourceNode || !targetNode) return null

    const sourcePoints = getConnectionPoints(sourceNode)
    const targetPoints = getConnectionPoints(targetNode)

    // Simplified: connect from the node's right side to the left side
    const sourcePos = getConnectionPointPosition(sourceNode, {
      nodeId: sourceNode.id,
      side: 'right',
      index: 0,
      type: 'output'
    })
    const targetPos = getConnectionPointPosition(targetNode, {
      nodeId: targetNode.id,
      side: 'left',
      index: 0,
      type: 'input'
    })

    const path = calculateConnectionPath(sourcePos, targetPos, 'right', 'left')
    const typeConfig = connectionTypes[connection.type || 'data']

    const pathD = `M ${path.start.x} ${path.start.y} C ${path.controlPoints[0].x} ${path.controlPoints[0].y}, ${path.controlPoints[1].x} ${path.controlPoints[1].y}, ${path.end.x} ${path.end.y}`

    return (
      <g key={connection.id}>
        {/* Connection line */}
        <path
          d={pathD}
          stroke={typeConfig.color}
          strokeWidth={selectedConnection === connection.id ? 3 : 2}
          fill="none"
          className={cn(
            "cursor-pointer transition-all",
            connection.animated && "animate-pulse"
          )}
          onClick={() => setSelectedConnection(connection.id)}
        />

        {/* Arrow */}
        <defs>
          <marker
            id={`arrow-${connection.id}`}
            markerWidth="10"
            markerHeight="10"
            refX="9"
            refY="3"
            orient="auto"
          >
            <polygon
              points="0 0, 10 3, 0 6"
              fill={typeConfig.color}
            />
          </marker>
        </defs>
        <path
          d={pathD}
          stroke="transparent"
          strokeWidth={2}
          fill="none"
          markerEnd={`url(#arrow-${connection.id})`}
        />

        {/* Connection type icon */}
        <foreignObject
          x={path.controlPoints[0].x - 10}
          y={path.controlPoints[0].y - 10}
          width="20"
          height="20"
        >
          <div className="w-5 h-5 bg-white rounded-full border-2 flex items-center justify-center shadow-sm"
               style={{ borderColor: typeConfig.color }}>
            <typeConfig.icon className="w-3 h-3" style={{ color: typeConfig.color }} />
          </div>
        </foreignObject>
      </g>
    )
  }, [nodes, getConnectionPoints, getConnectionPointPosition, calculateConnectionPath, selectedConnection])

  // ==================== Connection point rendering ====================

  const renderConnectionPoints = useCallback((node: FlowNode) => {
    if (mode !== 'connect') return null

    const points = getConnectionPoints(node)

    return points.map((point, index) => {
      const position = getConnectionPointPosition(node, point)
      const isOutput = point.type === 'output'

      return (
        <Circle
          key={`${node.id}-${point.side}-${index}`}
          className={cn(
            "cursor-pointer transition-all",
            isOutput ? "fill-green-500 stroke-green-600" : "fill-blue-500 stroke-blue-600"
          )}
          r="4"
          cx={position.x}
          cy={position.y}
          strokeWidth="2"
          onMouseDown={(e) => {
            e.stopPropagation()
            handleConnectionStart(point, position)
          }}
        />
      )
    })
  }, [mode, getConnectionPoints, getConnectionPointPosition, handleConnectionStart])

  // ==================== Mouse events ====================

  useEffect(() => {
    const handleMouseMove = (e: MouseEvent) => {
      if (dragState.isDragging) {
        const rect = svgRef.current?.getBoundingClientRect()
        if (rect) {
          const position = {
            x: (e.clientX - rect.left) / zoom,
            y: (e.clientY - rect.top) / zoom
          }
          handleConnectionDrag(position)
        }
      }
    }

    const handleMouseUp = () => {
      if (dragState.isDragging) {
        handleConnectionEnd()
      }
    }

    if (dragState.isDragging) {
      document.addEventListener('mousemove', handleMouseMove)
      document.addEventListener('mouseup', handleMouseUp)
    }

    return () => {
      document.removeEventListener('mousemove', handleMouseMove)
      document.removeEventListener('mouseup', handleMouseUp)
    }
  }, [dragState.isDragging, zoom, handleConnectionDrag, handleConnectionEnd])

  // ==================== Render ====================

  return (
    <div className="absolute inset-0 pointer-events-none">
      {/* Connection line SVG */}
      <svg
        ref={svgRef}
        className="absolute inset-0 w-full h-full pointer-events-auto"
        style={{ zIndex: 10 }}
      >
        {/* Render existing connections */}
        {connections.map(renderConnection)}

        {/* Render the connection line being dragged */}
        {dragState.isDragging && dragState.currentPath && (
          <path
            d={`M ${dragState.currentPath.start.x} ${dragState.currentPath.start.y} C ${dragState.currentPath.controlPoints[0].x} ${dragState.currentPath.controlPoints[0].y}, ${dragState.currentPath.controlPoints[1].x} ${dragState.currentPath.controlPoints[1].y}, ${dragState.currentPath.end.x} ${dragState.currentPath.end.y}`}
            stroke={connectionTypes[connectionType].color}
            strokeWidth="2"
            strokeDasharray="5,5"
            fill="none"
            opacity="0.7"
          />
        )}

        {/* Render connection points */}
        {nodes.map(renderConnectionPoints)}
      </svg>

      {/* Connection toolbar */}
      {mode === 'connect' && (
        <div className="absolute top-4 left-4 bg-white rounded-lg shadow-lg border p-3 pointer-events-auto">
          <div className="text-sm font-medium mb-2">Connection Mode</div>
          <div className="space-y-2">
            {Object.entries(connectionTypes).map(([key, config]) => (
              <Button
                key={key}
                variant={connectionType === key ? 'primary' : 'ghost'}
                size="sm"
                onClick={() => setConnectionType(key as any)}
                className="w-full justify-start"
              >
                <config.icon className="w-4 h-4 mr-2" />
                {config.label}
              </Button>
            ))}
          </div>
        </div>
      )}

      {/* Connection properties panel */}
      {selectedConnection && (
        <div className="absolute top-4 right-4 bg-white rounded-lg shadow-lg border p-3 pointer-events-auto w-64">
          <div className="flex items-center justify-between mb-3">
            <div className="text-sm font-medium">Connection Properties</div>
            <Button
              variant="ghost"
              size="sm"
              onClick={() => deleteConnection(selectedConnection)}
            >
              <Trash2 className="w-4 h-4" />
            </Button>
          </div>

          <div className="space-y-3">
            <div>
              <label className="text-xs text-gray-600">Connection Type</label>
              <Select
                value={connections.find(c => c.id === selectedConnection)?.type || 'data'}
                onValueChange={(value) => {
                  const updatedConnections = connections.map(conn =>
                    conn.id === selectedConnection
                      ? { ...conn, type: value as any }
                      : conn
                  )
                  onConnectionsChange(updatedConnections)
                }}
              >
                {Object.entries(connectionTypes).map(([key, config]) => (
                  <option key={key} value={key}>
                    {config.label}
                  </option>
                ))}
              </Select>
            </div>

            <div className="text-xs text-gray-500">
              {connectionTypes[connections.find(c => c.id === selectedConnection)?.type || 'data'].description}
            </div>
          </div>
        </div>
      )}
    </div>
  )
}

export default ConnectionEditor

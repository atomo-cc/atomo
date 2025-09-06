/**
 * Drag & Drop Helpers - 拖拽辅助组件
 * 
 * 提供高级拖拽功能：
 * - 智能吸附
 * - 碰撞检测
 * - 拖拽预览
 * - 多选拖拽
 * - 区域选择
 */

import { useState, useRef, useCallback, useEffect } from 'react'
import { FlowNode } from './BlocksEditor'
import { cn } from '../../lib/utils'

// ==================== 类型定义 ====================

export interface DragState {
  isDragging: boolean
  draggedNodes: string[]
  startPosition: { x: number; y: number }
  currentPosition: { x: number; y: number }
  offset: { x: number; y: number }
  dragType: 'move' | 'resize' | 'select'
}

export interface SnapPoint {
  x: number
  y: number
  type: 'grid' | 'node' | 'guide'
  nodeId?: string
}

export interface CollisionInfo {
  hasCollision: boolean
  collidingNodes: string[]
}

export interface DragDropConfig {
  snapToGrid: boolean
  gridSize: number
  snapDistance: number
  enableCollisionDetection: boolean
  allowOverlap: boolean
  magneticSnap: boolean
}

// ==================== 拖拽管理器 ====================

export class DragDropManager {
  private nodes: FlowNode[] = []
  private config: DragDropConfig
  private snapPoints: SnapPoint[] = []

  constructor(config: DragDropConfig) {
    this.config = config
  }

  updateNodes(nodes: FlowNode[]) {
    this.nodes = nodes
    this.calculateSnapPoints()
  }

  updateConfig(config: Partial<DragDropConfig>) {
    this.config = { ...this.config, ...config }
  }

  // 计算吸附点
  private calculateSnapPoints() {
    this.snapPoints = []

    // 网格吸附点
    if (this.config.snapToGrid) {
      // 网格点会在拖拽时动态计算
    }

    // 节点边缘吸附点
    this.nodes.forEach(node => {
      // 节点的关键位置点
      const points = [
        { x: node.position.x, y: node.position.y }, // 左上
        { x: node.position.x + node.size.width, y: node.position.y }, // 右上
        { x: node.position.x, y: node.position.y + node.size.height }, // 左下
        { x: node.position.x + node.size.width, y: node.position.y + node.size.height }, // 右下
        { x: node.position.x + node.size.width / 2, y: node.position.y }, // 顶部中心
        { x: node.position.x + node.size.width / 2, y: node.position.y + node.size.height }, // 底部中心
        { x: node.position.x, y: node.position.y + node.size.height / 2 }, // 左侧中心
        { x: node.position.x + node.size.width, y: node.position.y + node.size.height / 2 }, // 右侧中心
        { x: node.position.x + node.size.width / 2, y: node.position.y + node.size.height / 2 } // 中心
      ]

      points.forEach(point => {
        this.snapPoints.push({
          ...point,
          type: 'node',
          nodeId: node.id
        })
      })
    })
  }

  // 计算最佳吸附位置
  calculateSnap(position: { x: number; y: number }, excludeNodes: string[] = []): { x: number; y: number } {
    let snappedX = position.x
    let snappedY = position.y

    // 网格吸附
    if (this.config.snapToGrid) {
      snappedX = Math.round(position.x / this.config.gridSize) * this.config.gridSize
      snappedY = Math.round(position.y / this.config.gridSize) * this.config.gridSize
    }

    // 磁性吸附到其他节点
    if (this.config.magneticSnap) {
      const availableSnapPoints = this.snapPoints.filter(point => 
        !excludeNodes.includes(point.nodeId || '')
      )

      let closestXSnap = snappedX
      let closestYSnap = snappedY
      let minXDistance = this.config.snapDistance
      let minYDistance = this.config.snapDistance

      availableSnapPoints.forEach(point => {
        const xDistance = Math.abs(position.x - point.x)
        const yDistance = Math.abs(position.y - point.y)

        if (xDistance < minXDistance) {
          closestXSnap = point.x
          minXDistance = xDistance
        }

        if (yDistance < minYDistance) {
          closestYSnap = point.y
          minYDistance = yDistance
        }
      })

      snappedX = closestXSnap
      snappedY = closestYSnap
    }

    return { x: snappedX, y: snappedY }
  }

  // 碰撞检测
  checkCollision(node: FlowNode, excludeNodes: string[] = []): CollisionInfo {
    if (!this.config.enableCollisionDetection) {
      return { hasCollision: false, collidingNodes: [] }
    }

    const collidingNodes: string[] = []

    this.nodes.forEach(otherNode => {
      if (otherNode.id === node.id || excludeNodes.includes(otherNode.id)) {
        return
      }

      // AABB 碰撞检测
      const isColliding = !(
        node.position.x >= otherNode.position.x + otherNode.size.width ||
        node.position.x + node.size.width <= otherNode.position.x ||
        node.position.y >= otherNode.position.y + otherNode.size.height ||
        node.position.y + node.size.height <= otherNode.position.y
      )

      if (isColliding) {
        collidingNodes.push(otherNode.id)
      }
    })

    return {
      hasCollision: collidingNodes.length > 0,
      collidingNodes
    }
  }

  // 获取建议的无碰撞位置
  getSafePosition(node: FlowNode, preferredPosition: { x: number; y: number }): { x: number; y: number } {
    if (this.config.allowOverlap) {
      return preferredPosition
    }

    const testNode = { ...node, position: preferredPosition }
    const collision = this.checkCollision(testNode)

    if (!collision.hasCollision) {
      return preferredPosition
    }

    // 尝试在附近找到安全位置
    const searchRadius = 50
    const step = 10

    for (let radius = step; radius <= searchRadius; radius += step) {
      for (let angle = 0; angle < 360; angle += 45) {
        const radian = (angle * Math.PI) / 180
        const testX = preferredPosition.x + Math.cos(radian) * radius
        const testY = preferredPosition.y + Math.sin(radian) * radius

        const testNodeAtNewPos = { ...node, position: { x: testX, y: testY } }
        const testCollision = this.checkCollision(testNodeAtNewPos)

        if (!testCollision.hasCollision) {
          return { x: testX, y: testY }
        }
      }
    }

    // 如果找不到安全位置，返回原始位置
    return preferredPosition
  }
}

// ==================== 拖拽预览组件 ====================

export function DragPreview({ 
  nodes, 
  position, 
  visible = true,
  opacity = 0.7 
}: {
  nodes: FlowNode[]
  position: { x: number; y: number }
  visible?: boolean
  opacity?: number
}) {
  if (!visible || nodes.length === 0) return null

  return (
    <div
      className="absolute pointer-events-none z-50"
      style={{
        left: position.x,
        top: position.y,
        opacity
      }}
    >
      {nodes.map((node, index) => (
        <div
          key={node.id}
          className="absolute bg-blue-500 border-2 border-blue-600 rounded-lg shadow-lg"
          style={{
            left: index * 5, // 轻微偏移显示多个节点
            top: index * 5,
            width: node.size.width,
            height: node.size.height,
            backgroundColor: 'rgba(59, 130, 246, 0.3)',
            border: '2px dashed #3b82f6'
          }}
        >
          <div className="p-2 text-xs text-blue-800 font-medium truncate">
            {node.data.label || node.type}
          </div>
        </div>
      ))}
    </div>
  )
}

// ==================== 区域选择组件 ====================

export function SelectionArea({
  startPosition,
  endPosition,
  visible = true
}: {
  startPosition: { x: number; y: number }
  endPosition: { x: number; y: number }
  visible?: boolean
}) {
  if (!visible) return null

  const left = Math.min(startPosition.x, endPosition.x)
  const top = Math.min(startPosition.y, endPosition.y)
  const width = Math.abs(endPosition.x - startPosition.x)
  const height = Math.abs(endPosition.y - startPosition.y)

  return (
    <div
      className="absolute border-2 border-blue-500 bg-blue-500 bg-opacity-10 pointer-events-none z-40"
      style={{
        left,
        top,
        width,
        height
      }}
    />
  )
}

// ==================== 对齐辅助线组件 ====================

export function AlignmentGuides({ 
  guides 
}: { 
  guides: Array<{ position: number; type: 'horizontal' | 'vertical' }> 
}) {
  return (
    <div className="absolute inset-0 pointer-events-none z-30">
      {guides.map((guide, index) => (
        <div
          key={index}
          className={cn(
            "absolute bg-pink-500 opacity-75",
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
}

// ==================== 拖拽钩子 ====================

export function useDragDrop(
  nodes: FlowNode[],
  config: DragDropConfig,
  onNodesChange: (nodes: FlowNode[]) => void
) {
  const [dragState, setDragState] = useState<DragState>({
    isDragging: false,
    draggedNodes: [],
    startPosition: { x: 0, y: 0 },
    currentPosition: { x: 0, y: 0 },
    offset: { x: 0, y: 0 },
    dragType: 'move'
  })

  const [isSelecting, setIsSelecting] = useState(false)
  const [selectionStart, setSelectionStart] = useState({ x: 0, y: 0 })
  const [selectionEnd, setSelectionEnd] = useState({ x: 0, y: 0 })

  const dragManagerRef = useRef(new DragDropManager(config))

  // 更新拖拽管理器
  useEffect(() => {
    dragManagerRef.current.updateNodes(nodes)
    dragManagerRef.current.updateConfig(config)
  }, [nodes, config])

  // 开始拖拽节点
  const startNodeDrag = useCallback((nodeIds: string[], startPos: { x: number; y: number }) => {
    setDragState({
      isDragging: true,
      draggedNodes: nodeIds,
      startPosition: startPos,
      currentPosition: startPos,
      offset: { x: 0, y: 0 },
      dragType: 'move'
    })
  }, [])

  // 开始区域选择
  const startAreaSelection = useCallback((startPos: { x: number; y: number }) => {
    setIsSelecting(true)
    setSelectionStart(startPos)
    setSelectionEnd(startPos)
  }, [])

  // 更新拖拽位置
  const updateDrag = useCallback((currentPos: { x: number; y: number }) => {
    if (dragState.isDragging) {
      const offset = {
        x: currentPos.x - dragState.startPosition.x,
        y: currentPos.y - dragState.startPosition.y
      }

      setDragState(prev => ({
        ...prev,
        currentPosition: currentPos,
        offset
      }))

      // 实时更新节点位置
      const updatedNodes = nodes.map(node => {
        if (dragState.draggedNodes.includes(node.id)) {
          const newPosition = {
            x: node.position.x + offset.x,
            y: node.position.y + offset.y
          }

          // 应用吸附
          const snappedPosition = dragManagerRef.current.calculateSnap(
            newPosition,
            dragState.draggedNodes
          )

          return {
            ...node,
            position: snappedPosition
          }
        }
        return node
      })

      onNodesChange(updatedNodes)
    } else if (isSelecting) {
      setSelectionEnd(currentPos)
    }
  }, [dragState, isSelecting, nodes, onNodesChange])

  // 结束拖拽
  const endDrag = useCallback(() => {
    if (dragState.isDragging) {
      // 应用最终位置和碰撞检测
      const finalNodes = nodes.map(node => {
        if (dragState.draggedNodes.includes(node.id)) {
          const safePosition = dragManagerRef.current.getSafePosition(
            node,
            node.position
          )
          return {
            ...node,
            position: safePosition
          }
        }
        return node
      })

      onNodesChange(finalNodes)
    }

    setDragState({
      isDragging: false,
      draggedNodes: [],
      startPosition: { x: 0, y: 0 },
      currentPosition: { x: 0, y: 0 },
      offset: { x: 0, y: 0 },
      dragType: 'move'
    })
  }, [dragState, nodes, onNodesChange])

  // 结束区域选择
  const endAreaSelection = useCallback(() => {
    if (isSelecting) {
      // 计算选择区域内的节点
      const selectionRect = {
        left: Math.min(selectionStart.x, selectionEnd.x),
        top: Math.min(selectionStart.y, selectionEnd.y),
        right: Math.max(selectionStart.x, selectionEnd.x),
        bottom: Math.max(selectionStart.y, selectionEnd.y)
      }

      const selectedNodeIds = nodes
        .filter(node => {
          const nodeRect = {
            left: node.position.x,
            top: node.position.y,
            right: node.position.x + node.size.width,
            bottom: node.position.y + node.size.height
          }

          // 检查节点是否与选择区域相交
          return !(
            nodeRect.right < selectionRect.left ||
            nodeRect.left > selectionRect.right ||
            nodeRect.bottom < selectionRect.top ||
            nodeRect.top > selectionRect.bottom
          )
        })
        .map(node => node.id)

      setIsSelecting(false)
      return selectedNodeIds
    }
    return []
  }, [isSelecting, selectionStart, selectionEnd, nodes])

  return {
    dragState,
    isSelecting,
    selectionStart,
    selectionEnd,
    startNodeDrag,
    startAreaSelection,
    updateDrag,
    endDrag,
    endAreaSelection
  }
}

export default {
  DragDropManager,
  DragPreview,
  SelectionArea,
  AlignmentGuides,
  useDragDrop
}

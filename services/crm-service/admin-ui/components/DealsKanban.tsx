import React, { useMemo, useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { apiClient } from '../lib/api'
import { useNavigate } from 'react-router-dom'
import { Card, CardContent } from '../ui/Card'
import { Button } from '../ui/Button'

// Import CRM service types
import { Deal, DealStage } from '../../packages/atomo-client-sdk/types'

const STAGES: { key: string; title: string; color: string }[] = [
  { key: 'lead', title: '线索', color: '#e3f2fd' },
  { key: 'qualified', title: '已资格', color: '#e8f5e9' },
  { key: 'proposal', title: '提案', color: '#fff3e0' },
  { key: 'negotiation', title: '谈判', color: '#fce4ec' },
  { key: 'won', title: '已成交', color: '#e8f5e8' },
  { key: 'lost', title: '已失败', color: '#ffebee' },
]

export function DealsKanban() {
  const queryClient = useQueryClient()
  const navigate = useNavigate()
  const [draggingId, setDraggingId] = useState<string | null>(null)
  const [draggingStage, setDraggingStage] = useState<string | null>(null)
  const [overCard, setOverCard] = useState<{ id: string; stage: string } | null>(null)

  const { data, isLoading, error } = useQuery({
    queryKey: ['deals', { limit: 200 }],
    queryFn: async () => {
      const res = await apiClient.listEntities('Deal', { page: 1, limit: 200, sort: 'position', order: 'asc' })
      return res.data as Deal[]
    },
    staleTime: 5_000,
  })

  const dealsByStage = useMemo(() => {
    const map: Record<string, Deal[]> = {}
    for (const s of STAGES) map[s.key] = []
    for (const d of (data || [])) {
      // Map DealStage enum to stage key, fallback to lowercase string
      let stageKey = (d.stage || '').toLowerCase()
      
      // Handle enum values - if it's a DealStage enum, convert to lowercase
      if (typeof d.stage === 'string' && ['lead', 'qualified', 'proposal', 'negotiation', 'won', 'lost'].includes(d.stage)) {
        stageKey = d.stage.toLowerCase()
      }
      
      if (!map[stageKey]) map[stageKey] = []
      map[stageKey].push(d)
    }
    return map
  }, [data])

  const [localOrder, setLocalOrder] = useState<Record<string, string[]>>({})
  React.useEffect(() => {
    const initial: Record<string, string[]> = {}
    for (const s of STAGES) {
      initial[s.key] = (dealsByStage[s.key] || []).map(d => d.id)
    }
    setLocalOrder(initial)
  }, [JSON.stringify(dealsByStage)])

  const updateMutation = useMutation({
    mutationKey: ['deal-stage-update'],
    mutationFn: async ({ id, stage, position }: { id: string; stage: string; position?: number }) => {
      await apiClient.updateEntity('Deal', id, position != null ? { stage, position } : { stage })
    },
    onMutate: async ({ id, stage, position }) => {
      await queryClient.cancelQueries({ queryKey: ['deals', { limit: 200 }] })
      const prev = queryClient.getQueryData<Deal[]>(['deals', { limit: 200 }])
      if (prev) {
        const next = prev.map(d => (d.id === id ? { ...d, stage, position: position ?? d.position } : d))
        queryClient.setQueryData(['deals', { limit: 200 }], next)
      }
      return { prev }
    },
    onError: (_err, _vars, ctx) => {
      if (ctx?.prev) queryClient.setQueryData(['deals', { limit: 200 }], ctx.prev)
    },
    onSettled: () => {
      queryClient.invalidateQueries({ queryKey: ['deals', { limit: 200 }] })
    }
  })

  const onDragStart = (id: string, stageKey: string) => { setDraggingId(id); setDraggingStage(stageKey) }
  const persistStageOrders = async (stageKey: string) => {
    const ids = localOrder[stageKey] || []
    const updates = ids.map((id, idx) => ({ id, position: idx, stage: stageKey }))
    try { await apiClient.updateDealPositions(updates) } catch {}
  }

  const onDropOnStage = async (stageKey: string) => {
    if (draggingId) {
      if (draggingStage && draggingStage === stageKey) {
        // same column reorder only updates local state
        const current = localOrder[stageKey] || []
        const filtered = current.filter(i => i !== draggingId)
        let idx = filtered.length
        if (overCard && overCard.stage === stageKey) {
          const pos = filtered.indexOf(overCard.id)
          if (pos >= 0) idx = pos
        }
        const next = [...filtered.slice(0, idx), draggingId, ...filtered.slice(idx)]
        const newState = { ...localOrder, [stageKey]: next }
        setLocalOrder(newState)
        persistStageOrders(stageKey)
      } else {
        // move across columns
        const src = draggingStage!
        const srcIds = (localOrder[src] || []).filter(i => i !== draggingId)
        let destIds = (localOrder[stageKey] || []).filter(i => i !== draggingId)
        let idx = destIds.length
        if (overCard && overCard.stage === stageKey) {
          const pos = destIds.indexOf(overCard.id)
          if (pos >= 0) idx = pos
        }
        destIds = [...destIds.slice(0, idx), draggingId, ...destIds.slice(idx)]
        const newState = { ...localOrder, [src]: srcIds, [stageKey]: destIds }
        setLocalOrder(newState)
        // persist stage change and both columns' positions
        // Batch persist: include the moved item stage change
        const updates = [
          { id: draggingId, stage: stageKey, position: idx },
          ...srcIds.map((id, i) => ({ id, position: i, stage: src })),
          ...destIds.map((id, i) => ({ id, position: i, stage: stageKey }))
        ]
        try { await apiClient.updateDealPositions(updates) } catch (e) { console.warn('Batch update failed', e) }
      }
      setDraggingId(null)
      setDraggingStage(null)
      setOverCard(null)
    }
  }

  if (isLoading) {
    return (
      <div className="p-6">
        <Card><CardContent className="py-8 text-center">加载商机中...</CardContent></Card>
      </div>
    )
  }
  if (error) {
    return (
      <div className="p-6">
        <Card><CardContent className="py-8 text-center text-red-600">加载失败</CardContent></Card>
      </div>
    )
  }

  return (
    <div className="p-4">
      <div className="flex items-center justify-between mb-4">
        <h1 className="text-2xl font-semibold">商机看板</h1>
        <Button onClick={() => queryClient.invalidateQueries({ queryKey: ['deals', { limit: 200 }] })}>
          刷新
        </Button>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-3 xl:grid-cols-6 gap-4">
        {STAGES.map(col => {
          const items = dealsByStage[col.key] || []
          const totalValue = items.reduce((sum, d) => sum + (d.value || 0), 0)
          return (
            <div key={col.key}
                 className="rounded-lg border border-gray-200 bg-white flex flex-col"
                 onDragOver={(e) => e.preventDefault()}
                 onDrop={() => onDropOnStage(col.key)}>
              <div className="px-3 py-2 border-b" style={{ backgroundColor: col.color }}>
                <div className="flex items-center justify-between">
                  <div className="font-medium">{col.title}</div>
                  <div className="text-sm text-gray-600">{items.length} | ¥{totalValue.toLocaleString()}</div>
                </div>
              </div>
              <div className="p-2 space-y-2 overflow-auto" style={{ minHeight: 300 }}>
                {(localOrder[col.key] || items.map(i => i.id)).map(id => items.find(d => d.id === id)).filter((deal): deal is Deal => deal !== undefined).map(deal => (
                  <div key={deal.id}
                       className="rounded-md border p-3 bg-white shadow-sm cursor-move"
                       draggable
                       onDragStart={() => onDragStart(deal.id, col.key)}
                       onDragOver={(e) => { e.preventDefault(); setOverCard({ id: deal.id, stage: col.key }) }}
                       onClick={() => navigate(`/entities/Deal/${deal.id}`)}>
                    <div className="flex items-center justify-between">
                      <div className="font-medium truncate" title={deal.title}>{deal.title}</div>
                      <div className="text-sm text-gray-600">¥{(deal.value || 0).toLocaleString()}</div>
                    </div>
                    {deal.expectedCloseDate && (
                      <div className="text-xs text-gray-500 mt-1">预期成交: {new Date(deal.expectedCloseDate).toLocaleDateString()}</div>
                    )}
                  </div>
                ))}
                {items.length === 0 && (
                  <div className="text-sm text-gray-400 text-center py-4">无商机</div>
                )}
              </div>
            </div>
          )
        })}
      </div>
    </div>
  )
}

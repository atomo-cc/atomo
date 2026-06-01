import { useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { useTranslation } from 'react-i18next'
import '../i18n'
import { Card, CardContent } from '../ui/Card'
import { Button } from '../ui/Button'
import { apiClient } from '../lib/api'
import type { Contact } from '../../packages/atomo-client-sdk/types'

interface ContactTimelineProps {
  contactId: string
}

type ActivityType = 'note' | 'call' | 'meeting' | 'email' | 'task'

interface Activity {
  id: string
  activityType?: ActivityType
  type?: string
  title?: string
  content?: string
  createdAt?: string
  created_at?: string
}

interface NoteBlock {
  id?: string
  type?: string
  text?: string
  notes?: string
  createdAt?: string
  recordedAt?: string
}

export function ContactTimeline({ contactId }: ContactTimelineProps) {
  const { t } = useTranslation()
  const queryClient = useQueryClient()
  const { data, isLoading, error } = useQuery({
    queryKey: ['contact', contactId],
    queryFn: async () => {
      return apiClient.getEntity('Contact', contactId)
    },
    staleTime: 5_000,
  })
  const { data: activities } = useQuery({
    queryKey: ['activities', contactId],
    queryFn: async () => {
      const res = await apiClient.listEntities('Activity', { filters: { contactId }, sort: 'createdAt', order: 'desc', limit: 100 })
      return res.data as Activity[]
    },
    staleTime: 5_000,
  })

  const [noteText, setNoteText] = useState('')
  const [activityType, setActivityType] = useState<ActivityType>('note')
  const [activityTitle, setActivityTitle] = useState('')
  const [activityContent, setActivityContent] = useState('')
  const addNote = useMutation({
    mutationFn: async (text: string) => {
      const existing = ((data as Contact | undefined)?.notes || []) as NoteBlock[]
      const newBlock = { id: String(Date.now()), text, order: existing.length + 1, type: 'ParagraphBlock' }
      await apiClient.updateEntity('Contact', contactId, { notes: [...existing, newBlock] })
    },
    onSuccess: () => {
      setNoteText('')
      queryClient.invalidateQueries({ queryKey: ['contact', contactId] })
    }
  })
  const addActivity = useMutation({
    mutationFn: async () => {
      const metadata: Record<string, unknown> = {}
      if (activityType === 'call') {
        // Example fields for call
        metadata.durationMinutes = Number(prompt(t('timeline.callDuration'), '10') || 10)
        metadata.outcome = prompt(t('timeline.callOutcome'), 'left voicemail')
      } else if (activityType === 'meeting') {
        const when = prompt(t('timeline.meetingTime'), '')
        if (when) metadata.meetingTime = when
        const attendees = prompt(t('timeline.attendees'), '')
        if (attendees) metadata.attendees = attendees.split(',').map(s => s.trim())
      }
      await apiClient.createEntity('Activity', {
        contactId,
        activityType,
        title: activityTitle,
        content: activityContent,
        metadata
      })
    },
    onSuccess: () => {
      setActivityContent('')
      setActivityTitle('')
      queryClient.invalidateQueries({ queryKey: ['activities', contactId] })
    }
  })

  if (isLoading) {
    return (
      <div className="p-6"><Card><CardContent className="py-8 text-center">{t('timeline.loading')}</CardContent></Card></div>
    )
  }
  if (error) {
    return (
      <div className="p-6"><Card><CardContent className="py-8 text-center text-red-600">{t('common.error')}</CardContent></Card></div>
    )
  }

  const notes = Array.isArray((data as Contact | undefined)?.notes)
    ? ((data as Contact).notes as NoteBlock[])
    : []
  const noteItems = notes.map((n, idx) => ({
    id: n.id || String(idx),
    type: n.type || 'Note',
    time: n.createdAt || n.recordedAt || null,
    text: n.text || n.notes || (typeof n === 'string' ? n : JSON.stringify(n))
  }))
  const activityItems = (activities || []).map((a) => ({
    id: a.id,
    type: a.activityType || a.type,
    time: a.createdAt || a.created_at || null,
    text: a.title || a.content || ''
  }))
  const timelineItems = [...noteItems, ...activityItems]
    .sort((a, b) => new Date(b.time || 0).getTime() - new Date(a.time || 0).getTime())

  return (
    <div className="p-6 space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-semibold">{t('timeline.title')}</h1>
        <div className="text-gray-600">ID: {contactId}</div>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
        <Card>
          <CardContent className="space-y-3">
            <textarea
              className="w-full border rounded p-2"
              rows={3}
              placeholder={t('timeline.notePlaceholder')}
              value={noteText}
              onChange={(e) => setNoteText(e.target.value)}
            />
            <div className="flex justify-end">
              <Button disabled={!noteText || addNote.isLoading} onClick={() => addNote.mutate(noteText)}>
                {t('timeline.addNote')}
              </Button>
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardContent className="space-y-3">
            <div className="grid grid-cols-2 gap-2">
              <select className="border rounded p-2" value={activityType} onChange={(e) => setActivityType(e.target.value as ActivityType)}>
                <option value="note">{t('timeline.types.note')}</option>
                <option value="call">{t('timeline.types.call')}</option>
                <option value="meeting">{t('timeline.types.meeting')}</option>
                <option value="email">{t('timeline.types.email')}</option>
                <option value="task">{t('timeline.types.task')}</option>
              </select>
              <input className="border rounded p-2" placeholder={t('timeline.titlePlaceholder')} value={activityTitle} onChange={(e) => setActivityTitle(e.target.value)} />
            </div>
            <textarea className="w-full border rounded p-2" rows={3} placeholder={t('timeline.contentPlaceholder')} value={activityContent} onChange={(e) => setActivityContent(e.target.value)} />
            <div className="flex justify-end">
              <Button disabled={addActivity.isLoading || !activityType} onClick={() => addActivity.mutate()}>
                {t('timeline.addActivity')}
              </Button>
            </div>
          </CardContent>
        </Card>
      </div>

      <Card>
        <CardContent>
          <div className="space-y-4">
            {timelineItems.length === 0 && (
              <div className="text-gray-500 text-center py-8">{t('timeline.noActivities')}</div>
            )}
            {timelineItems.map(item => (
              <div key={item.id} className="border-b pb-3">
                <div className="text-sm text-gray-500">{item.time ? new Date(item.time).toLocaleString() : ''}</div>
                <div className="mt-1">{item.text}</div>
              </div>
            ))}
          </div>
        </CardContent>
      </Card>
    </div>
  )
}

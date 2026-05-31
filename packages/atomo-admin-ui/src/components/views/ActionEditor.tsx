import React from 'react'
import type { StepAction } from '../../lib/workflow-serde'
import { Input } from '../ui/Input'
import { Textarea } from '../ui/Textarea'

export interface ActionEditorProps {
  action: StepAction
  onChange: (action: StepAction) => void
}

export function ActionEditor({ action, onChange }: ActionEditorProps) {
  const kind = Object.keys(action)[0] as keyof StepAction
  const body: any = (action as any)[kind]

  switch (kind) {
    case 'SetVariable':
      return (
        <>
          <Input
            label="Key"
            value={body.key}
            onChange={(e) =>
              onChange({ SetVariable: { ...body, key: e.target.value } })
            }
          />
          <Textarea
            label="Value (JSON)"
            value={JSON.stringify(body.value, null, 2)}
            onChange={(e) => {
              try {
                const parsed = JSON.parse(e.target.value)
                onChange({ SetVariable: { ...body, value: parsed } })
              } catch {
                // ignore parse errors while typing
              }
            }}
          />
        </>
      )

    case 'Delay':
      return (
        <Input
          label="Seconds"
          type="number"
          value={String(body.seconds)}
          onChange={(e) => {
            const n = parseFloat(e.target.value)
            onChange({ Delay: { seconds: isNaN(n) ? 0 : n } })
          }}
        />
      )

    case 'Http':
      return (
        <>
          <Input
            label="Method"
            value={body.method}
            onChange={(e) =>
              onChange({ Http: { ...body, method: e.target.value } })
            }
          />
          <Input
            label="URL"
            value={body.url}
            onChange={(e) =>
              onChange({ Http: { ...body, url: e.target.value } })
            }
          />
          <Textarea
            label="Body (JSON, optional)"
            value={body.body != null ? JSON.stringify(body.body, null, 2) : ''}
            onChange={(e) => {
              const raw = e.target.value.trim()
              if (!raw) {
                const { body: _, ...rest } = body
                onChange({ Http: rest })
                return
              }
              try {
                const parsed = JSON.parse(raw)
                onChange({ Http: { ...body, body: parsed } })
              } catch {
                // ignore parse errors while typing
              }
            }}
          />
        </>
      )

    case 'Mutation':
      return (
        <>
          <Textarea
            label="Query"
            value={body.query}
            onChange={(e) =>
              onChange({ Mutation: { ...body, query: e.target.value } })
            }
          />
          <Textarea
            label="Variables (JSON)"
            value={JSON.stringify(body.variables, null, 2)}
            onChange={(e) => {
              try {
                const parsed = JSON.parse(e.target.value)
                onChange({ Mutation: { ...body, variables: parsed } })
              } catch {
                // ignore parse errors while typing
              }
            }}
          />
        </>
      )

    case 'Plugin':
      return (
        <>
          <Input
            label="Plugin Name"
            value={body.plugin_name}
            onChange={(e) =>
              onChange({ Plugin: { ...body, plugin_name: e.target.value } })
            }
          />
          <Input
            label="Function"
            value={body.function}
            onChange={(e) =>
              onChange({ Plugin: { ...body, function: e.target.value } })
            }
          />
        </>
      )

    default:
      return null
  }
}

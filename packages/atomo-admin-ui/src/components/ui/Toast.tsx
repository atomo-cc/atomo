/**
 * Minimal toast system — replaces the blocking `alert()` calls in the live
 * CRUD paths. A module-level emitter (no context plumbing) plus one <Toaster/>
 * mounted at the app shell.
 */

import { useEffect, useState } from 'react'
import { CheckCircle2, AlertCircle, X } from 'lucide-react'

export interface ToastMessage {
  id: number
  kind: 'success' | 'error'
  text: string
}

type Listener = (t: ToastMessage) => void
let listener: Listener | null = null
let nextId = 1

export const toast = {
  success(text: string) {
    listener?.({ id: nextId++, kind: 'success', text })
  },
  error(text: string) {
    listener?.({ id: nextId++, kind: 'error', text })
  },
}

const AUTO_DISMISS_MS = 5000

export function Toaster() {
  const [toasts, setToasts] = useState<ToastMessage[]>([])

  useEffect(() => {
    listener = (t) => {
      setToasts((prev) => [...prev, t])
      setTimeout(() => setToasts((prev) => prev.filter((x) => x.id !== t.id)), AUTO_DISMISS_MS)
    }
    return () => {
      listener = null
    }
  }, [])

  if (toasts.length === 0) return null

  return (
    <div className="fixed bottom-4 right-4 z-50 flex flex-col gap-2" role="status" aria-live="polite">
      {toasts.map((t) => (
        <div
          key={t.id}
          className={`flex items-center gap-2 rounded-lg border px-4 py-3 shadow-lg bg-white text-sm ${
            t.kind === 'error' ? 'border-red-300 text-red-800' : 'border-green-300 text-green-800'
          }`}
        >
          {t.kind === 'error' ? (
            <AlertCircle className="h-4 w-4 shrink-0" />
          ) : (
            <CheckCircle2 className="h-4 w-4 shrink-0" />
          )}
          <span>{t.text}</span>
          <button
            onClick={() => setToasts((prev) => prev.filter((x) => x.id !== t.id))}
            className="ml-2 rounded p-0.5 hover:bg-gray-100"
            aria-label="Dismiss"
          >
            <X className="h-3 w-3" />
          </button>
        </div>
      ))}
    </div>
  )
}

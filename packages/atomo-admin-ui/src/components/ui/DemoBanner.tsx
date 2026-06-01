/**
 * DemoBanner — a visible notice that a feature is a demo/preview with no live backend yet.
 * Keeps the UI honest about which capabilities are real vs. simulated.
 */
import * as React from 'react'
import { Badge } from './Badge'
import { cn } from '../../lib/utils'

export interface DemoBannerProps {
  /** What the feature shows when there's no backend (e.g. "sample data"). */
  detail?: string
  className?: string
}

export function DemoBanner({ detail, className }: DemoBannerProps) {
  return (
    <div
      className={cn(
        'flex items-center gap-2 rounded-md border border-warning-200 bg-warning-50 px-3 py-2 text-sm text-warning-800',
        className
      )}
      role="note"
    >
      <Badge variant="warning">Demo / Preview</Badge>
      <span>{detail ?? 'This view shows sample data — no live backend is wired yet.'}</span>
    </div>
  )
}

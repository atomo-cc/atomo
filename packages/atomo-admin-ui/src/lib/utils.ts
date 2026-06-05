/**
 * Utility functions for Atomo Admin UI
 */

import { type ClassValue, clsx } from 'clsx'
import { twMerge } from 'tailwind-merge'

/**
 * Merge Tailwind CSS class names.
 */
export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}

/**
 * Format a date.
 */
export function formatDate(date: Date | string | null | undefined, format: 'short' | 'long' | 'time' = 'short') {
  // Guard missing/invalid dates — many records have null timestamps, and an
  // unguarded `undefined.toLocaleString()` crashes the whole view.
  if (date == null) return '—'
  const d = typeof date === 'string' ? new Date(date) : date
  if (!(d instanceof Date) || Number.isNaN(d.getTime())) return '—'

  switch (format) {
    case 'short':
      return d.toLocaleDateString()
    case 'long':
      return d.toLocaleDateString('zh-CN', {
        year: 'numeric',
        month: 'long',
        day: 'numeric',
      })
    case 'time':
      return d.toLocaleString()
    default:
      return d.toLocaleDateString()
  }
}

/**
 * Format a file size.
 */
export function formatFileSize(bytes: number) {
  if (bytes === 0) return '0 Bytes'
  
  const k = 1024
  const sizes = ['Bytes', 'KB', 'MB', 'GB']
  const i = Math.floor(Math.log(bytes) / Math.log(k))
  
  return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i]
}

/**
 * Debounce function.
 */
export function debounce<T extends (...args: any[]) => any>(
  func: T,
  wait: number
): (...args: Parameters<T>) => void {
  let timeout: ReturnType<typeof setTimeout>
  
  return (...args: Parameters<T>) => {
    clearTimeout(timeout)
    timeout = setTimeout(() => func(...args), wait)
  }
}

/**
 * Truncate text.
 */
export function truncate(text: string, length: number) {
  if (text.length <= length) return text
  return text.slice(0, length) + '...'
}

/**
 * Get a field's display name.
 */
export function getFieldLabel(fieldName: string, customLabel?: string) {
  if (customLabel) return customLabel

  // Convert camelCase into a readable label
  return fieldName
    .replace(/([A-Z])/g, ' $1')
    .replace(/^./, (str) => str.toUpperCase())
    .trim()
}

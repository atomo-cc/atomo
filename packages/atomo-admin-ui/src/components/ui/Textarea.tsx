/**
 * Textarea Component — Dashin Multiline Input Primitive
 */

import * as React from 'react'
import { cn } from '../../lib/utils'

export interface TextareaProps
  extends React.TextareaHTMLAttributes<HTMLTextAreaElement> {
  error?: string
  label?: string
  helpText?: string
}

const Textarea = React.forwardRef<HTMLTextAreaElement, TextareaProps>(
  ({ className, error, label, helpText, id, ...props }, ref) => {
    const textareaId = id || React.useId()
    
    return (
      <div className="space-y-1.5">
        {label && (
          <label 
            htmlFor={textareaId}
            className="text-xs font-medium text-foreground block"
          >
            {label}
            {props.required && <span className="text-danger ml-0.5">*</span>}
          </label>
        )}
        
        <textarea
          id={textareaId}
          className={cn(
            'flex min-h-[80px] w-full rounded-bn border border-bn-border bg-content-box px-3 py-2 text-sm text-foreground shadow-sm',
            'placeholder:text-icon-muted transition-colors',
            'focus:outline-none focus:ring-2 focus:ring-primary/40 focus:border-primary',
            'disabled:cursor-not-allowed disabled:opacity-50',
            'resize-vertical',
            error && 'border-danger focus:ring-danger/40 focus:border-danger',
            className
          )}
          ref={ref}
          {...props}
        />
        
        {error && (
          <p className="text-xs text-danger font-medium">{error}</p>
        )}
        
        {helpText && !error && (
          <p className="text-xs text-icon-muted">{helpText}</p>
        )}
      </div>
    )
  }
)
Textarea.displayName = 'Textarea'

export { Textarea }
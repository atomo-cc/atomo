/**
 * Input Component — Dashin Input Primitive
 */

import * as React from 'react'
import { cn } from '../../lib/utils'

export interface InputProps
  extends React.InputHTMLAttributes<HTMLInputElement> {
  error?: string
  label?: string
  helpText?: string
}

const Input = React.forwardRef<HTMLInputElement, InputProps>(
  ({ className, type, error, label, helpText, id, ...props }, ref) => {
    const inputId = id || React.useId()
    
    return (
      <div className="space-y-1.5">
        {label && (
          <label 
            htmlFor={inputId}
            className="text-xs font-medium text-foreground block"
          >
            {label}
            {props.required && <span className="text-danger ml-0.5">*</span>}
          </label>
        )}
        
        <input
          id={inputId}
          type={type}
          className={cn(
            'flex h-9 w-full rounded-bn border border-bn-border bg-content-box px-3 py-1.5 text-sm text-foreground shadow-sm',
            'placeholder:text-icon-muted transition-colors',
            'focus:outline-none focus:ring-2 focus:ring-primary/40 focus:border-primary',
            'disabled:cursor-not-allowed disabled:opacity-50',
            error && 'border-danger focus:ring-danger/40 focus:border-danger',
            className
          )}
          ref={ref}
          {...props}
        />
        
        {error && (
          <p className="text-xs text-danger font-medium">{error}</p>
        )}
        
        {helpText && !error && !label && (
          <p className="text-xs text-icon-muted">{helpText}</p>
        )}
      </div>
    )
  }
)
Input.displayName = 'Input'

export { Input }

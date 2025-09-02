/**
 * Input Component - 基础输入组件
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
      <div className="space-y-2">
        {label && (
          <label 
            htmlFor={inputId}
            className="text-sm font-medium text-gray-700"
          >
            {label}
            {props.required && <span className="text-danger-500 ml-1">*</span>}
          </label>
        )}
        
        <input
          id={inputId}
          type={type}
          className={cn(
            'flex h-10 w-full rounded-md border border-gray-300 bg-white px-3 py-2 text-sm',
            'placeholder:text-gray-400',
            'focus:outline-none focus:ring-2 focus:ring-primary-500 focus:border-transparent',
            'disabled:cursor-not-allowed disabled:opacity-50',
            error && 'border-danger-500 focus:ring-danger-500',
            className
          )}
          ref={ref}
          {...props}
        />
        
        {/* 只显示error，不显示helpText，因为FormField会处理这些 */}
        {error && (
          <p className="text-sm text-danger-600">{error}</p>
        )}
        
        {helpText && !error && !label && (
          <p className="text-sm text-gray-500">{helpText}</p>
        )}
      </div>
    )
  }
)
Input.displayName = 'Input'

export { Input }

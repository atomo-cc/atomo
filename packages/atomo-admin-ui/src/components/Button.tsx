/**
 * Admin UI Button - Platform-neutral button component
 * 
 * Designed to be:
 * - Minimal and functional
 * - Easy to theme via CSS variables
 * - Accessible by default
 */

import React from 'react'

interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: 'primary' | 'secondary' | 'danger'
  size?: 'sm' | 'md' | 'lg'
  loading?: boolean
}

export function Button({ 
  variant = 'primary', 
  size = 'md', 
  loading = false,
  children, 
  className = '',
  disabled,
  ...props 
}: ButtonProps) {
  return (
    <button
      className={`admin-button admin-button--${variant} admin-button--${size} ${className}`}
      disabled={disabled || loading}
      {...props}
    >
      {loading ? <span className="admin-spinner" /> : children}
    </button>
  )
}

export default Button

/**
 * DatePicker Component - 日期选择器组件
 * 
 * 简单的日期输入组件，后续可以升级为更复杂的日期选择器
 */

import * as React from 'react'
import { Input } from './Input'
import { format, parseISO } from 'date-fns'

export interface DatePickerProps {
  value?: Date | string
  onChange: (date: Date | undefined) => void
  disabled?: boolean
  error?: string
  showTime?: boolean
  placeholder?: string
}

const DatePicker = React.forwardRef<HTMLInputElement, DatePickerProps>(
  ({ value, onChange, disabled, error, showTime = false, placeholder, ...props }, ref) => {
    // 格式化显示值
    const formatValue = (val: Date | string | undefined) => {
      if (!val) return ''
      
      const date = typeof val === 'string' ? parseISO(val) : val
      if (showTime) {
        return format(date, "yyyy-MM-dd'T'HH:mm")
      }
      return format(date, 'yyyy-MM-dd')
    }

    // 处理值变化
    const handleChange = (e: React.ChangeEvent<HTMLInputElement>) => {
      const inputValue = e.target.value
      if (!inputValue) {
        onChange(undefined)
        return
      }
      
      try {
        const date = new Date(inputValue)
        if (!isNaN(date.getTime())) {
          onChange(date)
        }
      } catch {
        // 忽略无效日期
      }
    }

    return (
      <Input
        ref={ref}
        type={showTime ? 'datetime-local' : 'date'}
        value={formatValue(value)}
        onChange={handleChange}
        disabled={disabled}
        error={error}
        placeholder={placeholder}
        {...props}
      />
    )
  }
)
DatePicker.displayName = 'DatePicker'

export { DatePicker }

/**
 * Date Picker Component - 日期选择器组件
 * 
 * 简化版本的日期选择器
 */

import * as React from 'react'
import { Calendar, Clock } from 'lucide-react'
import { formatDate } from '../../lib/utils'
import { Input } from './Input'
import { Button } from './Button'

export interface DatePickerProps {
  value?: Date | string
  onChange: (date: Date | undefined) => void
  disabled?: boolean
  error?: string
  placeholder?: string
  showTime?: boolean
  minDate?: Date
  maxDate?: Date
}

const DatePicker = React.forwardRef<HTMLInputElement, DatePickerProps>(
  ({ 
    value, 
    onChange, 
    disabled = false, 
    error, 
    placeholder = '选择日期',
    showTime = false,
    minDate,
    maxDate,
    ...props 
  }, ref) => {
      const [inputValue, setInputValue] = React.useState('')

    // 转换值为 Date 对象
    const dateValue = React.useMemo(() => {
      if (!value) return undefined
      if (value instanceof Date) return value
      return new Date(value)
    }, [value])

    // 同步输入框值
    React.useEffect(() => {
      if (dateValue && !isNaN(dateValue.getTime())) {
        const formatted = showTime 
          ? dateValue.toISOString().slice(0, 16) // YYYY-MM-DDTHH:mm
          : dateValue.toISOString().slice(0, 10)  // YYYY-MM-DD
        setInputValue(formatted)
      } else {
        setInputValue('')
      }
    }, [dateValue, showTime])

    // 处理输入变化
    const handleInputChange = (e: React.ChangeEvent<HTMLInputElement>) => {
      const newValue = e.target.value
      setInputValue(newValue)

      if (newValue) {
        const date = new Date(newValue)
        if (!isNaN(date.getTime())) {
          // 检查日期范围
          if (minDate && date < minDate) return
          if (maxDate && date > maxDate) return
          
          onChange(date)
        }
      } else {
        onChange(undefined)
      }
    }

    // 清空日期
    const handleClear = () => {
      setInputValue('')
      onChange(undefined)
    }

    // 设置为今天
    const handleToday = () => {
      const today = new Date()
      onChange(today)
    }

    return (
      <div className="relative">
        <div className="relative">
          <Input
            ref={ref}
            type={showTime ? 'datetime-local' : 'date'}
            value={inputValue}
            onChange={handleInputChange}
            disabled={disabled}
            error={error}
            placeholder={placeholder}
            min={minDate?.toISOString().slice(0, showTime ? 16 : 10)}
            max={maxDate?.toISOString().slice(0, showTime ? 16 : 10)}
            className="pr-20"
            {...props}
          />
          
          <div className="absolute right-2 top-1/2 transform -translate-y-1/2 flex items-center gap-1">
            {dateValue && (
              <Button
                type="button"
                variant="ghost"
                size="sm"
                onClick={handleClear}
                disabled={disabled}
                className="h-6 w-6 p-0 text-gray-400 hover:text-gray-600"
              >
                ×
              </Button>
            )}
            
            <Button
              type="button"
              variant="ghost"
              size="sm"
              onClick={handleToday}
              disabled={disabled}
              className="h-6 w-6 p-0 text-gray-400 hover:text-gray-600"
              title={showTime ? '设为现在' : '设为今天'}
            >
              {showTime ? <Clock className="h-3 w-3" /> : <Calendar className="h-3 w-3" />}
            </Button>
          </div>
        </div>

        {/* 显示格式化的日期（如果有值） */}
        {dateValue && !error && (
          <p className="text-xs text-gray-500 mt-1">
            {formatDate(dateValue, showTime ? 'time' : 'long')}
          </p>
        )}
      </div>
    )
  }
)

DatePicker.displayName = 'DatePicker'

export { DatePicker }
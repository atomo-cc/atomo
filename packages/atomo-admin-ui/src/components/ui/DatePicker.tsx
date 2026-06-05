/**
 * Date Picker Component — date picker
 * 
 * A simplified date picker.
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
    placeholder = 'Select date',
    showTime = false,
    minDate,
    maxDate,
    ...props 
  }, ref) => {
      const [inputValue, setInputValue] = React.useState('')

    // Convert the value to a Date object
    const dateValue = React.useMemo(() => {
      if (!value) return undefined
      if (value instanceof Date) return value
      return new Date(value)
    }, [value])

    // Sync the input value
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

    // Handle input changes
    const handleInputChange = (e: React.ChangeEvent<HTMLInputElement>) => {
      const newValue = e.target.value
      setInputValue(newValue)

      if (newValue) {
        const date = new Date(newValue)
        if (!isNaN(date.getTime())) {
          // Check the date range
          if (minDate && date < minDate) return
          if (maxDate && date > maxDate) return
          
          onChange(date)
        }
      } else {
        onChange(undefined)
      }
    }

    // Clear the date
    const handleClear = () => {
      setInputValue('')
      onChange(undefined)
    }

    // Set to today
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
              title={showTime ? 'Set to now' : 'Set to today'}
            >
              {showTime ? <Clock className="h-3 w-3" /> : <Calendar className="h-3 w-3" />}
            </Button>
          </div>
        </div>

        {/* Show the formatted date (if any) */}
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
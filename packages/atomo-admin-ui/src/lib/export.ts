/**
 * Data Export Utilities
 * 
 * 提供数据导出功能，支持 CSV 和 Excel 格式
 */

import { EntityData, ColumnConfig } from './types'
import { formatDate } from './utils'

export type ExportFormat = 'csv' | 'excel'

/**
 * 导出数据到指定格式
 */
export async function exportData(
  data: EntityData[],
  columns: ColumnConfig[],
  format: ExportFormat,
  filename?: string
): Promise<void> {
  const exportFilename = filename || `data_export_${new Date().toISOString().split('T')[0]}`
  
  switch (format) {
    case 'csv':
      return exportToCSV(data, columns, exportFilename)
    case 'excel':
      return exportToExcel(data, columns, exportFilename)
    default:
      throw new Error(`Unsupported export format: ${format}`)
  }
}

/**
 * 导出为 CSV 格式
 */
export function exportToCSV(
  data: EntityData[],
  columns: ColumnConfig[],
  filename: string
): void {
  // 生成 CSV 内容
  const csvContent = generateCSVContent(data, columns)
  
  // 创建 Blob
  const blob = new Blob(['\uFEFF' + csvContent], { type: 'text/csv;charset=utf-8;' })
  
  // 下载文件
  downloadBlob(blob, `${filename}.csv`)
}

/**
 * 导出为 Excel 格式 (使用 CSV 格式，但扩展名为 .xlsx)
 * 注意：这是一个简化实现，实际项目中可能需要使用 SheetJS 等库
 */
export function exportToExcel(
  data: EntityData[],
  columns: ColumnConfig[],
  filename: string
): void {
  // 对于简化实现，我们使用 CSV 格式但以 .xlsx 扩展名下载
  // 实际项目中应该使用专门的 Excel 库如 xlsx.js
  const csvContent = generateCSVContent(data, columns)
  
  const blob = new Blob(['\uFEFF' + csvContent], { type: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet' })
  
  downloadBlob(blob, `${filename}.xlsx`)
}

/**
 * 生成 CSV 内容
 */
function generateCSVContent(data: EntityData[], columns: ColumnConfig[]): string {
  // 生成表头
  const headers = columns.map(col => escapeCSVField(col.label))
  const csvLines = [headers.join(',')]
  
  // 生成数据行
  data.forEach(row => {
    const values = columns.map(col => {
      let value = row[col.key]
      
      // 格式化不同类型的数据
      if (value === null || value === undefined) {
        return ''
      }
      
      // 日期格式化
      if (col.type === 'date' || col.type === 'datetime') {
        if (value instanceof Date || typeof value === 'string') {
          value = formatDate(value, col.type === 'datetime' ? 'time' : 'short')
        }
      }
      
      // 数组类型
      if (Array.isArray(value)) {
        value = value.join('; ')
      }
      
      // 对象类型
      if (typeof value === 'object') {
        value = JSON.stringify(value)
      }
      
      return escapeCSVField(String(value))
    })
    
    csvLines.push(values.join(','))
  })
  
  return csvLines.join('\n')
}

/**
 * 转义 CSV 字段
 */
function escapeCSVField(field: string): string {
  // 如果字段包含逗号、引号或换行符，需要用引号包围并转义内部引号
  if (field.includes(',') || field.includes('"') || field.includes('\n') || field.includes('\r')) {
    return `"${field.replace(/"/g, '""')}"`
  }
  return field
}

/**
 * 下载 Blob 文件
 */
function downloadBlob(blob: Blob, filename: string): void {
  const url = window.URL.createObjectURL(blob)
  const link = document.createElement('a')
  
  link.href = url
  link.download = filename
  document.body.appendChild(link)
  link.click()
  
  // 清理
  document.body.removeChild(link)
  window.URL.revokeObjectURL(url)
}

/**
 * 获取数据统计信息
 */
export function getExportStats(data: EntityData[], columns: ColumnConfig[]) {
  return {
    totalRows: data.length,
    totalColumns: columns.length,
    estimatedSize: estimateExportSize(data, columns),
  }
}

/**
 * 估算导出文件大小（字节）
 */
function estimateExportSize(data: EntityData[], columns: ColumnConfig[]): number {
  if (data.length === 0) return 0
  
  // 计算表头大小
  const headerSize = columns.reduce((sum, col) => sum + col.label.length, 0) + columns.length
  
  // 估算数据大小（取前几行的平均值）
  const sampleSize = Math.min(10, data.length)
  const sampleData = data.slice(0, sampleSize)
  
  const avgRowSize = sampleData.reduce((sum, row) => {
    const rowSize = columns.reduce((cellSum, col) => {
      const value = row[col.key]
      const cellSize = value ? String(value).length : 0
      return cellSum + cellSize
    }, 0)
    return sum + rowSize + columns.length // 加上分隔符
  }, 0) / sampleSize
  
  return Math.round(headerSize + (avgRowSize * data.length))
}

/**
 * 格式化文件大小
 */
export function formatFileSize(bytes: number): string {
  if (bytes === 0) return '0 B'
  
  const k = 1024
  const sizes = ['B', 'KB', 'MB', 'GB']
  const i = Math.floor(Math.log(bytes) / Math.log(k))
  
  return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i]
}

/**
 * 验证导出参数
 */
export function validateExportParams(
  data: EntityData[],
  columns: ColumnConfig[],
  format: ExportFormat
): { valid: boolean; errors: string[] } {
  const errors: string[] = []
  
  if (!data || data.length === 0) {
    errors.push('没有数据可导出')
  }
  
  if (!columns || columns.length === 0) {
    errors.push('没有列配置')
  }
  
  if (!['csv', 'excel'].includes(format)) {
    errors.push('不支持的导出格式')
  }
  
  // 检查数据量限制（避免浏览器崩溃）
  if (data && data.length > 50000) {
    errors.push('数据量过大，建议分批导出（最大 50,000 行）')
  }
  
  return {
    valid: errors.length === 0,
    errors
  }
}

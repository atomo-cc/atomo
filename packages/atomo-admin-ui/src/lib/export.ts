/**
 * Data Export Utilities
 * 
 * 数据导出功能，支持 CSV 和 Excel 格式
 */

import { EntityData, ColumnConfig } from './types'

/**
 * 导出数据到指定格式
 */
export async function exportData(
  data: EntityData[],
  columns: ColumnConfig[],
  format: 'csv' | 'excel',
  filename: string
): Promise<void> {
  try {
    if (format === 'csv') {
      await exportToCSV(data, columns, filename)
    } else if (format === 'excel') {
      await exportToExcel(data, columns, filename)
    }
  } catch (error) {
    console.error('导出失败:', error)
    throw new Error('导出失败，请重试')
  }
}

/**
 * 导出为 CSV 格式
 */
async function exportToCSV(
  data: EntityData[],
  columns: ColumnConfig[],
  filename: string
): Promise<void> {
  // 准备表头
  const headers = columns.map(col => col.label)
  
  // 准备数据行
  const rows = data.map(row => {
    return columns.map(col => {
      const value = row[col.key]
      
      // 格式化不同类型的数据
      if (value === null || value === undefined) {
        return ''
      }
      
      if (typeof value === 'object') {
        if (value instanceof Date) {
          return value.toISOString().split('T')[0]
        }
        if (Array.isArray(value)) {
          return value.join('; ')
        }
        return JSON.stringify(value)
      }
      
      // 处理包含逗号或换行的字符串
      const stringValue = String(value)
      if (stringValue.includes(',') || stringValue.includes('\n') || stringValue.includes('"')) {
        return `"${stringValue.replace(/"/g, '""')}"`
      }
      
      return stringValue
    })
  })

  // 组合 CSV 内容
  const csvContent = [
    headers.join(','),
    ...rows.map(row => row.join(','))
  ].join('\n')

  // 创建和下载文件
  const blob = new Blob(['\ufeff' + csvContent], { type: 'text/csv;charset=utf-8;' })
  downloadBlob(blob, `${filename}.csv`)
}

/**
 * 导出为 Excel 格式
 * 注意：这是一个简化版本，生产环境中建议使用专门的 Excel 库如 SheetJS
 */
async function exportToExcel(
  data: EntityData[],
  columns: ColumnConfig[],
  filename: string
): Promise<void> {
  // 创建简单的 XML 格式（Excel 兼容）
  const xmlHeader = '<?xml version="1.0"?>\n' +
    '<Workbook xmlns="urn:schemas-microsoft-com:office:spreadsheet"\n' +
    ' xmlns:o="urn:schemas-microsoft-com:office:office"\n' +
    ' xmlns:x="urn:schemas-microsoft-com:office:excel"\n' +
    ' xmlns:ss="urn:schemas-microsoft-com:office:spreadsheet"\n' +
    ' xmlns:html="http://www.w3.org/TR/REC-html40">\n' +
    '<Worksheet ss:Name="Data">\n<Table>\n'

  const xmlFooter = '</Table>\n</Worksheet>\n</Workbook>'

  // 表头行
  const headerRow = '<Row>\n' + 
    columns.map(col => `<Cell><Data ss:Type="String">${escapeXml(col.label)}</Data></Cell>`).join('\n') +
    '\n</Row>\n'

  // 数据行
  const dataRows = data.map(row => {
    const cells = columns.map(col => {
      const value = row[col.key]
      let cellType = 'String'
      let cellValue = ''

      if (value === null || value === undefined) {
        cellValue = ''
      } else if (typeof value === 'number') {
        cellType = 'Number'
        cellValue = String(value)
      } else if (value instanceof Date) {
        cellType = 'DateTime'
        cellValue = value.toISOString()
      } else if (typeof value === 'boolean') {
        cellType = 'Boolean'
        cellValue = value ? '1' : '0'
      } else if (Array.isArray(value)) {
        cellValue = value.join('; ')
      } else if (typeof value === 'object') {
        cellValue = JSON.stringify(value)
      } else {
        cellValue = String(value)
      }

      return `<Cell><Data ss:Type="${cellType}">${escapeXml(cellValue)}</Data></Cell>`
    }).join('\n')

    return '<Row>\n' + cells + '\n</Row>\n'
  }).join('')

  const xmlContent = xmlHeader + headerRow + dataRows + xmlFooter

  // 创建和下载文件
  const blob = new Blob([xmlContent], { type: 'application/vnd.ms-excel' })
  downloadBlob(blob, `${filename}.xls`)
}

/**
 * 转义 XML 特殊字符
 */
function escapeXml(text: string): string {
  return text
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;')
}

/**
 * 下载 Blob 文件
 */
function downloadBlob(blob: Blob, filename: string): void {
  const url = window.URL.createObjectURL(blob)
  const link = document.createElement('a')
  link.href = url
  link.download = filename
  link.style.display = 'none'
  
  document.body.appendChild(link)
  link.click()
  document.body.removeChild(link)
  
  // 清理 URL
  setTimeout(() => {
    window.URL.revokeObjectURL(url)
  }, 100)
}

/**
 * 格式化数据用于显示
 */
export function formatValueForExport(value: any, type: string): string {
  if (value === null || value === undefined) {
    return ''
  }

  switch (type) {
    case 'date':
    case 'datetime':
      if (value instanceof Date) {
        return type === 'date' 
          ? value.toISOString().split('T')[0]
          : value.toLocaleString()
      }
      return String(value)

    case 'boolean':
      return value ? '是' : '否'

    case 'array':
      return Array.isArray(value) ? value.join(', ') : String(value)

    case 'json':
      return typeof value === 'object' ? JSON.stringify(value) : String(value)

    case 'number':
      return typeof value === 'number' ? value.toLocaleString() : String(value)

    default:
      return String(value)
  }
}
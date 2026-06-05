/**
 * Data Export Utilities
 *
 * Data export functionality, supporting CSV and Excel formats.
 */

import { EntityData, ColumnConfig } from './types'

/**
 * Export data to the specified format.
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
    console.error('Export failed:', error)
    throw new Error('Export failed, please try again')
  }
}

/**
 * Export to CSV format.
 */
async function exportToCSV(
  data: EntityData[],
  columns: ColumnConfig[],
  filename: string
): Promise<void> {
  // Prepare the header row
  const headers = columns.map(col => col.label)

  // Prepare the data rows
  const rows = data.map(row => {
    return columns.map(col => {
      const value = row[col.key]
      
      // Format the different data types
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
      
      // Handle strings containing commas or newlines
      const stringValue = String(value)
      if (stringValue.includes(',') || stringValue.includes('\n') || stringValue.includes('"')) {
        return `"${stringValue.replace(/"/g, '""')}"`
      }
      
      return stringValue
    })
  })

  // Assemble the CSV content
  const csvContent = [
    headers.join(','),
    ...rows.map(row => row.join(','))
  ].join('\n')

  // Create and download the file
  const blob = new Blob(['\ufeff' + csvContent], { type: 'text/csv;charset=utf-8;' })
  downloadBlob(blob, `${filename}.csv`)
}

/**
 * Export to Excel format.
 * Note: this is a simplified version; for production use a dedicated Excel library such as SheetJS.
 */
async function exportToExcel(
  data: EntityData[],
  columns: ColumnConfig[],
  filename: string
): Promise<void> {
  // Create a simple XML format (Excel-compatible)
  const xmlHeader = '<?xml version="1.0"?>\n' +
    '<Workbook xmlns="urn:schemas-microsoft-com:office:spreadsheet"\n' +
    ' xmlns:o="urn:schemas-microsoft-com:office:office"\n' +
    ' xmlns:x="urn:schemas-microsoft-com:office:excel"\n' +
    ' xmlns:ss="urn:schemas-microsoft-com:office:spreadsheet"\n' +
    ' xmlns:html="http://www.w3.org/TR/REC-html40">\n' +
    '<Worksheet ss:Name="Data">\n<Table>\n'

  const xmlFooter = '</Table>\n</Worksheet>\n</Workbook>'

  // Header row
  const headerRow = '<Row>\n' +
    columns.map(col => `<Cell><Data ss:Type="String">${escapeXml(col.label)}</Data></Cell>`).join('\n') +
    '\n</Row>\n'

  // Data rows
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

  // Create and download the file
  const blob = new Blob([xmlContent], { type: 'application/vnd.ms-excel' })
  downloadBlob(blob, `${filename}.xls`)
}

/**
 * Escape XML special characters.
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
 * Download a Blob as a file.
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
  
  // Clean up the URL
  setTimeout(() => {
    window.URL.revokeObjectURL(url)
  }, 100)
}

/**
 * Format a value for display in the export.
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
      return value ? 'Yes' : 'No'

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
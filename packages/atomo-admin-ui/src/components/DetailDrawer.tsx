import React, { useState } from 'react'
import { X, Copy, Check, ExternalLink, Image as ImageIcon, ShieldAlert, Clock, CheckCircle2, AlertCircle } from 'lucide-react'
import { ModelMetadata, EntityData } from '../lib/types'

interface DetailDrawerProps {
  modelName: string
  modelMetadata: ModelMetadata
  record: EntityData | null
  isOpen: boolean
  onClose: () => void
}

export function DetailDrawer({
  modelName,
  modelMetadata,
  record,
  isOpen,
  onClose,
}: DetailDrawerProps) {
  const [copiedKey, setCopiedKey] = useState<string | null>(null)
  const [lightboxUrl, setLightboxUrl] = useState<string | null>(null)

  if (!isOpen || !record) return null

  const copyToClipboard = (key: string, val: any) => {
    const text = typeof val === 'object' ? JSON.stringify(val, null, 2) : String(val)
    navigator.clipboard.writeText(text)
    setCopiedKey(key)
    setTimeout(() => setCopiedKey(null), 1500)
  }

  const isReadOnly =
    modelMetadata.access?.update === 'never' ||
    modelMetadata.access?.update === 'system'

  return (
    <>
      {/* Backdrop */}
      <div
        className="fixed inset-0 bg-black/50 backdrop-blur-sm z-40 transition-opacity"
        onClick={onClose}
      />

      {/* Drawer */}
      <div className="fixed inset-y-0 right-0 z-50 w-full max-w-xl sm:max-w-2xl bg-content-box border-l border-bn-border shadow-2xl flex flex-col animate-slide-in overflow-hidden">
        {/* Header */}
        <div className="h-16 px-6 border-b border-bn-border flex items-center justify-between bg-content-bg/40">
          <div className="flex items-center space-x-3">
            <span className="px-2.5 py-1 text-xs font-semibold rounded-full bg-primary/10 text-primary border border-primary/20">
              {modelName}
            </span>
            <h2 className="font-semibold text-foreground text-base truncate max-w-xs">
              {String(record[modelMetadata.primaryKey || 'id'] || 'Record Details')}
            </h2>
          </div>
          <button
            onClick={onClose}
            className="p-2 rounded-bn text-icon-muted hover:text-foreground hover:bg-content-bg transition-colors"
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        {/* Read-only audit notice if applicable */}
        {isReadOnly && (
          <div className="px-6 py-2.5 bg-amber-500/10 border-b border-amber-500/20 text-amber-600 dark:text-amber-400 text-xs flex items-center space-x-2">
            <ShieldAlert className="w-4 h-4 flex-shrink-0" />
            <span>This model is append-only / system-managed. Direct mutations are disabled.</span>
          </div>
        )}

        {/* Body Content */}
        <div className="flex-1 overflow-y-auto p-6 space-y-5">
          {Object.entries(modelMetadata.fields).map(([fieldName, fieldMeta]) => {
            const rawVal = record[fieldName]
            const isNull = rawVal === null || rawVal === undefined
            const isFile = fieldMeta.type === 'file' || fieldName.toLowerCase().includes('mediaid')
            const isStatus = fieldName.toLowerCase() === 'status'

            return (
              <div key={fieldName} className="bg-content-bg/50 rounded-bn p-3.5 border border-bn-border">
                <div className="flex items-center justify-between mb-1.5">
                  <span className="text-xs font-semibold text-icon-muted uppercase tracking-wider">
                    {fieldName}
                  </span>
                  {!isNull && (
                    <button
                      onClick={() => copyToClipboard(fieldName, rawVal)}
                      className="text-icon-muted hover:text-foreground p-1 text-xs flex items-center space-x-1"
                      title="Copy Value"
                    >
                      {copiedKey === fieldName ? (
                        <Check className="w-3.5 h-3.5 text-success" />
                      ) : (
                        <Copy className="w-3.5 h-3.5" />
                      )}
                    </button>
                  )}
                </div>

                {/* Render Value */}
                {isNull ? (
                  <span className="text-xs text-icon-muted italic">null</span>
                ) : isStatus ? (
                  <span
                    className={`inline-flex items-center px-2.5 py-1 rounded-full text-xs font-semibold ${
                      String(rawVal) === 'done' || String(rawVal) === 'active'
                        ? 'bg-success/15 text-success border border-success/30'
                        : String(rawVal) === 'pending' || String(rawVal) === 'processing'
                        ? 'bg-warning/15 text-warning border border-warning/30'
                        : String(rawVal) === 'failed'
                        ? 'bg-danger/15 text-danger border border-danger/30'
                        : 'bg-primary/10 text-primary border border-primary/20'
                    }`}
                  >
                    {String(rawVal)}
                  </span>
                ) : isFile ? (
                  <div className="flex items-center space-x-3 mt-1">
                    <div className="w-16 h-16 rounded-bn bg-content-box border border-bn-border overflow-hidden flex items-center justify-center">
                      <img
                        src={`/media/${rawVal}`}
                        alt="Media Preview"
                        className="w-full h-full object-cover cursor-pointer hover:opacity-90 transition-opacity"
                        onClick={() => setLightboxUrl(`/media/${rawVal}`)}
                        onError={(e) => {
                          (e.target as HTMLElement).style.display = 'none'
                        }}
                      />
                      <ImageIcon className="w-6 h-6 text-icon-muted" />
                    </div>
                    <div>
                      <p className="text-xs font-mono text-foreground break-all">{String(rawVal)}</p>
                      <a
                        href={`/media/${rawVal}`}
                        target="_blank"
                        rel="noreferrer"
                        className="text-xs text-primary hover:underline flex items-center space-x-1 mt-1"
                      >
                        <span>Open raw media</span>
                        <ExternalLink className="w-3 h-3" />
                      </a>
                    </div>
                  </div>
                ) : typeof rawVal === 'object' ? (
                  <pre className="text-xs font-mono bg-content-box p-3 rounded-bn border border-bn-border text-foreground overflow-x-auto">
                    {JSON.stringify(rawVal, null, 2)}
                  </pre>
                ) : (
                  <div className="text-sm font-medium text-foreground break-words">
                    {String(rawVal)}
                  </div>
                )}
              </div>
            )
          })}
        </div>

        {/* Footer */}
        <div className="h-16 px-6 border-t border-bn-border bg-content-bg/40 flex items-center justify-end space-x-3">
          <button
            onClick={onClose}
            className="px-4 py-2 text-sm font-medium text-foreground bg-content-box border border-bn-border hover:bg-content-bg rounded-bn transition-colors shadow-sm"
          >
            Close
          </button>
        </div>
      </div>

      {/* Media Lightbox Modal */}
      {lightboxUrl && (
        <div
          className="fixed inset-0 bg-black/80 z-[60] flex items-center justify-center p-4 backdrop-blur-md"
          onClick={() => setLightboxUrl(null)}
        >
          <div className="relative max-w-4xl max-h-[90vh] bg-content-box rounded-bn p-2 border border-bn-border shadow-2xl">
            <button
              onClick={() => setLightboxUrl(null)}
              className="absolute -top-3 -right-3 p-1.5 bg-content-box border border-bn-border text-foreground rounded-full shadow-md hover:scale-110 transition-transform"
            >
              <X className="w-4 h-4" />
            </button>
            <img
              src={lightboxUrl}
              alt="Expanded media preview"
              className="max-w-full max-h-[85vh] object-contain rounded-bn"
            />
          </div>
        </div>
      )}
    </>
  )
}

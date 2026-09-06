import React from 'react'

interface State {
  error: Error | null
}

/**
 * Top-level error boundary. A render-time crash in any view shows a recoverable
 * message instead of a blank white screen — production hardening so one bad render
 * doesn't take down the whole admin. A real deployment can forward the error to an
 * error tracker from `componentDidCatch`.
 */
export class ErrorBoundary extends React.Component<{ children: React.ReactNode }, State> {
  state: State = { error: null }

  static getDerivedStateFromError(error: Error): State {
    return { error }
  }

  componentDidCatch(error: Error, info: React.ErrorInfo) {
    // eslint-disable-next-line no-console
    console.error('Admin UI crashed:', error, info)
  }

  render() {
    if (this.state.error) {
      return (
        <div className="min-h-screen flex items-center justify-center bg-content-bg p-6">
          <div className="max-w-md w-full rounded-bn border border-bn-border bg-content-box p-8 text-center shadow-bn">
            <div className="w-12 h-12 rounded-full bg-danger/10 text-danger flex items-center justify-center mx-auto mb-4">
              <span className="text-xl font-bold">!</span>
            </div>
            <h1 className="text-lg font-semibold text-foreground mb-2">Something went wrong</h1>
            <p className="text-sm text-icon-muted mb-6">
              The admin hit an unexpected error and couldn’t render this view.
            </p>
            <pre className="mb-6 max-h-40 overflow-auto rounded-bn bg-content-bg border border-bn-border p-3 text-left text-xs font-mono text-danger">
              {this.state.error.message}
            </pre>
            <button
              onClick={() => window.location.reload()}
              className="rounded-bn bg-primary hover:bg-primary-hover px-4 py-2 text-sm font-medium text-white shadow-sm transition-all"
            >
              Reload
            </button>
          </div>
        </div>
      )
    }
    return this.props.children
  }
}

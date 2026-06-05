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
        <div className="min-h-screen flex items-center justify-center bg-gray-50 p-6">
          <div className="max-w-md w-full rounded-lg border border-gray-200 bg-white p-8 text-center shadow-sm">
            <h1 className="text-lg font-semibold text-gray-900 mb-2">Something went wrong</h1>
            <p className="text-sm text-gray-600 mb-6">
              The admin hit an unexpected error and couldn’t render this view.
            </p>
            <pre className="mb-6 max-h-40 overflow-auto rounded bg-gray-50 p-3 text-left text-xs text-gray-500">
              {this.state.error.message}
            </pre>
            <button
              onClick={() => window.location.reload()}
              className="rounded bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700"
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

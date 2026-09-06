import { useState } from 'react'
import { Sparkles, ArrowRight, ShieldCheck, AlertCircle, Loader2 } from 'lucide-react'
import { apiClient } from '../lib/api'

/**
 * Sign-in form. Posts to /auth/login, stores the JWT, then reloads at the admin
 * base so the app re-reads the token and renders the authenticated UI.
 */
export function Login() {
  const [email, setEmail] = useState('')
  const [password, setPassword] = useState('')
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(false)

  const onSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    setError(null)
    setLoading(true)
    try {
      await apiClient.login(email, password)
      window.location.href = (import.meta as any).env.BASE_URL
    } catch (err: any) {
      setError(
        err?.response?.data?.message ||
          err?.response?.data?.error ||
          (err?.response?.status === 401 ? 'Invalid email or password' : null) ||
          err?.message ||
          'Login failed',
      )
      setLoading(false)
    }
  }

  return (
    <div className="min-h-screen flex items-center justify-center bg-content-bg p-4 relative overflow-hidden">
      {/* Background ambient gradient glow */}
      <div className="absolute -top-40 -right-40 w-96 h-96 rounded-full bg-primary/10 blur-3xl pointer-events-none" />
      <div className="absolute -bottom-40 -left-40 w-96 h-96 rounded-full bg-primary-hover/10 blur-3xl pointer-events-none" />

      <div className="w-full max-w-md relative z-10">
        <div className="bg-content-box p-8 rounded-bn shadow-bn border border-bn-border backdrop-blur-sm">
          {/* Logo & Brand Header */}
          <div className="flex flex-col items-center text-center mb-8">
            <div className="w-12 h-12 rounded-bn bg-primary-gradient flex items-center justify-center text-white shadow-bn mb-4">
              <Sparkles className="w-6 h-6" />
            </div>
            <h1 className="text-2xl font-bold tracking-tight text-foreground">
              Dashin Admin
            </h1>
            <p className="text-sm text-icon-muted mt-1">
              Sign in to manage your cloud schema and services
            </p>
          </div>

          {error && (
            <div className="mb-6 rounded-bn border border-rose-500/20 bg-rose-500/10 p-3 text-sm text-rose-600 dark:text-rose-400 flex items-start space-x-2 animate-fade-in">
              <AlertCircle className="w-4 h-4 mt-0.5 flex-shrink-0" />
              <span>{error}</span>
            </div>
          )}

          <form onSubmit={onSubmit} className="space-y-5">
            <div>
              <label className="block text-xs font-semibold uppercase tracking-wider text-icon-muted mb-2">
                Email Address
              </label>
              <input
                type="email"
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                required
                autoFocus
                autoComplete="username"
                placeholder="admin@example.com"
                className="w-full rounded-bn border border-bn-border bg-content-box px-3.5 py-2.5 text-sm text-foreground placeholder:text-icon-muted focus:outline-none focus:ring-2 focus:ring-primary/20 focus:border-primary transition-colors"
              />
            </div>

            <div>
              <label className="block text-xs font-semibold uppercase tracking-wider text-icon-muted mb-2">
                Password
              </label>
              <input
                type="password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                required
                autoComplete="current-password"
                placeholder="••••••••"
                className="w-full rounded-bn border border-bn-border bg-content-box px-3.5 py-2.5 text-sm text-foreground placeholder:text-icon-muted focus:outline-none focus:ring-2 focus:ring-primary/20 focus:border-primary transition-colors"
              />
            </div>

            <button
              type="submit"
              disabled={loading}
              className="w-full mt-2 rounded-bn bg-primary hover:bg-primary-hover active:scale-[0.99] px-4 py-2.5 font-medium text-sm text-white shadow-sm transition-all flex items-center justify-center space-x-2 disabled:opacity-50 cursor-pointer disabled:cursor-not-allowed"
            >
              {loading ? (
                <>
                  <Loader2 className="w-4 h-4 animate-spin" />
                  <span>Signing in…</span>
                </>
              ) : (
                <>
                  <span>Sign In</span>
                  <ArrowRight className="w-4 h-4" />
                </>
              )}
            </button>
          </form>

          {/* Credentials Helper Box */}
          <div className="mt-8 pt-6 border-t border-bn-border text-center">
            <div className="inline-flex items-center space-x-1.5 text-xs text-icon-muted bg-content-bg/80 border border-bn-border px-3 py-1.5 rounded-bn">
              <ShieldCheck className="w-3.5 h-3.5 text-primary" />
              <span>Default admin:</span>
              <code className="font-mono text-foreground font-semibold">admin@example.com</code>
              <span>/</span>
              <code className="font-mono text-foreground font-semibold">change-me-too</code>
            </div>
          </div>
        </div>
      </div>
    </div>
  )
}


import { useState } from 'react'
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
    <div className="min-h-screen flex items-center justify-center bg-gray-50 p-4">
      <form
        onSubmit={onSubmit}
        className="w-full max-w-sm bg-white p-8 rounded-lg shadow-md border border-gray-100"
      >
        <h1 className="text-xl font-semibold text-gray-900 mb-1">Atomo Admin</h1>
        <p className="text-sm text-gray-500 mb-6">Sign in to continue</p>

        {error && (
          <div className="mb-4 rounded border border-red-200 bg-red-50 px-3 py-2 text-sm text-red-700">
            {error}
          </div>
        )}

        <label className="block mb-3">
          <span className="text-sm font-medium text-gray-700">Email</span>
          <input
            type="email"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            required
            autoFocus
            autoComplete="username"
            placeholder="admin@example.com"
            className="mt-1 w-full rounded border border-gray-300 px-3 py-2 focus:outline-none focus:ring-2 focus:ring-blue-500"
          />
        </label>

        <label className="block mb-6">
          <span className="text-sm font-medium text-gray-700">Password</span>
          <input
            type="password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            required
            autoComplete="current-password"
            placeholder="change-me-too"
            className="mt-1 w-full rounded border border-gray-300 px-3 py-2 focus:outline-none focus:ring-2 focus:ring-blue-500"
          />
        </label>

        <button
          type="submit"
          disabled={loading}
          className="w-full rounded bg-blue-600 px-3 py-2 font-medium text-white transition hover:bg-blue-700 disabled:opacity-50"
        >
          {loading ? 'Signing in…' : 'Sign in'}
        </button>

        <div className="mt-4 text-center text-xs text-gray-500">
          Default admin: <span className="font-mono text-gray-700">admin@example.com</span> / <span className="font-mono text-gray-700">change-me-too</span>
        </div>
      </form>
    </div>
  )
}

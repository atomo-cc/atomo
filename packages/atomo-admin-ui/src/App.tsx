import { useEffect, useState } from 'react'
import { DynamicRenderer, useRouteParser } from './components/DynamicRenderer'
import { Navigation } from './components/Navigation'
import { TopBar } from './components/TopBar'
import { Login } from './components/Login'
import { Toaster } from './components/ui/Toast'
import { apiClient, AuthUser } from './lib/api'
import { initializeServicePlugins } from './lib/service-plugin-loader'
import './index.css'

// Initialize service plugins
initializeServicePlugins()

/**
 * Atomo Admin UI — powered by Dashin Design System.
 */
function App() {
  const route = useRouteParser()
  const [authState, setAuthState] = useState<'checking' | 'authed' | 'unauthed'>('checking')
  const [user, setUser] = useState<AuthUser | null>(null)
  const [isMobileOpen, setIsMobileOpen] = useState(false)

  // Initialize theme from storage
  useEffect(() => {
    const saved = localStorage.getItem('dashin_theme')
    if (saved === 'dark' || (!saved && window.matchMedia('(prefers-color-scheme: dark)').matches)) {
      document.documentElement.classList.add('dark')
    } else {
      document.documentElement.classList.remove('dark')
    }
  }, [])

  // Validate session on mount
  useEffect(() => {
    if (!apiClient.isAuthenticated()) {
      setAuthState('unauthed')
      return
    }
    apiClient
      .getCurrentUser()
      .then((u) => {
        setUser(u)
        apiClient.currentUser = u
        setAuthState('authed')
      })
      .catch(() => {
        apiClient.logout()
        setAuthState('unauthed')
      })
  }, [])

  if (authState === 'checking') {
    return (
      <div className="min-h-screen flex items-center justify-center bg-content-bg text-foreground">
        <div className="flex flex-col items-center space-y-3">
          <div className="w-8 h-8 rounded-full border-2 border-primary border-t-transparent animate-spin" />
          <div className="text-xs text-icon-muted font-medium">Loading Dashin Admin…</div>
        </div>
      </div>
    )
  }

  // Gate behind authentication
  if (authState === 'unauthed') {
    return <Login />
  }

  return (
    <div className="min-h-screen bg-content-bg text-foreground transition-colors">
      {/* Dashin Sidebar Navigation */}
      <Navigation
        user={user}
        isMobileOpen={isMobileOpen}
        onCloseMobileMenu={() => setIsMobileOpen(false)}
      />

      {/* Main Content Area */}
      <div className="lg:pl-64 flex flex-col min-h-screen transition-all">
        {/* Dashin TopBar */}
        <TopBar
          user={user}
          onToggleMobileMenu={() => setIsMobileOpen((prev) => !prev)}
        />

        {/* Dynamic Route Content */}
        <main className="flex-1 overflow-x-hidden">
          <DynamicRenderer route={route} />
        </main>
      </div>

      <Toaster />
    </div>
  )
}

export default App

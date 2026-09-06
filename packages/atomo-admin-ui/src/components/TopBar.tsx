import React, { useState, useEffect } from 'react'
import { Link, useLocation } from 'react-router-dom'
import { Sun, Moon, LogOut, ChevronRight, Menu } from 'lucide-react'
import { AuthUser, apiClient } from '../lib/api'

interface TopBarProps {
  user?: AuthUser | null
  onToggleMobileMenu?: () => void
}

export function TopBar({ user, onToggleMobileMenu }: TopBarProps) {
  const location = useLocation()
  const [isDark, setIsDark] = useState<boolean>(() => {
    if (typeof window !== 'undefined') {
      return (
        localStorage.getItem('dashin_theme') === 'dark' ||
        (!('dashin_theme' in localStorage) && window.matchMedia('(prefers-color-scheme: dark)').matches)
      )
    }
    return false
  })
  const [userMenuOpen, setUserMenuOpen] = useState(false)

  useEffect(() => {
    if (isDark) {
      document.documentElement.classList.add('dark')
      localStorage.setItem('dashin_theme', 'dark')
    } else {
      document.documentElement.classList.remove('dark')
      localStorage.setItem('dashin_theme', 'light')
    }
  }, [isDark])

  const toggleTheme = () => setIsDark((prev) => !prev)

  const handleSignOut = () => {
    apiClient.logout()
    window.location.href = `${(import.meta as any).env.BASE_URL}login`
  }

  const rawSegments = location.pathname
    .replace(new RegExp(`^${(import.meta as any).env.BASE_URL || '/'}`), '')
    .split('/')
    .filter(Boolean)

  const breadcrumb = rawSegments.length > 0 ? rawSegments : ['Dashboard']

  return (
    <header className="sticky top-0 z-30 h-16 bg-content-box/90 backdrop-blur border-b border-bn-border px-4 sm:px-6 flex items-center justify-between transition-colors">
      <div className="flex items-center space-x-3">
        {onToggleMobileMenu && (
          <button
            onClick={onToggleMobileMenu}
            className="lg:hidden p-2 rounded-bn text-icon-muted hover:text-foreground hover:bg-content-bg transition-colors"
            title="Toggle Menu"
          >
            <Menu className="w-5 h-5" />
          </button>
        )}

        <nav className="flex items-center space-x-2 text-sm font-medium">
          <Link
            to="/"
            className="text-icon-muted hover:text-foreground transition-colors"
          >
            Dashboard
          </Link>
          {breadcrumb[0] !== 'Dashboard' &&
            breadcrumb.map((seg, idx) => (
              <React.Fragment key={idx}>
                <ChevronRight className="w-4 h-4 text-icon-muted opacity-60" />
                <span className={idx === breadcrumb.length - 1 ? 'text-primary font-semibold capitalize' : 'text-icon-muted capitalize'}>
                  {decodeURIComponent(seg)}
                </span>
              </React.Fragment>
            ))}
        </nav>
      </div>

      <div className="flex items-center space-x-3 sm:space-x-4">
        <div className="hidden md:flex items-center space-x-1.5 px-2.5 py-1 rounded-full text-xs bg-success/10 text-success border border-success/20">
          <span className="relative flex h-2 w-2">
            <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-success opacity-75"></span>
            <span className="relative inline-flex rounded-full h-2 w-2 bg-success"></span>
          </span>
          <span className="font-medium">Atomo v0.6.4</span>
        </div>

        <button
          onClick={toggleTheme}
          className="p-2 rounded-bn text-icon-muted hover:text-foreground hover:bg-content-bg border border-bn-border transition-all shadow-sm"
          title={isDark ? 'Switch to Light Mode' : 'Switch to Dark Mode'}
        >
          {isDark ? <Sun className="w-4 h-4 text-warning" /> : <Moon className="w-4 h-4 text-primary" />}
        </button>

        <div className="relative">
          <button
            onClick={() => setUserMenuOpen((prev) => !prev)}
            className="flex items-center space-x-2 p-1.5 rounded-bn hover:bg-content-bg transition-colors border border-transparent hover:border-bn-border"
          >
            <div className="w-8 h-8 rounded-full bg-primary-gradient flex items-center justify-center text-white font-semibold text-xs shadow-bn">
              {(user?.email || 'A').charAt(0).toUpperCase()}
            </div>
            <div className="hidden sm:block text-left text-xs">
              <div className="font-medium text-foreground truncate max-w-[120px]">
                {user?.email || 'Admin'}
              </div>
              <div className="text-icon-muted capitalize text-[10px]">
                {user?.role || 'Administrator'}
              </div>
            </div>
          </button>

          {userMenuOpen && (
            <>
              <div
                className="fixed inset-0 z-40"
                onClick={() => setUserMenuOpen(false)}
              />
              <div className="absolute right-0 mt-2 w-56 bg-content-box rounded-bn border border-bn-border shadow-bn z-50 p-2 text-sm animate-fade-in">
                <div className="px-3 py-2 border-b border-bn-border mb-1">
                  <p className="text-xs text-icon-muted">Signed in as</p>
                  <p className="font-semibold text-foreground truncate">{user?.email}</p>
                  <span className="inline-block mt-1 px-2 py-0.5 text-[10px] font-medium rounded-full bg-primary/10 text-primary border border-primary/20 capitalize">
                    {user?.role || 'admin'}
                  </span>
                </div>

                <button
                  onClick={handleSignOut}
                  className="w-full flex items-center space-x-2 px-3 py-2 text-xs font-medium text-danger hover:bg-danger/10 rounded-bn transition-colors"
                >
                  <LogOut className="w-4 h-4" />
                  <span>Sign out</span>
                </button>
              </div>
            </>
          )}
        </div>
      </div>
    </header>
  )
}

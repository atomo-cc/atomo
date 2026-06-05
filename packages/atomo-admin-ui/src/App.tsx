import { useEffect, useState } from 'react'
import { DynamicRenderer, useRouteParser } from './components/DynamicRenderer'
import { Navigation } from './components/Navigation'
import { Login } from './components/Login'
import { apiClient, AuthUser } from './lib/api'
import { initializeServicePlugins } from './lib/service-plugin-loader'
import './index.css'

// Initialize service plugins
initializeServicePlugins()

/**
 * Atomo Admin UI - 下一代动态管理界面
 *
 * 基于 Schema 驱动的动态渲染系统，实现：
 * - 零配置的 CRUD 界面生成
 * - 实时协作能力
 * - 可扩展的组件系统
 */
function App() {
  const route = useRouteParser()
  const [authState, setAuthState] = useState<'checking' | 'authed' | 'unauthed'>('checking')
  const [user, setUser] = useState<AuthUser | null>(null)

  // Validate the session on mount (covers refresh + reopen): a stored token only
  // means "maybe signed in" — confirm it against /auth/me and fetch the real user.
  // An expired/revoked token then shows the login form cleanly, instead of a
  // half-rendered admin that 401s on its first data fetch.
  useEffect(() => {
    if (!apiClient.isAuthenticated()) {
      setAuthState('unauthed')
      return
    }
    apiClient
      .getCurrentUser()
      .then((u) => {
        setUser(u)
        setAuthState('authed')
      })
      .catch(() => {
        apiClient.logout()
        setAuthState('unauthed')
      })
  }, [])

  if (authState === 'checking') {
    return (
      <div className="min-h-screen flex items-center justify-center bg-gray-50">
        <div className="text-sm text-gray-500">Loading…</div>
      </div>
    )
  }

  // Gate the whole admin behind a *validated* sign-in.
  if (authState === 'unauthed') {
    return <Login />
  }

  return (
    <div className="min-h-screen bg-gray-50">
      {/* 导航栏 */}
      <Navigation user={user} />

      {/* 主内容区 */}
      <main className="lg:pl-64">
        <DynamicRenderer route={route} />
      </main>
    </div>
  )
}

export default App

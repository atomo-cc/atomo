import { DynamicRenderer, useRouteParser } from './components/DynamicRenderer'
import { Navigation } from './components/Navigation'
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

  return (
    <div className="min-h-screen bg-gray-50">
      {/* 导航栏 */}
      <Navigation />

      {/* 主内容区 */}
      <main className="lg:pl-64">
        <DynamicRenderer route={route} />
      </main>
    </div>
  )
}

export default App

import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [react()],
  // 🎯 开发模式：保持简单的根路径配置
  // 通过后端代理实现 http://localhost:3001/admin 访问
  build: {
    outDir: 'dist',
    assetsDir: 'assets',
    // 生产构建时使用admin路径
    // base会在构建脚本中动态设置
  },
  server: {
    port: 5173,
    host: true,
    // 开发模式下运行在根路径，通过后端代理访问
  },
})

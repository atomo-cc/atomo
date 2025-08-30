import React from 'react'
import { Outlet } from 'react-router-dom'
import Sidebar from './Sidebar'
import Header from './Header'

/**
 * Admin UI Layout - Universal layout for Atomo admin interfaces
 * 
 * This layout is designed to be:
 * - Platform-neutral (no specific branding)
 * - Highly customizable through CSS variables
 * - Responsive and accessible
 */
export default function Layout({ children }: { children: React.ReactNode }) {
  return (
    <div className="admin-layout">
      <Sidebar />
      <div className="admin-main">
        <Header />
        <main className="admin-content">
          {children || <Outlet />}
        </main>
      </div>
    </div>
  )
}

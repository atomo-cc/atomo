import React from 'react'
import { Routes, Route } from 'react-router-dom'
import Layout from './components/Layout'
import Dashboard from './pages/Dashboard'
import EntityList from './pages/EntityList'
import EntityDetail from './pages/EntityDetail'

/**
 * Atomo Admin UI - Universal admin interface
 * 
 * This component dynamically renders admin interfaces based on
 * the schema metadata from Atomo Core.
 */
function App() {
  return (
    <Layout>
      <Routes>
        <Route path="/" element={<Dashboard />} />
        <Route path="/entities/:entityType" element={<EntityList />} />
        <Route path="/entities/:entityType/:entityId" element={<EntityDetail />} />
      </Routes>
    </Layout>
  )
}

export default App

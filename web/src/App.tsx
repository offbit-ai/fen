import { Routes, Route, Navigate } from 'react-router-dom'
import { LoginPage } from '@/pages/auth/login'
import { DashboardPage } from '@/pages/dashboard'
import { AppShell } from '@/components/layout/app-shell'

function App() {
  return (
    <Routes>
      {/* Auth routes */}
      <Route path="/login" element={<LoginPage />} />

      {/* Protected routes */}
      <Route path="/" element={<AppShell />}>
        <Route index element={<Navigate to="/dashboard" replace />} />
        <Route path="dashboard" element={<DashboardPage />} />
      </Route>

      {/* Catch all */}
      <Route path="*" element={<Navigate to="/dashboard" replace />} />
    </Routes>
  )
}

export default App

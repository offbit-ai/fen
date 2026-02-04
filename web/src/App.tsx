import { Routes, Route, Navigate } from 'react-router-dom'
import { LoginPage } from '@/pages/auth/login'
import { DashboardPage } from '@/pages/dashboard'
import { InvoicesPage } from '@/pages/documents/invoices'
import { InvoiceDetailPage } from '@/pages/documents/invoice-detail'
import { QueryWorkbenchPage } from '@/pages/analysis/workbench'
import { AnomaliesPage } from '@/pages/analysis/anomalies'
import { AppShell } from '@/components/layout/app-shell'
import { ProtectedRoute } from '@/components/auth/protected-route'

function App() {
  // For development, we skip auth. In production, wrap AppShell with ProtectedRoute
  const isDev = import.meta.env.DEV

  return (
    <Routes>
      {/* Auth routes */}
      <Route path="/login" element={<LoginPage />} />

      {/* Protected routes */}
      <Route
        path="/"
        element={
          isDev ? (
            <AppShell />
          ) : (
            <ProtectedRoute>
              <AppShell />
            </ProtectedRoute>
          )
        }
      >
        <Route index element={<Navigate to="/dashboard" replace />} />
        <Route path="dashboard" element={<DashboardPage />} />

        {/* Documents */}
        <Route path="documents">
          <Route path="invoices" element={<InvoicesPage />} />
          <Route path="invoices/:id" element={<InvoiceDetailPage />} />
          <Route path="contracts" element={<PlaceholderPage title="Contracts" />} />
        </Route>

        {/* Analysis */}
        <Route path="analysis">
          <Route path="workbench" element={<QueryWorkbenchPage />} />
          <Route path="anomalies" element={<AnomaliesPage />} />
          <Route path="anomalies/:id" element={<PlaceholderPage title="Anomaly Detail" />} />
          <Route path="reports" element={<PlaceholderPage title="Reports" />} />
          <Route path="baselines" element={<PlaceholderPage title="Baselines" />} />
        </Route>

        {/* Rules Engine */}
        <Route path="rules">
          <Route path="graph" element={<PlaceholderPage title="Rule Graph" />} />
          <Route path="tables" element={<PlaceholderPage title="Decision Tables" />} />
          <Route path="testing" element={<PlaceholderPage title="Rule Testing" />} />
          <Route path="history" element={<PlaceholderPage title="Execution History" />} />
        </Route>

        {/* Settings */}
        <Route path="settings">
          <Route path="users" element={<PlaceholderPage title="Users" />} />
          <Route path="tenant" element={<PlaceholderPage title="Tenant Settings" />} />
          <Route path="notifications" element={<PlaceholderPage title="Notifications" />} />
          <Route path="integrations" element={<PlaceholderPage title="Integrations" />} />
          <Route path="audit" element={<PlaceholderPage title="Audit Log" />} />
        </Route>
      </Route>

      {/* Catch all */}
      <Route path="*" element={<Navigate to="/dashboard" replace />} />
    </Routes>
  )
}

// Placeholder component for unimplemented pages
function PlaceholderPage({ title }: { title: string }) {
  return (
    <div className="flex flex-col items-center justify-center py-20">
      <div className="rounded-lg border-2 border-dashed border-primary-200 p-12 text-center">
        <h1 className="text-2xl font-bold text-primary-900">{title}</h1>
        <p className="mt-2 text-primary-500">This page is coming soon.</p>
      </div>
    </div>
  )
}

export default App

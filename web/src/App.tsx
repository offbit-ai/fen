import { Routes, Route, Navigate } from 'react-router-dom'
import { LoginPage } from '@/pages/auth/login'
import { DashboardPage } from '@/pages/dashboard'
import { InvoicesPage } from '@/pages/documents/invoices'
import { InvoiceDetailPage } from '@/pages/documents/invoice-detail'
import { ContractsPage } from '@/pages/documents/contracts'
import { ContractDetailPage } from '@/pages/documents/contract-detail'
import { QueryWorkbenchPage } from '@/pages/analysis/workbench'
import { AnomaliesPage } from '@/pages/analysis/anomalies'
import { AnomalyDetailPage } from '@/pages/analysis/anomaly-detail'
import { ReportsPage } from '@/pages/analysis/reports'
import { BaselinesPage } from '@/pages/analysis/baselines'
import { DecisionTablesPage } from '@/pages/rules/tables'
import { RuleGraphPage } from '@/pages/rules/graph'
import { RuleTestingPage } from '@/pages/rules/testing'
import { ExecutionHistoryPage } from '@/pages/rules/history'
import { UsersPage } from '@/pages/settings/users'
import { TenantSettingsPage } from '@/pages/settings/tenant'
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
          <Route path="contracts" element={<ContractsPage />} />
          <Route path="contracts/:id" element={<ContractDetailPage />} />
          <Route path="attachments" element={<PlaceholderPage title="Attachments" />} />
        </Route>

        {/* Analysis */}
        <Route path="analysis">
          <Route path="workbench" element={<QueryWorkbenchPage />} />
          <Route path="anomalies" element={<AnomaliesPage />} />
          <Route path="anomalies/:id" element={<AnomalyDetailPage />} />
          <Route path="reports" element={<ReportsPage />} />
          <Route path="baselines" element={<BaselinesPage />} />
        </Route>

        {/* Rules Engine */}
        <Route path="rules">
          <Route path="graph" element={<RuleGraphPage />} />
          <Route path="tables" element={<DecisionTablesPage />} />
          <Route path="testing" element={<RuleTestingPage />} />
          <Route path="history" element={<ExecutionHistoryPage />} />
        </Route>

        {/* Settings */}
        <Route path="settings">
          <Route path="users" element={<UsersPage />} />
          <Route path="tenant" element={<TenantSettingsPage />} />
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

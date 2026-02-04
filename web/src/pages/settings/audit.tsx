import { useState } from 'react'
import {
  History,
  Search,
  Download,
  User,
  FileText,
  Settings,
  Shield,
  AlertTriangle,
  CheckCircle,
  XCircle,
  Clock,
  ChevronLeft,
  ChevronRight,
  Eye,
} from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Card, CardContent } from '@/components/ui/card'
import { cn } from '@/lib/utils'

type AuditAction = 'create' | 'update' | 'delete' | 'view' | 'login' | 'logout' | 'export' | 'config_change'
type AuditCategory = 'document' | 'user' | 'rule' | 'system' | 'auth' | 'settings'
type AuditOutcome = 'success' | 'failure' | 'warning'

interface AuditEntry {
  id: string
  timestamp: string
  user_id: string
  user_name: string
  user_email: string
  action: AuditAction
  category: AuditCategory
  resource_type: string
  resource_id?: string
  description: string
  outcome: AuditOutcome
  ip_address: string
  user_agent?: string
  metadata?: Record<string, unknown>
}

const mockAuditLog: AuditEntry[] = [
  {
    id: 'aud-001',
    timestamp: '2024-02-04T14:32:15Z',
    user_id: 'usr-001',
    user_name: 'Sarah Anderson',
    user_email: 'sarah.anderson@company.com',
    action: 'update',
    category: 'rule',
    resource_type: 'ValidationRule',
    resource_id: 'rule-123',
    description: 'Updated validation rule "Invoice Amount Check"',
    outcome: 'success',
    ip_address: '192.168.1.100',
    metadata: { changes: { threshold: { old: 1000, new: 5000 } } },
  },
  {
    id: 'aud-002',
    timestamp: '2024-02-04T14:28:00Z',
    user_id: 'usr-002',
    user_name: 'John Smith',
    user_email: 'john.smith@company.com',
    action: 'export',
    category: 'document',
    resource_type: 'Invoice',
    description: 'Exported 156 invoices to CSV',
    outcome: 'success',
    ip_address: '192.168.1.101',
  },
  {
    id: 'aud-003',
    timestamp: '2024-02-04T14:15:30Z',
    user_id: 'usr-003',
    user_name: 'Emily Chen',
    user_email: 'emily.chen@company.com',
    action: 'login',
    category: 'auth',
    resource_type: 'Session',
    description: 'User logged in via SSO (Okta)',
    outcome: 'success',
    ip_address: '192.168.1.102',
    user_agent: 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)',
  },
  {
    id: 'aud-004',
    timestamp: '2024-02-04T13:45:00Z',
    user_id: 'usr-001',
    user_name: 'Sarah Anderson',
    user_email: 'sarah.anderson@company.com',
    action: 'delete',
    category: 'document',
    resource_type: 'Invoice',
    resource_id: 'inv-456',
    description: 'Deleted invoice INV-2024-0456',
    outcome: 'success',
    ip_address: '192.168.1.100',
  },
  {
    id: 'aud-005',
    timestamp: '2024-02-04T13:30:00Z',
    user_id: 'usr-004',
    user_name: 'Michael Brown',
    user_email: 'michael.brown@company.com',
    action: 'login',
    category: 'auth',
    resource_type: 'Session',
    description: 'Failed login attempt - invalid password',
    outcome: 'failure',
    ip_address: '192.168.1.103',
  },
  {
    id: 'aud-006',
    timestamp: '2024-02-04T12:00:00Z',
    user_id: 'sys-001',
    user_name: 'System',
    user_email: 'system@fen.internal',
    action: 'config_change',
    category: 'system',
    resource_type: 'TenantConfig',
    description: 'Retention policy updated: hot_retention_days changed from 30 to 60',
    outcome: 'success',
    ip_address: '127.0.0.1',
  },
  {
    id: 'aud-007',
    timestamp: '2024-02-04T11:30:00Z',
    user_id: 'usr-002',
    user_name: 'John Smith',
    user_email: 'john.smith@company.com',
    action: 'view',
    category: 'document',
    resource_type: 'Contract',
    resource_id: 'ctr-789',
    description: 'Viewed contract CTR-2024-0789',
    outcome: 'success',
    ip_address: '192.168.1.101',
  },
  {
    id: 'aud-008',
    timestamp: '2024-02-04T10:00:00Z',
    user_id: 'usr-001',
    user_name: 'Sarah Anderson',
    user_email: 'sarah.anderson@company.com',
    action: 'create',
    category: 'user',
    resource_type: 'User',
    resource_id: 'usr-005',
    description: 'Created new user account for david.kim@company.com',
    outcome: 'success',
    ip_address: '192.168.1.100',
  },
]

const actionConfig: Record<AuditAction, { label: string; icon: React.ReactNode }> = {
  create: { label: 'Create', icon: <FileText className="h-4 w-4" /> },
  update: { label: 'Update', icon: <Settings className="h-4 w-4" /> },
  delete: { label: 'Delete', icon: <XCircle className="h-4 w-4" /> },
  view: { label: 'View', icon: <Eye className="h-4 w-4" /> },
  login: { label: 'Login', icon: <User className="h-4 w-4" /> },
  logout: { label: 'Logout', icon: <User className="h-4 w-4" /> },
  export: { label: 'Export', icon: <Download className="h-4 w-4" /> },
  config_change: { label: 'Config Change', icon: <Shield className="h-4 w-4" /> },
}

const categoryConfig: Record<AuditCategory, { label: string; className: string }> = {
  document: { label: 'Document', className: 'bg-blue-100 text-blue-700' },
  user: { label: 'User', className: 'bg-purple-100 text-purple-700' },
  rule: { label: 'Rule', className: 'bg-green-100 text-green-700' },
  system: { label: 'System', className: 'bg-gray-100 text-gray-700' },
  auth: { label: 'Auth', className: 'bg-yellow-100 text-yellow-700' },
  settings: { label: 'Settings', className: 'bg-orange-100 text-orange-700' },
}

const outcomeConfig: Record<AuditOutcome, { icon: React.ReactNode; className: string }> = {
  success: { icon: <CheckCircle className="h-4 w-4" />, className: 'text-green-600' },
  failure: { icon: <XCircle className="h-4 w-4" />, className: 'text-red-600' },
  warning: { icon: <AlertTriangle className="h-4 w-4" />, className: 'text-yellow-600' },
}

export function AuditLogPage() {
  const [searchQuery, setSearchQuery] = useState('')
  const [categoryFilter, setCategoryFilter] = useState<AuditCategory | 'all'>('all')
  const [outcomeFilter, setOutcomeFilter] = useState<AuditOutcome | 'all'>('all')
  const [expandedEntry, setExpandedEntry] = useState<string | null>(null)

  const filteredLog = mockAuditLog.filter((entry) => {
    if (categoryFilter !== 'all' && entry.category !== categoryFilter) return false
    if (outcomeFilter !== 'all' && entry.outcome !== outcomeFilter) return false
    if (searchQuery) {
      const query = searchQuery.toLowerCase()
      return (
        entry.description.toLowerCase().includes(query) ||
        entry.user_name.toLowerCase().includes(query) ||
        entry.user_email.toLowerCase().includes(query)
      )
    }
    return true
  })

  // Stats
  const todayCount = mockAuditLog.filter((e) => {
    const today = new Date().toDateString()
    return new Date(e.timestamp).toDateString() === today
  }).length

  const failureCount = mockAuditLog.filter((e) => e.outcome === 'failure').length

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-primary-900">Audit Log</h1>
          <p className="text-sm text-primary-500">
            Track all user and system activities
          </p>
        </div>
        <Button variant="outline">
          <Download className="mr-2 h-4 w-4" />
          Export Log
        </Button>
      </div>

      {/* Stats */}
      <div className="grid gap-4 sm:grid-cols-3">
        <Card>
          <CardContent className="p-4">
            <div className="flex items-center gap-3">
              <div className="rounded-lg bg-primary-100 p-2">
                <History className="h-5 w-5 text-primary-600" />
              </div>
              <div>
                <p className="text-sm text-primary-500">Total Events</p>
                <p className="text-2xl font-bold text-primary-900">{mockAuditLog.length}</p>
              </div>
            </div>
          </CardContent>
        </Card>
        <Card>
          <CardContent className="p-4">
            <div className="flex items-center gap-3">
              <div className="rounded-lg bg-blue-100 p-2">
                <Clock className="h-5 w-5 text-blue-600" />
              </div>
              <div>
                <p className="text-sm text-primary-500">Today</p>
                <p className="text-2xl font-bold text-primary-900">{todayCount}</p>
              </div>
            </div>
          </CardContent>
        </Card>
        <Card>
          <CardContent className="p-4">
            <div className="flex items-center gap-3">
              <div className="rounded-lg bg-red-100 p-2">
                <AlertTriangle className="h-5 w-5 text-red-600" />
              </div>
              <div>
                <p className="text-sm text-primary-500">Failures</p>
                <p className="text-2xl font-bold text-primary-900">{failureCount}</p>
              </div>
            </div>
          </CardContent>
        </Card>
      </div>

      {/* Filters */}
      <Card className="p-4">
        <div className="flex flex-col gap-4 sm:flex-row sm:items-center">
          <div className="relative flex-1 max-w-md">
            <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-primary-400" />
            <Input
              type="search"
              placeholder="Search activities..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="pl-10"
            />
          </div>
          <div className="flex items-center gap-2">
            <select
              value={categoryFilter}
              onChange={(e) => setCategoryFilter(e.target.value as AuditCategory | 'all')}
              className="h-10 rounded-lg border border-primary-200 bg-white px-3 text-sm"
            >
              <option value="all">All Categories</option>
              <option value="document">Document</option>
              <option value="user">User</option>
              <option value="rule">Rule</option>
              <option value="auth">Auth</option>
              <option value="system">System</option>
              <option value="settings">Settings</option>
            </select>
            <select
              value={outcomeFilter}
              onChange={(e) => setOutcomeFilter(e.target.value as AuditOutcome | 'all')}
              className="h-10 rounded-lg border border-primary-200 bg-white px-3 text-sm"
            >
              <option value="all">All Outcomes</option>
              <option value="success">Success</option>
              <option value="failure">Failure</option>
              <option value="warning">Warning</option>
            </select>
          </div>
        </div>
      </Card>

      {/* Audit Log Table */}
      <Card>
        <CardContent className="p-0">
          <div className="overflow-x-auto">
            <table className="w-full">
              <thead>
                <tr className="border-b border-primary-200 bg-primary-50">
                  <th className="px-4 py-3 text-left text-sm font-medium text-primary-700">Timestamp</th>
                  <th className="px-4 py-3 text-left text-sm font-medium text-primary-700">User</th>
                  <th className="px-4 py-3 text-left text-sm font-medium text-primary-700">Action</th>
                  <th className="px-4 py-3 text-left text-sm font-medium text-primary-700">Category</th>
                  <th className="px-4 py-3 text-left text-sm font-medium text-primary-700">Description</th>
                  <th className="px-4 py-3 text-center text-sm font-medium text-primary-700">Outcome</th>
                  <th className="w-12 px-4 py-3"></th>
                </tr>
              </thead>
              <tbody>
                {filteredLog.map((entry) => (
                  <AuditRow
                    key={entry.id}
                    entry={entry}
                    expanded={expandedEntry === entry.id}
                    onToggle={() => setExpandedEntry(expandedEntry === entry.id ? null : entry.id)}
                  />
                ))}
              </tbody>
            </table>
          </div>
        </CardContent>
      </Card>

      {/* Pagination */}
      <div className="flex items-center justify-between">
        <p className="text-sm text-primary-500">
          Showing 1-{filteredLog.length} of {mockAuditLog.length} entries
        </p>
        <div className="flex items-center gap-2">
          <Button variant="outline" size="sm" disabled>
            <ChevronLeft className="h-4 w-4" />
          </Button>
          <Button variant="outline" size="sm" className="bg-primary-100">
            1
          </Button>
          <Button variant="outline" size="sm" disabled>
            <ChevronRight className="h-4 w-4" />
          </Button>
        </div>
      </div>
    </div>
  )
}

function AuditRow({
  entry,
  expanded,
  onToggle,
}: {
  entry: AuditEntry
  expanded: boolean
  onToggle: () => void
}) {
  const action = actionConfig[entry.action]
  const category = categoryConfig[entry.category]
  const outcome = outcomeConfig[entry.outcome]

  return (
    <>
      <tr
        className={cn(
          'border-b border-primary-100 hover:bg-primary-50 cursor-pointer',
          expanded && 'bg-primary-50',
          entry.outcome === 'failure' && 'bg-red-50/50'
        )}
        onClick={onToggle}
      >
        <td className="px-4 py-3 text-sm text-primary-600">
          {new Date(entry.timestamp).toLocaleString()}
        </td>
        <td className="px-4 py-3">
          <div>
            <p className="text-sm font-medium text-primary-900">{entry.user_name}</p>
            <p className="text-xs text-primary-500">{entry.user_email}</p>
          </div>
        </td>
        <td className="px-4 py-3">
          <div className="flex items-center gap-2">
            {action.icon}
            <span className="text-sm text-primary-700">{action.label}</span>
          </div>
        </td>
        <td className="px-4 py-3">
          <span className={cn('rounded-full px-2 py-0.5 text-xs font-medium', category.className)}>
            {category.label}
          </span>
        </td>
        <td className="px-4 py-3 text-sm text-primary-700 max-w-xs truncate">
          {entry.description}
        </td>
        <td className="px-4 py-3 text-center">
          <span className={outcome.className}>{outcome.icon}</span>
        </td>
        <td className="px-4 py-3">
          <Button variant="ghost" size="icon" className="h-8 w-8">
            <Eye className="h-4 w-4" />
          </Button>
        </td>
      </tr>
      {expanded && (
        <tr className="bg-primary-50">
          <td colSpan={7} className="px-4 py-4">
            <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4 text-sm">
              <div>
                <p className="text-xs font-medium text-primary-500 uppercase">Resource Type</p>
                <p className="text-primary-900">{entry.resource_type}</p>
              </div>
              {entry.resource_id && (
                <div>
                  <p className="text-xs font-medium text-primary-500 uppercase">Resource ID</p>
                  <p className="text-primary-900 font-mono">{entry.resource_id}</p>
                </div>
              )}
              <div>
                <p className="text-xs font-medium text-primary-500 uppercase">IP Address</p>
                <p className="text-primary-900 font-mono">{entry.ip_address}</p>
              </div>
              {entry.user_agent && (
                <div className="sm:col-span-2">
                  <p className="text-xs font-medium text-primary-500 uppercase">User Agent</p>
                  <p className="text-primary-900 text-xs truncate">{entry.user_agent}</p>
                </div>
              )}
              {entry.metadata && (
                <div className="sm:col-span-4">
                  <p className="text-xs font-medium text-primary-500 uppercase mb-1">Metadata</p>
                  <pre className="rounded bg-primary-100 p-2 text-xs overflow-x-auto">
                    {JSON.stringify(entry.metadata, null, 2)}
                  </pre>
                </div>
              )}
            </div>
          </td>
        </tr>
      )}
    </>
  )
}

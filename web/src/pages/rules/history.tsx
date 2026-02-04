import { useState } from 'react'
import { Link } from 'react-router-dom'
import {
  History,
  Search,
  Filter,
  Download,
  CheckCircle,
  XCircle,
  AlertTriangle,
  Clock,
  FileText,
  ChevronLeft,
  ChevronRight,
  RefreshCw,
  Eye,
} from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Card, CardContent } from '@/components/ui/card'
import { cn } from '@/lib/utils'

type ExecutionStatus = 'success' | 'partial' | 'failed'
type ValidationEngine = 'structural' | 'zen' | 'statistical' | 'all'

interface RuleExecution {
  id: string
  document_id: string
  document_type: 'invoice' | 'contract'
  invoice_number?: string
  vendor_name?: string
  executed_at: string
  duration_ms: number
  status: ExecutionStatus
  engines_run: ValidationEngine[]
  rules_evaluated: number
  anomalies_detected: number
  is_valid: boolean
  anomaly_types?: string[]
}

const mockExecutions: RuleExecution[] = [
  {
    id: 'exec-001',
    document_id: 'doc-456',
    document_type: 'invoice',
    invoice_number: 'INV-2024-889',
    vendor_name: 'TechParts Inc',
    executed_at: '2024-02-04T10:35:00Z',
    duration_ms: 45,
    status: 'partial',
    engines_run: ['structural', 'zen', 'statistical'],
    rules_evaluated: 8,
    anomalies_detected: 2,
    is_valid: false,
    anomaly_types: ['price_deviation', 'statistical_outlier'],
  },
  {
    id: 'exec-002',
    document_id: 'doc-457',
    document_type: 'invoice',
    invoice_number: 'INV-2024-890',
    vendor_name: 'GlobalSupply Co',
    executed_at: '2024-02-04T10:34:00Z',
    duration_ms: 38,
    status: 'success',
    engines_run: ['structural', 'zen', 'statistical'],
    rules_evaluated: 8,
    anomalies_detected: 0,
    is_valid: true,
  },
  {
    id: 'exec-003',
    document_id: 'doc-458',
    document_type: 'invoice',
    invoice_number: 'INV-2024-891',
    vendor_name: 'Acme Corp',
    executed_at: '2024-02-04T10:33:00Z',
    duration_ms: 52,
    status: 'partial',
    engines_run: ['structural', 'zen', 'statistical'],
    rules_evaluated: 8,
    anomalies_detected: 1,
    is_valid: false,
    anomaly_types: ['potential_duplicate'],
  },
  {
    id: 'exec-004',
    document_id: 'doc-459',
    document_type: 'invoice',
    invoice_number: 'INV-2024-892',
    vendor_name: 'Office Supplies Ltd',
    executed_at: '2024-02-04T10:32:00Z',
    duration_ms: 35,
    status: 'success',
    engines_run: ['structural', 'zen'],
    rules_evaluated: 6,
    anomalies_detected: 0,
    is_valid: true,
  },
  {
    id: 'exec-005',
    document_id: 'doc-460',
    document_type: 'invoice',
    invoice_number: 'INV-2024-893',
    vendor_name: 'MegaLogistics',
    executed_at: '2024-02-04T10:31:00Z',
    duration_ms: 125,
    status: 'failed',
    engines_run: ['structural'],
    rules_evaluated: 3,
    anomalies_detected: 0,
    is_valid: false,
  },
  {
    id: 'exec-006',
    document_id: 'doc-461',
    document_type: 'invoice',
    invoice_number: 'INV-2024-894',
    vendor_name: 'TechParts Inc',
    executed_at: '2024-02-04T10:30:00Z',
    duration_ms: 42,
    status: 'partial',
    engines_run: ['structural', 'zen', 'statistical'],
    rules_evaluated: 8,
    anomalies_detected: 3,
    is_valid: false,
    anomaly_types: ['math_mismatch', 'missing_field', 'contract_violation'],
  },
  {
    id: 'exec-007',
    document_id: 'doc-462',
    document_type: 'contract',
    vendor_name: 'NewVendor LLC',
    executed_at: '2024-02-04T10:29:00Z',
    duration_ms: 28,
    status: 'success',
    engines_run: ['structural'],
    rules_evaluated: 4,
    anomalies_detected: 0,
    is_valid: true,
  },
  {
    id: 'exec-008',
    document_id: 'doc-463',
    document_type: 'invoice',
    invoice_number: 'INV-2024-895',
    vendor_name: 'GlobalSupply Co',
    executed_at: '2024-02-04T10:28:00Z',
    duration_ms: 40,
    status: 'success',
    engines_run: ['structural', 'zen', 'statistical'],
    rules_evaluated: 8,
    anomalies_detected: 0,
    is_valid: true,
  },
]

export function ExecutionHistoryPage() {
  const [searchQuery, setSearchQuery] = useState('')
  const [statusFilter, setStatusFilter] = useState<ExecutionStatus | 'all'>('all')
  const [dateRange, setDateRange] = useState('today')

  const filteredExecutions = mockExecutions.filter((e) => {
    if (statusFilter !== 'all' && e.status !== statusFilter) return false
    if (searchQuery) {
      const query = searchQuery.toLowerCase()
      return (
        e.invoice_number?.toLowerCase().includes(query) ||
        e.vendor_name?.toLowerCase().includes(query) ||
        e.document_id.toLowerCase().includes(query)
      )
    }
    return true
  })

  // Stats
  const stats = {
    total: mockExecutions.length,
    success: mockExecutions.filter((e) => e.status === 'success').length,
    partial: mockExecutions.filter((e) => e.status === 'partial').length,
    failed: mockExecutions.filter((e) => e.status === 'failed').length,
    avgDuration: Math.round(mockExecutions.reduce((sum, e) => sum + e.duration_ms, 0) / mockExecutions.length),
  }

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-primary-900">Execution History</h1>
          <p className="text-sm text-primary-500">
            Monitor rule execution results and performance
          </p>
        </div>
        <div className="flex items-center gap-2">
          <Button variant="outline">
            <Download className="mr-2 h-4 w-4" />
            Export
          </Button>
          <Button variant="outline">
            <RefreshCw className="mr-2 h-4 w-4" />
            Refresh
          </Button>
        </div>
      </div>

      {/* Stats */}
      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-5">
        <StatCard
          title="Total Executions"
          value={stats.total.toLocaleString()}
          icon={<History className="h-5 w-5" />}
        />
        <StatCard
          title="Passed"
          value={stats.success.toLocaleString()}
          icon={<CheckCircle className="h-5 w-5" />}
          color="green"
        />
        <StatCard
          title="With Anomalies"
          value={stats.partial.toLocaleString()}
          icon={<AlertTriangle className="h-5 w-5" />}
          color="yellow"
        />
        <StatCard
          title="Failed"
          value={stats.failed.toLocaleString()}
          icon={<XCircle className="h-5 w-5" />}
          color="red"
        />
        <StatCard
          title="Avg Duration"
          value={`${stats.avgDuration}ms`}
          icon={<Clock className="h-5 w-5" />}
        />
      </div>

      {/* Filters */}
      <Card className="p-4">
        <div className="flex flex-col gap-4 sm:flex-row sm:items-center">
          <div className="relative flex-1 max-w-md">
            <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-primary-400" />
            <Input
              type="search"
              placeholder="Search by invoice number, vendor..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="pl-10"
            />
          </div>
          <div className="flex items-center gap-2">
            <select
              value={statusFilter}
              onChange={(e) => setStatusFilter(e.target.value as ExecutionStatus | 'all')}
              className="h-10 rounded-lg border border-primary-200 bg-white px-3 text-sm"
            >
              <option value="all">All Statuses</option>
              <option value="success">Passed</option>
              <option value="partial">With Anomalies</option>
              <option value="failed">Failed</option>
            </select>
            <select
              value={dateRange}
              onChange={(e) => setDateRange(e.target.value)}
              className="h-10 rounded-lg border border-primary-200 bg-white px-3 text-sm"
            >
              <option value="today">Today</option>
              <option value="yesterday">Yesterday</option>
              <option value="week">Last 7 days</option>
              <option value="month">Last 30 days</option>
            </select>
            <Button variant="outline" size="sm">
              <Filter className="mr-2 h-4 w-4" />
              More Filters
            </Button>
          </div>
        </div>
      </Card>

      {/* Executions Table */}
      <Card>
        <CardContent className="p-0">
          <div className="overflow-x-auto">
            <table className="w-full">
              <thead>
                <tr className="border-b border-primary-200 bg-primary-50">
                  <th className="px-4 py-3 text-left text-sm font-medium text-primary-700">
                    Status
                  </th>
                  <th className="px-4 py-3 text-left text-sm font-medium text-primary-700">
                    Document
                  </th>
                  <th className="px-4 py-3 text-left text-sm font-medium text-primary-700">
                    Vendor
                  </th>
                  <th className="px-4 py-3 text-left text-sm font-medium text-primary-700">
                    Engines
                  </th>
                  <th className="px-4 py-3 text-center text-sm font-medium text-primary-700">
                    Rules
                  </th>
                  <th className="px-4 py-3 text-center text-sm font-medium text-primary-700">
                    Anomalies
                  </th>
                  <th className="px-4 py-3 text-right text-sm font-medium text-primary-700">
                    Duration
                  </th>
                  <th className="px-4 py-3 text-right text-sm font-medium text-primary-700">
                    Time
                  </th>
                  <th className="w-12 px-4 py-3"></th>
                </tr>
              </thead>
              <tbody>
                {filteredExecutions.map((execution) => (
                  <ExecutionRow key={execution.id} execution={execution} />
                ))}
              </tbody>
            </table>
          </div>
        </CardContent>
      </Card>

      {/* Pagination */}
      <div className="flex items-center justify-between">
        <p className="text-sm text-primary-500">
          Showing 1-{filteredExecutions.length} of {mockExecutions.length} executions
        </p>
        <div className="flex items-center gap-2">
          <Button variant="outline" size="sm" disabled>
            <ChevronLeft className="h-4 w-4" />
          </Button>
          <Button variant="outline" size="sm" className="bg-primary-100">
            1
          </Button>
          <Button variant="outline" size="sm">
            2
          </Button>
          <Button variant="outline" size="sm">
            3
          </Button>
          <Button variant="outline" size="sm">
            <ChevronRight className="h-4 w-4" />
          </Button>
        </div>
      </div>
    </div>
  )
}

function StatCard({
  title,
  value,
  icon,
  color = 'gray',
}: {
  title: string
  value: string
  icon: React.ReactNode
  color?: 'gray' | 'green' | 'yellow' | 'red'
}) {
  const colorClasses = {
    gray: 'bg-primary-100 text-primary-600',
    green: 'bg-green-100 text-green-600',
    yellow: 'bg-yellow-100 text-yellow-600',
    red: 'bg-red-100 text-red-600',
  }

  return (
    <Card>
      <CardContent className="p-4">
        <div className="flex items-center gap-3">
          <div className={cn('rounded-lg p-2', colorClasses[color])}>{icon}</div>
          <div>
            <p className="text-sm text-primary-500">{title}</p>
            <p className="text-2xl font-bold text-primary-900">{value}</p>
          </div>
        </div>
      </CardContent>
    </Card>
  )
}

function ExecutionRow({ execution }: { execution: RuleExecution }) {
  const statusConfig: Record<ExecutionStatus, { icon: React.ReactNode; className: string; label: string }> = {
    success: {
      icon: <CheckCircle className="h-4 w-4" />,
      className: 'bg-green-100 text-green-700',
      label: 'Passed',
    },
    partial: {
      icon: <AlertTriangle className="h-4 w-4" />,
      className: 'bg-yellow-100 text-yellow-700',
      label: 'Anomalies',
    },
    failed: {
      icon: <XCircle className="h-4 w-4" />,
      className: 'bg-red-100 text-red-700',
      label: 'Failed',
    },
  }

  const engineLabels: Record<ValidationEngine, string> = {
    structural: 'S',
    zen: 'Z',
    statistical: 'T',
    all: 'All',
  }

  const status = statusConfig[execution.status]
  const executedAt = new Date(execution.executed_at)
  const timeAgo = getTimeAgo(executedAt)

  return (
    <tr className="border-b border-primary-100 hover:bg-primary-50 transition-colors">
      <td className="px-4 py-3">
        <span
          className={cn(
            'inline-flex items-center gap-1 rounded-full px-2.5 py-0.5 text-xs font-medium',
            status.className
          )}
        >
          {status.icon}
          {status.label}
        </span>
      </td>
      <td className="px-4 py-3">
        <div className="flex items-center gap-2">
          <FileText className="h-4 w-4 text-primary-400" />
          <div>
            <Link
              to={`/documents/invoices/${execution.document_id}`}
              className="font-medium text-primary-900 hover:text-primary-700 hover:underline"
            >
              {execution.invoice_number || execution.document_id}
            </Link>
            <p className="text-xs text-primary-400 capitalize">{execution.document_type}</p>
          </div>
        </div>
      </td>
      <td className="px-4 py-3 text-sm text-primary-700">
        {execution.vendor_name || '—'}
      </td>
      <td className="px-4 py-3">
        <div className="flex gap-1">
          {execution.engines_run.map((engine) => (
            <span
              key={engine}
              className={cn(
                'inline-flex items-center justify-center w-6 h-6 rounded text-xs font-medium',
                engine === 'structural' && 'bg-blue-100 text-blue-700',
                engine === 'zen' && 'bg-purple-100 text-purple-700',
                engine === 'statistical' && 'bg-green-100 text-green-700'
              )}
              title={`${engine.charAt(0).toUpperCase() + engine.slice(1)} Validator`}
            >
              {engineLabels[engine]}
            </span>
          ))}
        </div>
      </td>
      <td className="px-4 py-3 text-center text-sm text-primary-700">
        {execution.rules_evaluated}
      </td>
      <td className="px-4 py-3 text-center">
        {execution.anomalies_detected > 0 ? (
          <span className="inline-flex items-center gap-1 text-sm font-medium text-error">
            <AlertTriangle className="h-4 w-4" />
            {execution.anomalies_detected}
          </span>
        ) : (
          <span className="text-sm text-primary-400">0</span>
        )}
      </td>
      <td className="px-4 py-3 text-right">
        <span
          className={cn(
            'text-sm font-mono',
            execution.duration_ms > 100 ? 'text-yellow-600' : 'text-primary-600'
          )}
        >
          {execution.duration_ms}ms
        </span>
      </td>
      <td className="px-4 py-3 text-right text-sm text-primary-500">
        <span title={executedAt.toLocaleString()}>{timeAgo}</span>
      </td>
      <td className="px-4 py-3">
        <Button variant="ghost" size="icon" className="h-8 w-8">
          <Eye className="h-4 w-4" />
        </Button>
      </td>
    </tr>
  )
}

function getTimeAgo(date: Date): string {
  const now = new Date()
  const diffMs = now.getTime() - date.getTime()
  const diffMins = Math.floor(diffMs / 60000)

  if (diffMins < 1) return 'Just now'
  if (diffMins < 60) return `${diffMins}m ago`

  const diffHours = Math.floor(diffMins / 60)
  if (diffHours < 24) return `${diffHours}h ago`

  const diffDays = Math.floor(diffHours / 24)
  return `${diffDays}d ago`
}

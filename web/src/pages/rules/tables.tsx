import { useState } from 'react'
import {
  Table,
  Plus,
  Search,
  MoreHorizontal,
  Play,
  Pause,
  Copy,
  Trash2,
  Edit,
  CheckCircle,
  AlertTriangle,
  Clock,
  ChevronDown,
  ChevronRight,
} from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { cn } from '@/lib/utils'

type RuleStatus = 'active' | 'inactive' | 'draft' | 'error'
type RuleSeverity = 'low' | 'medium' | 'high' | 'critical'

interface DecisionRule {
  id: string
  name: string
  description: string
  category: string
  status: RuleStatus
  severity: RuleSeverity
  conditions: RuleCondition[]
  actions: RuleAction[]
  lastExecuted?: string
  executionCount: number
  matchCount: number
  created_at: string
  updated_at: string
}

interface RuleCondition {
  field: string
  operator: string
  value: string
}

interface RuleAction {
  type: string
  params: Record<string, string>
}

const mockRules: DecisionRule[] = [
  {
    id: 'rule-001',
    name: 'Price Deviation Check',
    description: 'Flag invoices with unit prices exceeding 25% of baseline',
    category: 'Price Validation',
    status: 'active',
    severity: 'high',
    conditions: [
      { field: 'unit_price_deviation', operator: '>', value: '25%' },
    ],
    actions: [
      { type: 'flag_anomaly', params: { type: 'price_deviation', severity: 'high' } },
    ],
    lastExecuted: '2024-02-04T10:30:00Z',
    executionCount: 12847,
    matchCount: 234,
    created_at: '2023-06-15T10:00:00Z',
    updated_at: '2024-01-15T14:30:00Z',
  },
  {
    id: 'rule-002',
    name: 'Duplicate Invoice Detection',
    description: 'Detect potential duplicate invoices within 30-day window',
    category: 'Duplicate Detection',
    status: 'active',
    severity: 'critical',
    conditions: [
      { field: 'invoice_number', operator: 'similar_to', value: 'existing' },
      { field: 'vendor', operator: '=', value: 'same' },
      { field: 'date_diff', operator: '<', value: '30 days' },
    ],
    actions: [
      { type: 'flag_anomaly', params: { type: 'duplicate_invoice', severity: 'critical' } },
      { type: 'send_alert', params: { channel: 'email' } },
    ],
    lastExecuted: '2024-02-04T10:30:00Z',
    executionCount: 12847,
    matchCount: 45,
    created_at: '2023-07-20T10:00:00Z',
    updated_at: '2024-02-01T09:00:00Z',
  },
  {
    id: 'rule-003',
    name: 'Contract Pricing Validation',
    description: 'Validate line item prices against contract terms',
    category: 'Contract Compliance',
    status: 'active',
    severity: 'high',
    conditions: [
      { field: 'has_contract', operator: '=', value: 'true' },
      { field: 'price_vs_contract', operator: '>', value: '0%' },
    ],
    actions: [
      { type: 'flag_anomaly', params: { type: 'contract_violation', severity: 'high' } },
    ],
    lastExecuted: '2024-02-04T10:30:00Z',
    executionCount: 8543,
    matchCount: 89,
    created_at: '2023-08-10T10:00:00Z',
    updated_at: '2024-01-20T11:00:00Z',
  },
  {
    id: 'rule-004',
    name: 'Missing PO Number',
    description: 'Flag invoices from vendors requiring PO without one',
    category: 'Completeness',
    status: 'inactive',
    severity: 'medium',
    conditions: [
      { field: 'vendor.requires_po', operator: '=', value: 'true' },
      { field: 'po_number', operator: 'is_empty', value: '' },
    ],
    actions: [
      { type: 'flag_anomaly', params: { type: 'missing_field', severity: 'medium' } },
    ],
    lastExecuted: '2024-01-15T08:00:00Z',
    executionCount: 5000,
    matchCount: 156,
    created_at: '2023-09-05T10:00:00Z',
    updated_at: '2024-01-15T08:00:00Z',
  },
  {
    id: 'rule-005',
    name: 'Large Invoice Alert',
    description: 'Alert on invoices exceeding $50,000',
    category: 'Threshold',
    status: 'draft',
    severity: 'medium',
    conditions: [
      { field: 'total_amount', operator: '>', value: '50000' },
    ],
    actions: [
      { type: 'send_alert', params: { channel: 'slack' } },
    ],
    executionCount: 0,
    matchCount: 0,
    created_at: '2024-02-01T10:00:00Z',
    updated_at: '2024-02-01T10:00:00Z',
  },
  {
    id: 'rule-006',
    name: 'Math Validation Error',
    description: 'Rule with configuration error',
    category: 'Math Validation',
    status: 'error',
    severity: 'high',
    conditions: [
      { field: 'line_total_sum', operator: '!=', value: 'subtotal' },
    ],
    actions: [
      { type: 'flag_anomaly', params: { type: 'math_mismatch', severity: 'high' } },
    ],
    lastExecuted: '2024-02-03T15:00:00Z',
    executionCount: 100,
    matchCount: 0,
    created_at: '2024-01-25T10:00:00Z',
    updated_at: '2024-02-03T15:00:00Z',
  },
]

const categories = ['All', 'Price Validation', 'Duplicate Detection', 'Contract Compliance', 'Completeness', 'Threshold', 'Math Validation']

export function DecisionTablesPage() {
  const [searchQuery, setSearchQuery] = useState('')
  const [selectedCategory, setSelectedCategory] = useState('All')
  const [expandedRules, setExpandedRules] = useState<Set<string>>(new Set(['rule-001']))

  const filteredRules = mockRules.filter((r) => {
    if (selectedCategory !== 'All' && r.category !== selectedCategory) return false
    if (searchQuery) {
      const query = searchQuery.toLowerCase()
      return (
        r.name.toLowerCase().includes(query) ||
        r.description.toLowerCase().includes(query)
      )
    }
    return true
  })

  const toggleRule = (id: string) => {
    const newExpanded = new Set(expandedRules)
    if (newExpanded.has(id)) {
      newExpanded.delete(id)
    } else {
      newExpanded.add(id)
    }
    setExpandedRules(newExpanded)
  }

  // Stats
  const stats = {
    total: mockRules.length,
    active: mockRules.filter((r) => r.status === 'active').length,
    inactive: mockRules.filter((r) => r.status === 'inactive' || r.status === 'draft').length,
    errors: mockRules.filter((r) => r.status === 'error').length,
  }

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-primary-900">Decision Tables</h1>
          <p className="text-sm text-primary-500">
            Configure validation rules and anomaly detection logic
          </p>
        </div>
        <Button>
          <Plus className="mr-2 h-4 w-4" />
          Create Rule
        </Button>
      </div>

      {/* Stats */}
      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
        <StatCard
          title="Total Rules"
          value={stats.total}
          icon={<Table className="h-5 w-5" />}
        />
        <StatCard
          title="Active"
          value={stats.active}
          icon={<CheckCircle className="h-5 w-5" />}
          color="green"
        />
        <StatCard
          title="Inactive/Draft"
          value={stats.inactive}
          icon={<Pause className="h-5 w-5" />}
          color="yellow"
        />
        <StatCard
          title="Errors"
          value={stats.errors}
          icon={<AlertTriangle className="h-5 w-5" />}
          color="red"
        />
      </div>

      {/* Filters */}
      <Card className="p-4">
        <div className="flex flex-col gap-4 sm:flex-row sm:items-center">
          <div className="relative flex-1 max-w-md">
            <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-primary-400" />
            <Input
              type="search"
              placeholder="Search rules..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="pl-10"
            />
          </div>
          <select
            value={selectedCategory}
            onChange={(e) => setSelectedCategory(e.target.value)}
            className="h-10 rounded-lg border border-primary-200 bg-white px-3 text-sm"
          >
            {categories.map((cat) => (
              <option key={cat} value={cat}>{cat}</option>
            ))}
          </select>
        </div>
      </Card>

      {/* Rules List */}
      <div className="space-y-4">
        {filteredRules.map((rule) => (
          <RuleCard
            key={rule.id}
            rule={rule}
            expanded={expandedRules.has(rule.id)}
            onToggle={() => toggleRule(rule.id)}
          />
        ))}

        {filteredRules.length === 0 && (
          <Card className="p-8 text-center">
            <Table className="mx-auto h-8 w-8 text-primary-300" />
            <p className="mt-2 text-primary-500">No rules found matching your filters.</p>
          </Card>
        )}
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
  value: number
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

interface RuleCardProps {
  rule: DecisionRule
  expanded: boolean
  onToggle: () => void
}

function RuleCard({ rule, expanded, onToggle }: RuleCardProps) {
  const statusConfig: Record<RuleStatus, { icon: React.ReactNode; className: string; label: string }> = {
    active: {
      icon: <CheckCircle className="h-4 w-4" />,
      className: 'bg-green-100 text-green-700',
      label: 'Active',
    },
    inactive: {
      icon: <Pause className="h-4 w-4" />,
      className: 'bg-gray-100 text-gray-700',
      label: 'Inactive',
    },
    draft: {
      icon: <Clock className="h-4 w-4" />,
      className: 'bg-yellow-100 text-yellow-700',
      label: 'Draft',
    },
    error: {
      icon: <AlertTriangle className="h-4 w-4" />,
      className: 'bg-red-100 text-red-700',
      label: 'Error',
    },
  }

  const severityColors: Record<RuleSeverity, string> = {
    low: 'bg-green-100 text-green-700',
    medium: 'bg-yellow-100 text-yellow-700',
    high: 'bg-orange-100 text-orange-700',
    critical: 'bg-red-100 text-red-700',
  }

  const status = statusConfig[rule.status]

  return (
    <Card className={cn(rule.status === 'error' && 'border-red-200')}>
      <CardHeader
        className="cursor-pointer hover:bg-primary-50 transition-colors"
        onClick={onToggle}
      >
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-3">
            {expanded ? (
              <ChevronDown className="h-5 w-5 text-primary-400" />
            ) : (
              <ChevronRight className="h-5 w-5 text-primary-400" />
            )}
            <div>
              <div className="flex items-center gap-2">
                <CardTitle className="text-base">{rule.name}</CardTitle>
                <span
                  className={cn(
                    'inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-xs font-medium',
                    status.className
                  )}
                >
                  {status.icon}
                  {status.label}
                </span>
                <span
                  className={cn(
                    'rounded-full px-2 py-0.5 text-xs font-medium',
                    severityColors[rule.severity]
                  )}
                >
                  {rule.severity.toUpperCase()}
                </span>
              </div>
              <p className="text-sm text-primary-500 mt-1">{rule.description}</p>
            </div>
          </div>
          <div className="flex items-center gap-4">
            <div className="text-right text-sm">
              <p className="text-primary-900 font-medium">{rule.matchCount.toLocaleString()}</p>
              <p className="text-primary-400">matches</p>
            </div>
            <Button
              variant="ghost"
              size="icon"
              className="h-8 w-8"
              onClick={(e) => e.stopPropagation()}
            >
              <MoreHorizontal className="h-4 w-4" />
            </Button>
          </div>
        </div>
      </CardHeader>

      {expanded && (
        <CardContent className="pt-0 border-t border-primary-100">
          <div className="grid gap-6 lg:grid-cols-2 mt-4">
            {/* Conditions */}
            <div>
              <h4 className="text-sm font-medium text-primary-700 mb-3">Conditions (IF)</h4>
              <div className="space-y-2">
                {rule.conditions.map((condition, i) => (
                  <div
                    key={i}
                    className="flex items-center gap-2 rounded-lg bg-primary-50 px-3 py-2 text-sm"
                  >
                    <span className="font-mono text-primary-600">{condition.field}</span>
                    <span className="text-primary-400">{condition.operator}</span>
                    <span className="font-medium text-primary-900">{condition.value}</span>
                  </div>
                ))}
              </div>
            </div>

            {/* Actions */}
            <div>
              <h4 className="text-sm font-medium text-primary-700 mb-3">Actions (THEN)</h4>
              <div className="space-y-2">
                {rule.actions.map((action, i) => (
                  <div
                    key={i}
                    className="rounded-lg bg-blue-50 px-3 py-2 text-sm"
                  >
                    <span className="font-medium text-blue-700">{action.type}</span>
                    {Object.keys(action.params).length > 0 && (
                      <span className="text-blue-500 ml-2">
                        ({Object.entries(action.params).map(([k, v]) => `${k}: ${v}`).join(', ')})
                      </span>
                    )}
                  </div>
                ))}
              </div>
            </div>
          </div>

          {/* Stats & Actions */}
          <div className="flex items-center justify-between mt-6 pt-4 border-t border-primary-100">
            <div className="flex items-center gap-6 text-sm text-primary-500">
              <span>Category: <span className="text-primary-700">{rule.category}</span></span>
              <span>Executions: <span className="text-primary-700">{rule.executionCount.toLocaleString()}</span></span>
              {rule.lastExecuted && (
                <span>Last run: <span className="text-primary-700">{new Date(rule.lastExecuted).toLocaleString()}</span></span>
              )}
            </div>
            <div className="flex items-center gap-2">
              <Button variant="outline" size="sm">
                <Play className="mr-2 h-4 w-4" />
                Test Rule
              </Button>
              <Button variant="outline" size="sm">
                <Edit className="mr-2 h-4 w-4" />
                Edit
              </Button>
              <Button variant="outline" size="sm">
                <Copy className="mr-2 h-4 w-4" />
                Clone
              </Button>
              <Button variant="outline" size="sm" className="text-red-600 hover:text-red-700">
                <Trash2 className="h-4 w-4" />
              </Button>
            </div>
          </div>
        </CardContent>
      )}
    </Card>
  )
}

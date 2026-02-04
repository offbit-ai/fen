import { useState } from 'react'
import { Link } from 'react-router-dom'
import {
  AlertTriangle,
  Search,
  CheckCircle,
  XCircle,
  Eye,
  ChevronLeft,
  ChevronRight,
} from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Card, CardContent } from '@/components/ui/card'
import { cn } from '@/lib/utils'
import type { Anomaly, AnomalySeverity, AnomalyStatus, AnomalyType } from '@/types/api'

// Mock data
const mockAnomalies: Anomaly[] = [
  {
    id: 'anom-1',
    document_id: 'inv-001',
    document_type: 'invoice',
    tenant_id: 'tenant-1',
    anomaly_type: 'price_deviation',
    severity: 'high',
    confidence: 0.92,
    description: 'Unit price 35% higher than historical average',
    details: { vendor: 'TechParts Inc', invoice: 'INV-2024-889' },
    status: 'open',
    created_at: '2024-02-01T10:05:00Z',
  },
  {
    id: 'anom-2',
    document_id: 'inv-002',
    document_type: 'invoice',
    tenant_id: 'tenant-1',
    anomaly_type: 'duplicate_invoice',
    severity: 'critical',
    confidence: 0.98,
    description: 'Potential duplicate of INV-2024-850',
    details: { vendor: 'GlobalSupply Co', invoice: 'INV-2024-887' },
    status: 'investigating',
    created_at: '2024-02-01T09:30:00Z',
  },
  {
    id: 'anom-3',
    document_id: 'inv-003',
    document_type: 'invoice',
    tenant_id: 'tenant-1',
    anomaly_type: 'statistical_outlier',
    severity: 'medium',
    confidence: 0.78,
    description: 'Invoice amount exceeds 2 standard deviations',
    details: { vendor: 'Acme Corp', invoice: 'INV-2024-885' },
    status: 'open',
    created_at: '2024-01-31T14:20:00Z',
  },
  {
    id: 'anom-4',
    document_id: 'inv-004',
    document_type: 'invoice',
    tenant_id: 'tenant-1',
    anomaly_type: 'missing_contract',
    severity: 'medium',
    confidence: 1.0,
    description: 'No matching contract found for vendor',
    details: { vendor: 'NewVendor LLC', invoice: 'INV-2024-890' },
    status: 'resolved',
    created_at: '2024-01-30T11:00:00Z',
  },
  {
    id: 'anom-5',
    document_id: 'inv-005',
    document_type: 'invoice',
    tenant_id: 'tenant-1',
    anomaly_type: 'quantity_mismatch',
    severity: 'low',
    confidence: 0.85,
    description: 'Quantity differs from PO by 5%',
    details: { vendor: 'Office Supplies Ltd', invoice: 'INV-2024-882' },
    status: 'dismissed',
    created_at: '2024-01-29T16:45:00Z',
  },
]

export function AnomaliesPage() {
  const [searchQuery, setSearchQuery] = useState('')
  const [selectedSeverity, setSelectedSeverity] = useState<AnomalySeverity | 'all'>('all')
  const [selectedStatus, setSelectedStatus] = useState<AnomalyStatus | 'all'>('all')

  const filteredAnomalies = mockAnomalies.filter((a) => {
    if (selectedSeverity !== 'all' && a.severity !== selectedSeverity) return false
    if (selectedStatus !== 'all' && a.status !== selectedStatus) return false
    if (searchQuery && !a.description.toLowerCase().includes(searchQuery.toLowerCase())) return false
    return true
  })

  // Stats
  const stats = {
    total: mockAnomalies.length,
    open: mockAnomalies.filter((a) => a.status === 'open').length,
    critical: mockAnomalies.filter((a) => a.severity === 'critical').length,
    high: mockAnomalies.filter((a) => a.severity === 'high').length,
  }

  return (
    <div className="space-y-6">
      {/* Header */}
      <div>
        <h1 className="text-2xl font-bold text-primary-900">Anomalies</h1>
        <p className="text-sm text-primary-500">
          Review and manage detected anomalies in your documents
        </p>
      </div>

      {/* Stats Cards */}
      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
        <StatCard
          title="Total Anomalies"
          value={stats.total}
          icon={<AlertTriangle className="h-5 w-5" />}
        />
        <StatCard
          title="Open"
          value={stats.open}
          icon={<Eye className="h-5 w-5" />}
          color="yellow"
        />
        <StatCard
          title="Critical"
          value={stats.critical}
          icon={<XCircle className="h-5 w-5" />}
          color="red"
        />
        <StatCard
          title="High Severity"
          value={stats.high}
          icon={<AlertTriangle className="h-5 w-5" />}
          color="orange"
        />
      </div>

      {/* Filters */}
      <Card className="p-4">
        <div className="flex flex-col gap-4 sm:flex-row sm:items-center">
          <div className="relative flex-1 max-w-md">
            <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-primary-400" />
            <Input
              type="search"
              placeholder="Search anomalies..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="pl-10"
            />
          </div>
          <div className="flex items-center gap-2">
            <select
              value={selectedSeverity}
              onChange={(e) => setSelectedSeverity(e.target.value as AnomalySeverity | 'all')}
              className="h-10 rounded-lg border border-primary-200 bg-white px-3 text-sm"
            >
              <option value="all">All Severities</option>
              <option value="critical">Critical</option>
              <option value="high">High</option>
              <option value="medium">Medium</option>
              <option value="low">Low</option>
            </select>
            <select
              value={selectedStatus}
              onChange={(e) => setSelectedStatus(e.target.value as AnomalyStatus | 'all')}
              className="h-10 rounded-lg border border-primary-200 bg-white px-3 text-sm"
            >
              <option value="all">All Statuses</option>
              <option value="open">Open</option>
              <option value="investigating">Investigating</option>
              <option value="resolved">Resolved</option>
              <option value="dismissed">Dismissed</option>
            </select>
          </div>
        </div>
      </Card>

      {/* Anomaly List */}
      <div className="space-y-4">
        {filteredAnomalies.map((anomaly) => (
          <AnomalyCard key={anomaly.id} anomaly={anomaly} />
        ))}

        {filteredAnomalies.length === 0 && (
          <Card className="p-8 text-center">
            <AlertTriangle className="mx-auto h-8 w-8 text-primary-300" />
            <p className="mt-2 text-primary-500">No anomalies found matching your filters.</p>
          </Card>
        )}
      </div>

      {/* Pagination */}
      {filteredAnomalies.length > 0 && (
        <div className="flex items-center justify-between">
          <div className="text-sm text-primary-500">
            Showing {filteredAnomalies.length} of {mockAnomalies.length} anomalies
          </div>
          <div className="flex items-center gap-2">
            <Button variant="outline" size="sm" disabled>
              <ChevronLeft className="h-4 w-4" />
            </Button>
            <span className="text-sm text-primary-700">Page 1 of 1</span>
            <Button variant="outline" size="sm" disabled>
              <ChevronRight className="h-4 w-4" />
            </Button>
          </div>
        </div>
      )}
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
  color?: 'gray' | 'yellow' | 'red' | 'orange' | 'green'
}) {
  const colorClasses = {
    gray: 'bg-primary-100 text-primary-600',
    yellow: 'bg-yellow-100 text-yellow-600',
    red: 'bg-red-100 text-red-600',
    orange: 'bg-orange-100 text-orange-600',
    green: 'bg-green-100 text-green-600',
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

function AnomalyCard({ anomaly }: { anomaly: Anomaly }) {
  const severityColors: Record<AnomalySeverity, string> = {
    critical: 'border-l-red-500',
    high: 'border-l-orange-500',
    medium: 'border-l-yellow-500',
    low: 'border-l-green-500',
  }

  const severityBadge: Record<AnomalySeverity, string> = {
    critical: 'bg-red-100 text-red-700',
    high: 'bg-orange-100 text-orange-700',
    medium: 'bg-yellow-100 text-yellow-700',
    low: 'bg-green-100 text-green-700',
  }

  const statusBadge: Record<AnomalyStatus, { className: string; icon: React.ReactNode }> = {
    open: {
      className: 'bg-blue-100 text-blue-700',
      icon: <Eye className="h-3 w-3" />,
    },
    investigating: {
      className: 'bg-purple-100 text-purple-700',
      icon: <Search className="h-3 w-3" />,
    },
    resolved: {
      className: 'bg-green-100 text-green-700',
      icon: <CheckCircle className="h-3 w-3" />,
    },
    dismissed: {
      className: 'bg-gray-100 text-gray-700',
      icon: <XCircle className="h-3 w-3" />,
    },
  }

  const typeLabels: Record<AnomalyType, string> = {
    // Backend types
    math_mismatch: 'Math Mismatch',
    missing_field: 'Missing Field',
    invalid_format: 'Invalid Format',
    out_of_range: 'Out of Range',
    potential_duplicate: 'Potential Duplicate',
    date_inconsistency: 'Date Inconsistency',
    contract_violation: 'Contract Violation',
    validation_failure: 'Validation Failure',
    statistical_outlier: 'Statistical Outlier',
    // Frontend display aliases
    price_deviation: 'Price Deviation',
    duplicate_invoice: 'Duplicate Invoice',
    missing_contract: 'Missing Contract',
    quantity_mismatch: 'Quantity Mismatch',
    date_anomaly: 'Date Anomaly',
    vendor_mismatch: 'Vendor Mismatch',
    rule_violation: 'Rule Violation',
  }

  return (
    <Card className={cn('border-l-4', severityColors[anomaly.severity])}>
      <CardContent className="p-4">
        <div className="flex items-start justify-between">
          <div className="flex-1">
            <div className="flex items-center gap-2 flex-wrap">
              <span
                className={cn(
                  'rounded-full px-2 py-0.5 text-xs font-medium',
                  severityBadge[anomaly.severity]
                )}
              >
                {anomaly.severity.toUpperCase()}
              </span>
              {anomaly.status && (
                <span
                  className={cn(
                    'inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-xs font-medium',
                    statusBadge[anomaly.status].className
                  )}
                >
                  {statusBadge[anomaly.status].icon}
                  {anomaly.status.charAt(0).toUpperCase() + anomaly.status.slice(1)}
                </span>
              )}
              <span className="text-xs text-primary-500">
                {typeLabels[anomaly.anomaly_type]}
              </span>
              <span className="text-xs text-primary-400">
                {Math.round(anomaly.confidence * 100)}% confidence
              </span>
            </div>
            <p className="mt-2 text-sm text-primary-900">{anomaly.description}</p>
            <div className="mt-2 flex items-center gap-4 text-xs text-primary-500">
              <span>
                Invoice: {String(anomaly.details?.invoice || anomaly.document_id)}
              </span>
              <span>
                Vendor: {String(anomaly.details?.vendor || 'Unknown')}
              </span>
              {anomaly.created_at && (
                <span>
                  {new Date(anomaly.created_at).toLocaleDateString()}
                </span>
              )}
            </div>
          </div>
          <div className="flex items-center gap-2 ml-4">
            {(anomaly.status === 'open' || anomaly.status === 'investigating') ? (
              <>
                <Button size="sm" variant="outline">
                  Resolve
                </Button>
                <Button size="sm" variant="ghost">
                  Dismiss
                </Button>
              </>
            ) : (
              <Button size="sm" variant="ghost" asChild>
                <Link to={`/analysis/anomalies/${anomaly.id}`}>
                  View Details
                </Link>
              </Button>
            )}
          </div>
        </div>
      </CardContent>
    </Card>
  )
}

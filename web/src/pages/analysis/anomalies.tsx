import { useState, useMemo } from 'react'
import { Link } from 'react-router-dom'
import {
  AlertTriangle,
  Search,
  CheckCircle,
  XCircle,
  Eye,
  ChevronLeft,
  ChevronRight,
  Loader2,
  WifiOff,
} from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Card, CardContent } from '@/components/ui/card'
import { cn } from '@/lib/utils'
import { useAnomalies, useAnomalyStats } from '@/hooks/use-anomalies'
import { useMutation } from '@/hooks/use-api'
import { anomaliesApi } from '@/api/endpoints/anomalies'
import type { Anomaly, AnomalySeverity, AnomalyStatus, AnomalyType } from '@/types/api'

// Mock data fallback
const MOCK_ANOMALIES: Anomaly[] = [
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
  const [currentPage, setCurrentPage] = useState(1)
  const pageSize = 20

  const { data: apiData, isLoading, error, refetch } = useAnomalies({
    page: currentPage,
    page_size: pageSize,
    severity: selectedSeverity !== 'all' ? selectedSeverity : undefined,
    status: selectedStatus !== 'all' ? selectedStatus : undefined,
  })
  const { data: apiStats } = useAnomalyStats()

  const { mutate: resolveAnomaly } = useMutation((id: string) => anomaliesApi.resolve(id))
  const { mutate: dismissAnomaly } = useMutation((id: string) => anomaliesApi.dismiss(id, 'Dismissed by user'))

  const isApiConnected = !error
  const anomalies = useMemo(() => {
    if (apiData?.items?.length) return apiData.items
    if (isLoading) return []
    return MOCK_ANOMALIES
  }, [apiData, isLoading])

  const totalItems = apiData?.total ?? MOCK_ANOMALIES.length
  const totalPages = Math.ceil(totalItems / pageSize)

  // Client-side filtering only when using mock data
  const filteredAnomalies = useMemo(() => {
    if (isApiConnected && apiData?.items) return anomalies
    return anomalies.filter((a) => {
      if (selectedSeverity !== 'all' && a.severity !== selectedSeverity) return false
      if (selectedStatus !== 'all' && a.status !== selectedStatus) return false
      if (searchQuery && !a.description.toLowerCase().includes(searchQuery.toLowerCase())) return false
      return true
    })
  }, [anomalies, selectedSeverity, selectedStatus, searchQuery, isApiConnected, apiData])

  // Stats from API or computed from mock data
  const stats = useMemo(() => {
    if (apiStats) {
      return {
        total: apiStats.total,
        open: apiStats.by_status?.open ?? 0,
        critical: apiStats.by_severity?.critical ?? 0,
        high: apiStats.by_severity?.high ?? 0,
      }
    }
    const source = apiData?.items?.length ? apiData.items : MOCK_ANOMALIES
    return {
      total: apiData?.total ?? source.length,
      open: source.filter((a) => a.status === 'open').length,
      critical: source.filter((a) => a.severity === 'critical').length,
      high: source.filter((a) => a.severity === 'high').length,
    }
  }, [apiStats, apiData])

  const handleResolve = async (id: string) => {
    try {
      await resolveAnomaly(id)
      refetch()
    } catch {
      // Error handled by useMutation
    }
  }

  const handleDismiss = async (id: string) => {
    try {
      await dismissAnomaly(id)
      refetch()
    } catch {
      // Error handled by useMutation
    }
  }

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-primary-900">Anomalies</h1>
          <p className="text-sm text-primary-500">
            Review and manage detected anomalies in your documents
          </p>
        </div>
        {!isApiConnected && (
          <div className="flex items-center gap-2 rounded-lg border border-amber-200 bg-amber-50 px-3 py-1.5 text-xs text-amber-700">
            <WifiOff className="h-3.5 w-3.5" />
            API offline &middot; showing sample data
          </div>
        )}
      </div>

      {/* Stats Cards */}
      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
        <StatCard
          title="Total Anomalies"
          value={stats.total}
          icon={<AlertTriangle className="h-5 w-5" />}
          loading={isLoading}
        />
        <StatCard
          title="Open"
          value={stats.open}
          icon={<Eye className="h-5 w-5" />}
          color="yellow"
          loading={isLoading}
        />
        <StatCard
          title="Critical"
          value={stats.critical}
          icon={<XCircle className="h-5 w-5" />}
          color="red"
          loading={isLoading}
        />
        <StatCard
          title="High Severity"
          value={stats.high}
          icon={<AlertTriangle className="h-5 w-5" />}
          color="orange"
          loading={isLoading}
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
        {isLoading ? (
          <Card className="p-8 text-center">
            <Loader2 className="mx-auto h-6 w-6 animate-spin text-primary-400" />
            <p className="mt-2 text-sm text-primary-500">Loading anomalies...</p>
          </Card>
        ) : filteredAnomalies.length === 0 ? (
          <Card className="p-8 text-center">
            <AlertTriangle className="mx-auto h-8 w-8 text-primary-300" />
            <p className="mt-2 text-primary-500">No anomalies found matching your filters.</p>
          </Card>
        ) : (
          filteredAnomalies.map((anomaly) => (
            <AnomalyCard
              key={anomaly.id ?? anomaly.document_id}
              anomaly={anomaly}
              onResolve={handleResolve}
              onDismiss={handleDismiss}
            />
          ))
        )}
      </div>

      {/* Pagination */}
      {filteredAnomalies.length > 0 && (
        <div className="flex items-center justify-between">
          <div className="text-sm text-primary-500">
            Showing {(currentPage - 1) * pageSize + 1}-{Math.min(currentPage * pageSize, totalItems)} of {totalItems} anomalies
          </div>
          <div className="flex items-center gap-2">
            <Button
              variant="outline"
              size="sm"
              disabled={currentPage === 1}
              onClick={() => setCurrentPage((p) => p - 1)}
            >
              <ChevronLeft className="h-4 w-4" />
            </Button>
            <span className="text-sm text-primary-700">
              Page {currentPage} of {totalPages || 1}
            </span>
            <Button
              variant="outline"
              size="sm"
              disabled={currentPage === totalPages || totalPages === 0}
              onClick={() => setCurrentPage((p) => p + 1)}
            >
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
  loading = false,
}: {
  title: string
  value: number
  icon: React.ReactNode
  color?: 'gray' | 'yellow' | 'red' | 'orange' | 'green'
  loading?: boolean
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
            {loading ? (
              <div className="h-8 w-12 animate-pulse rounded bg-primary-100" />
            ) : (
              <p className="text-2xl font-bold text-primary-900">{value}</p>
            )}
          </div>
        </div>
      </CardContent>
    </Card>
  )
}

function AnomalyCard({
  anomaly,
  onResolve,
  onDismiss,
}: {
  anomaly: Anomaly
  onResolve: (id: string) => void
  onDismiss: (id: string) => void
}) {
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
    math_mismatch: 'Math Mismatch',
    missing_field: 'Missing Field',
    invalid_format: 'Invalid Format',
    out_of_range: 'Out of Range',
    potential_duplicate: 'Potential Duplicate',
    date_inconsistency: 'Date Inconsistency',
    contract_violation: 'Contract Violation',
    validation_failure: 'Validation Failure',
    statistical_outlier: 'Statistical Outlier',
    price_deviation: 'Price Deviation',
    duplicate_invoice: 'Duplicate Invoice',
    missing_contract: 'Missing Contract',
    quantity_mismatch: 'Quantity Mismatch',
    date_anomaly: 'Date Anomaly',
    vendor_mismatch: 'Vendor Mismatch',
    rule_violation: 'Rule Violation',
  }

  const anomalyId = anomaly.id ?? anomaly.document_id

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
                {typeLabels[anomaly.anomaly_type] ?? anomaly.anomaly_type.replace(/_/g, ' ')}
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
              {(anomaly.created_at || anomaly.detected_at) && (
                <span>
                  {new Date(anomaly.created_at ?? anomaly.detected_at!).toLocaleDateString()}
                </span>
              )}
            </div>
          </div>
          <div className="flex items-center gap-2 ml-4">
            {(anomaly.status === 'open' || anomaly.status === 'investigating') ? (
              <>
                <Button size="sm" variant="outline" onClick={() => onResolve(anomalyId)}>
                  Resolve
                </Button>
                <Button size="sm" variant="ghost" onClick={() => onDismiss(anomalyId)}>
                  Dismiss
                </Button>
              </>
            ) : (
              <Button size="sm" variant="ghost" asChild>
                <Link to={`/app/analysis/anomalies/${anomalyId}`}>
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

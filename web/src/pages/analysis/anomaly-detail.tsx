import { useParams, Link } from 'react-router-dom'
import {
  ArrowLeft,
  AlertTriangle,
  FileText,
  Calendar,
  User,
  CheckCircle,
  XCircle,
  Clock,
  TrendingUp,
  TrendingDown,
  Activity,
  BarChart2,
  ExternalLink,
  MessageSquare,
} from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { cn } from '@/lib/utils'
import type { Anomaly, AnomalyType, AnomalySeverity, AnomalyStatus, StatisticalScore } from '@/types/api'

// Mock anomaly data
const mockAnomaly: Anomaly & {
  id: string
  tenant_id: string
  document_type: 'invoice' | 'contract'
  status: AnomalyStatus
  resolved_by?: string
  resolved_at?: string
  created_at: string
  invoice_number?: string
  vendor_name?: string
  total_amount?: number
  notes?: string[]
} = {
  id: 'anom-001',
  document_id: 'inv-001',
  tenant_id: 'tenant-1',
  anomaly_type: 'statistical_outlier',
  severity: 'high',
  description: 'Invoice total amount ($45,890.00) is 3.2 standard deviations above the vendor baseline mean of $12,450.00',
  field_path: 'total_amount',
  expected_value: '$8,500 - $16,400 (within 2σ)',
  actual_value: '$45,890.00',
  confidence: 0.94,
  statistical_score: {
    z_score: 3.2,
    percentile: 99.2,
    trend: 'increasing',
    is_outlier: true,
    baseline_mean: 12450,
    baseline_stddev: 10450,
    sample_count: 156,
  },
  detected_at: '2024-02-04T10:30:00Z',
  document_type: 'invoice',
  status: 'open',
  created_at: '2024-02-04T10:30:00Z',
  invoice_number: 'INV-2024-0892',
  vendor_name: 'Acme Industrial Supplies',
  total_amount: 45890,
  notes: [
    'Flagged for manager review due to high severity',
    'Vendor has history of quarterly bulk orders - may be legitimate',
  ],
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

const severityConfig: Record<AnomalySeverity, { label: string; className: string; bgClass: string }> = {
  critical: { label: 'Critical', className: 'text-red-700', bgClass: 'bg-red-100' },
  high: { label: 'High', className: 'text-orange-700', bgClass: 'bg-orange-100' },
  medium: { label: 'Medium', className: 'text-yellow-700', bgClass: 'bg-yellow-100' },
  low: { label: 'Low', className: 'text-blue-700', bgClass: 'bg-blue-100' },
}

const statusConfig: Record<AnomalyStatus, { label: string; icon: React.ReactNode; className: string }> = {
  open: { label: 'Open', icon: <AlertTriangle className="h-4 w-4" />, className: 'bg-red-100 text-red-700' },
  investigating: { label: 'Investigating', icon: <Clock className="h-4 w-4" />, className: 'bg-yellow-100 text-yellow-700' },
  resolved: { label: 'Resolved', icon: <CheckCircle className="h-4 w-4" />, className: 'bg-green-100 text-green-700' },
  dismissed: { label: 'Dismissed', icon: <XCircle className="h-4 w-4" />, className: 'bg-gray-100 text-gray-700' },
}

export function AnomalyDetailPage() {
  const { id: _id } = useParams<{ id: string }>()
  const anomaly = mockAnomaly // In real app, fetch by _id

  const severity = severityConfig[anomaly.severity]
  const status = statusConfig[anomaly.status]

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-4">
          <Link to="/analysis/anomalies">
            <Button variant="ghost" size="icon">
              <ArrowLeft className="h-5 w-5" />
            </Button>
          </Link>
          <div>
            <div className="flex items-center gap-3">
              <h1 className="text-2xl font-bold text-primary-900">
                {typeLabels[anomaly.anomaly_type]}
              </h1>
              <span className={cn('rounded-full px-3 py-1 text-sm font-medium', severity.bgClass, severity.className)}>
                {severity.label}
              </span>
              <span className={cn('inline-flex items-center gap-1 rounded-full px-3 py-1 text-sm font-medium', status.className)}>
                {status.icon}
                {status.label}
              </span>
            </div>
            <p className="text-sm text-primary-500 mt-1">
              Anomaly ID: {anomaly.id} • Detected {anomaly.detected_at ? new Date(anomaly.detected_at).toLocaleString() : 'Unknown'}
            </p>
          </div>
        </div>
        <div className="flex items-center gap-2">
          <Button variant="outline">
            <MessageSquare className="mr-2 h-4 w-4" />
            Add Note
          </Button>
          <Button variant="outline">
            Mark as Investigating
          </Button>
          <Button>
            Resolve
          </Button>
        </div>
      </div>

      <div className="grid gap-6 lg:grid-cols-3">
        {/* Main Content */}
        <div className="lg:col-span-2 space-y-6">
          {/* Description */}
          <Card>
            <CardHeader>
              <CardTitle>Description</CardTitle>
            </CardHeader>
            <CardContent>
              <p className="text-primary-700 leading-relaxed">{anomaly.description}</p>

              <div className="mt-6 grid gap-4 sm:grid-cols-2">
                <div className="rounded-lg border border-primary-100 p-4">
                  <p className="text-xs font-medium text-primary-500 uppercase">Expected Value</p>
                  <p className="mt-1 text-lg font-semibold text-primary-900">
                    {anomaly.expected_value || '—'}
                  </p>
                </div>
                <div className="rounded-lg border border-red-200 bg-red-50 p-4">
                  <p className="text-xs font-medium text-red-600 uppercase">Actual Value</p>
                  <p className="mt-1 text-lg font-semibold text-red-900">
                    {anomaly.actual_value || '—'}
                  </p>
                </div>
              </div>

              {anomaly.field_path && (
                <div className="mt-4">
                  <p className="text-xs font-medium text-primary-500 uppercase">Affected Field</p>
                  <code className="mt-1 inline-block rounded bg-primary-100 px-2 py-1 text-sm font-mono text-primary-700">
                    {anomaly.field_path}
                  </code>
                </div>
              )}
            </CardContent>
          </Card>

          {/* Statistical Analysis */}
          {anomaly.statistical_score && (
            <Card>
              <CardHeader>
                <CardTitle className="flex items-center gap-2">
                  <BarChart2 className="h-5 w-5" />
                  Statistical Analysis
                </CardTitle>
              </CardHeader>
              <CardContent>
                <StatisticalAnalysis score={anomaly.statistical_score} />
              </CardContent>
            </Card>
          )}

          {/* Notes & Activity */}
          <Card>
            <CardHeader>
              <CardTitle>Notes & Activity</CardTitle>
            </CardHeader>
            <CardContent>
              {anomaly.notes && anomaly.notes.length > 0 ? (
                <div className="space-y-4">
                  {anomaly.notes.map((note, i) => (
                    <div key={i} className="flex gap-3">
                      <div className="flex h-8 w-8 items-center justify-center rounded-full bg-primary-100">
                        <User className="h-4 w-4 text-primary-600" />
                      </div>
                      <div className="flex-1">
                        <p className="text-sm text-primary-700">{note}</p>
                        <p className="text-xs text-primary-400 mt-1">System • Auto-generated</p>
                      </div>
                    </div>
                  ))}
                </div>
              ) : (
                <p className="text-sm text-primary-500">No notes yet.</p>
              )}
            </CardContent>
          </Card>
        </div>

        {/* Sidebar */}
        <div className="space-y-6">
          {/* Related Document */}
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <FileText className="h-5 w-5" />
                Related Document
              </CardTitle>
            </CardHeader>
            <CardContent>
              <div className="space-y-3">
                <div>
                  <p className="text-xs font-medium text-primary-500 uppercase">Document Type</p>
                  <p className="text-sm text-primary-900 capitalize">{anomaly.document_type}</p>
                </div>
                <div>
                  <p className="text-xs font-medium text-primary-500 uppercase">Invoice Number</p>
                  <p className="text-sm text-primary-900">{anomaly.invoice_number}</p>
                </div>
                <div>
                  <p className="text-xs font-medium text-primary-500 uppercase">Vendor</p>
                  <p className="text-sm text-primary-900">{anomaly.vendor_name}</p>
                </div>
                <div>
                  <p className="text-xs font-medium text-primary-500 uppercase">Amount</p>
                  <p className="text-sm font-semibold text-primary-900">
                    ${anomaly.total_amount?.toLocaleString()}
                  </p>
                </div>
                <Link
                  to={`/documents/invoices/${anomaly.document_id}`}
                  className="mt-4 flex items-center gap-2 text-sm font-medium text-primary-600 hover:text-primary-800"
                >
                  <ExternalLink className="h-4 w-4" />
                  View Document
                </Link>
              </div>
            </CardContent>
          </Card>

          {/* Detection Info */}
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <Activity className="h-5 w-5" />
                Detection Info
              </CardTitle>
            </CardHeader>
            <CardContent>
              <div className="space-y-3">
                <div>
                  <p className="text-xs font-medium text-primary-500 uppercase">Confidence Score</p>
                  <div className="flex items-center gap-2 mt-1">
                    <div className="flex-1 h-2 rounded-full bg-primary-100">
                      <div
                        className="h-2 rounded-full bg-primary-600"
                        style={{ width: `${anomaly.confidence * 100}%` }}
                      />
                    </div>
                    <span className="text-sm font-medium text-primary-700">
                      {(anomaly.confidence * 100).toFixed(0)}%
                    </span>
                  </div>
                </div>
                <div>
                  <p className="text-xs font-medium text-primary-500 uppercase">Detection Method</p>
                  <p className="text-sm text-primary-900">
                    {anomaly.statistical_score ? 'Statistical Analysis' : 'Rule-based Validation'}
                  </p>
                </div>
                <div>
                  <p className="text-xs font-medium text-primary-500 uppercase">Detected At</p>
                  <p className="text-sm text-primary-900">
                    {anomaly.detected_at ? new Date(anomaly.detected_at).toLocaleString() : '—'}
                  </p>
                </div>
              </div>
            </CardContent>
          </Card>

          {/* Quick Actions */}
          <Card>
            <CardHeader>
              <CardTitle>Quick Actions</CardTitle>
            </CardHeader>
            <CardContent className="space-y-2">
              <Button variant="outline" className="w-full justify-start">
                <CheckCircle className="mr-2 h-4 w-4 text-green-600" />
                Mark as False Positive
              </Button>
              <Button variant="outline" className="w-full justify-start">
                <AlertTriangle className="mr-2 h-4 w-4 text-yellow-600" />
                Escalate to Manager
              </Button>
              <Button variant="outline" className="w-full justify-start">
                <Calendar className="mr-2 h-4 w-4" />
                Schedule Review
              </Button>
            </CardContent>
          </Card>
        </div>
      </div>
    </div>
  )
}

function StatisticalAnalysis({ score }: { score: StatisticalScore }) {
  const trendIcon = score.trend === 'increasing' ? (
    <TrendingUp className="h-4 w-4 text-red-500" />
  ) : score.trend === 'decreasing' ? (
    <TrendingDown className="h-4 w-4 text-green-500" />
  ) : (
    <Activity className="h-4 w-4 text-primary-500" />
  )

  return (
    <div className="space-y-6">
      {/* Key Metrics */}
      <div className="grid gap-4 sm:grid-cols-4">
        <MetricCard
          label="Z-Score"
          value={score.z_score.toFixed(2)}
          subtext={score.is_outlier ? 'Outlier' : 'Normal'}
          highlight={score.is_outlier}
        />
        <MetricCard
          label="Percentile"
          value={`${score.percentile.toFixed(1)}%`}
          subtext="Distribution position"
        />
        <MetricCard
          label="Baseline Mean"
          value={`$${score.baseline_mean.toLocaleString()}`}
          subtext={`σ = $${score.baseline_stddev.toLocaleString()}`}
        />
        <MetricCard
          label="Sample Size"
          value={score.sample_count.toString()}
          subtext="Historical invoices"
        />
      </div>

      {/* Trend */}
      <div className="flex items-center gap-3 rounded-lg border border-primary-100 p-4">
        {trendIcon}
        <div>
          <p className="text-sm font-medium text-primary-900 capitalize">{score.trend} Trend</p>
          <p className="text-xs text-primary-500">Based on recent invoice patterns for this vendor</p>
        </div>
      </div>

      {/* Visual Distribution */}
      <div>
        <p className="text-sm font-medium text-primary-700 mb-3">Distribution Position</p>
        <div className="relative h-8 rounded-full bg-gradient-to-r from-green-200 via-yellow-200 to-red-200">
          {/* Markers */}
          <div className="absolute inset-0 flex items-center justify-between px-2">
            <span className="text-xs text-green-700">-3σ</span>
            <span className="text-xs text-primary-600">μ</span>
            <span className="text-xs text-red-700">+3σ</span>
          </div>
          {/* Current position marker */}
          <div
            className="absolute top-1/2 -translate-y-1/2 w-4 h-4 rounded-full bg-primary-900 border-2 border-white shadow"
            style={{
              left: `${Math.min(Math.max(((score.z_score + 3) / 6) * 100, 0), 100)}%`,
              transform: 'translate(-50%, -50%)',
            }}
          />
        </div>
        <p className="text-xs text-primary-500 mt-2 text-center">
          Current value is {Math.abs(score.z_score).toFixed(1)} standard deviations {score.z_score > 0 ? 'above' : 'below'} the mean
        </p>
      </div>
    </div>
  )
}

function MetricCard({
  label,
  value,
  subtext,
  highlight,
}: {
  label: string
  value: string
  subtext: string
  highlight?: boolean
}) {
  return (
    <div className={cn(
      'rounded-lg border p-4',
      highlight ? 'border-red-200 bg-red-50' : 'border-primary-100'
    )}>
      <p className="text-xs font-medium text-primary-500 uppercase">{label}</p>
      <p className={cn(
        'text-xl font-bold mt-1',
        highlight ? 'text-red-700' : 'text-primary-900'
      )}>
        {value}
      </p>
      <p className="text-xs text-primary-400 mt-1">{subtext}</p>
    </div>
  )
}

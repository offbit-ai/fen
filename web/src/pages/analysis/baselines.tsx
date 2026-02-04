import { useState } from 'react'
import {
  TrendingDown,
  TrendingUp,
  Search,
  RefreshCw,
  Calendar,
  BarChart3,
  AlertTriangle,
  CheckCircle,
  ChevronDown,
  ChevronRight,
} from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { cn } from '@/lib/utils'
import type { VendorBaseline, BaselinePeriod } from '@/types/api'

// Mock data matching backend VendorBaseline type
const mockBaselines: VendorBaseline[] = [
  {
    id: 'bl-001',
    vendor_name: 'TechParts Inc',
    metric_name: 'invoice_total',
    period: 'rolling_90_days',
    stats: {
      count: 45,
      mean: 28500,
      stddev: 8200,
      min: 5000,
      max: 85000,
      p25: 15000,
      p50: 25000,
      p75: 38000,
      p90: 45000,
      p95: 52000,
      p99: 75000,
    },
    recent_values: [25000, 32000, 18000, 45000, 28000, 35000, 22000, 48000],
    computed_at: '2024-02-04T00:00:00Z',
    expires_at: '2024-02-11T00:00:00Z',
  },
  {
    id: 'bl-002',
    vendor_name: 'TechParts Inc',
    metric_name: 'unit_price_widget_a',
    period: 'rolling_90_days',
    stats: {
      count: 120,
      mean: 333.33,
      stddev: 45.5,
      min: 280,
      max: 450,
      p25: 305,
      p50: 330,
      p75: 360,
      p90: 395,
      p95: 420,
      p99: 445,
    },
    recent_values: [320, 335, 310, 345, 330, 450, 325, 340],
    computed_at: '2024-02-04T00:00:00Z',
    expires_at: '2024-02-11T00:00:00Z',
  },
  {
    id: 'bl-003',
    vendor_name: 'GlobalSupply Co',
    metric_name: 'invoice_total',
    period: 'rolling_90_days',
    stats: {
      count: 78,
      mean: 12300,
      stddev: 3500,
      min: 2500,
      max: 28000,
      p25: 8500,
      p50: 11500,
      p75: 15000,
      p90: 18500,
      p95: 22000,
      p99: 26000,
    },
    recent_values: [11500, 13200, 9800, 15400, 12100, 10500, 14200, 11800],
    computed_at: '2024-02-04T00:00:00Z',
    expires_at: '2024-02-11T00:00:00Z',
  },
  {
    id: 'bl-004',
    vendor_name: 'Acme Corp',
    metric_name: 'invoice_total',
    period: 'rolling_90_days',
    stats: {
      count: 32,
      mean: 8750,
      stddev: 2100,
      min: 3500,
      max: 15000,
      p25: 6500,
      p50: 8200,
      p75: 10500,
      p90: 12500,
      p95: 14000,
      p99: 14800,
    },
    recent_values: [8200, 9500, 7800, 10200, 8500, 7200, 9800, 8900],
    computed_at: '2024-02-04T00:00:00Z',
    expires_at: '2024-02-11T00:00:00Z',
  },
  {
    id: 'bl-005',
    vendor_name: 'Office Supplies Ltd',
    metric_name: 'invoice_total',
    period: 'rolling_30_days',
    stats: {
      count: 15,
      mean: 2100,
      stddev: 650,
      min: 850,
      max: 3500,
      p25: 1500,
      p50: 2000,
      p75: 2600,
      p90: 3100,
      p95: 3300,
      p99: 3450,
    },
    recent_values: [2000, 1850, 2200, 1950, 2400, 2100, 1800, 2300],
    computed_at: '2024-02-04T00:00:00Z',
    expires_at: '2024-02-07T00:00:00Z',
  },
]

// Group baselines by vendor
function groupByVendor(baselines: VendorBaseline[]): Record<string, VendorBaseline[]> {
  return baselines.reduce((acc, baseline) => {
    if (!acc[baseline.vendor_name]) {
      acc[baseline.vendor_name] = []
    }
    acc[baseline.vendor_name].push(baseline)
    return acc
  }, {} as Record<string, VendorBaseline[]>)
}

export function BaselinesPage() {
  const [searchQuery, setSearchQuery] = useState('')
  const [selectedPeriod, setSelectedPeriod] = useState<BaselinePeriod | 'all'>('all')
  const [expandedVendors, setExpandedVendors] = useState<Set<string>>(new Set(['TechParts Inc']))

  const filteredBaselines = mockBaselines.filter((b) => {
    if (selectedPeriod !== 'all' && b.period !== selectedPeriod) return false
    if (searchQuery) {
      const query = searchQuery.toLowerCase()
      return (
        b.vendor_name.toLowerCase().includes(query) ||
        b.metric_name.toLowerCase().includes(query)
      )
    }
    return true
  })

  const groupedBaselines = groupByVendor(filteredBaselines)

  const toggleVendor = (vendor: string) => {
    const newExpanded = new Set(expandedVendors)
    if (newExpanded.has(vendor)) {
      newExpanded.delete(vendor)
    } else {
      newExpanded.add(vendor)
    }
    setExpandedVendors(newExpanded)
  }

  // Stats
  const totalVendors = Object.keys(groupedBaselines).length
  const totalMetrics = filteredBaselines.length
  const recentUpdates = filteredBaselines.filter((b) => {
    const computedAt = new Date(b.computed_at)
    const hourAgo = new Date(Date.now() - 60 * 60 * 1000)
    return computedAt > hourAgo
  }).length

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-primary-900">Statistical Baselines</h1>
          <p className="text-sm text-primary-500">
            Vendor spending patterns and anomaly detection thresholds
          </p>
        </div>
        <div className="flex items-center gap-2">
          <Button variant="outline">
            <Calendar className="mr-2 h-4 w-4" />
            History
          </Button>
          <Button>
            <RefreshCw className="mr-2 h-4 w-4" />
            Recompute All
          </Button>
        </div>
      </div>

      {/* Stats Cards */}
      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
        <StatCard
          title="Tracked Vendors"
          value={totalVendors}
          icon={<BarChart3 className="h-5 w-5" />}
        />
        <StatCard
          title="Active Metrics"
          value={totalMetrics}
          icon={<TrendingUp className="h-5 w-5" />}
        />
        <StatCard
          title="Recent Updates"
          value={recentUpdates}
          icon={<RefreshCw className="h-5 w-5" />}
          color="green"
        />
        <StatCard
          title="Anomalies Today"
          value={3}
          icon={<AlertTriangle className="h-5 w-5" />}
          color="yellow"
        />
      </div>

      {/* Filters */}
      <Card className="p-4">
        <div className="flex flex-col gap-4 sm:flex-row sm:items-center">
          <div className="relative flex-1 max-w-md">
            <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-primary-400" />
            <Input
              type="search"
              placeholder="Search vendors or metrics..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="pl-10"
            />
          </div>
          <select
            value={selectedPeriod}
            onChange={(e) => setSelectedPeriod(e.target.value as BaselinePeriod | 'all')}
            className="h-10 rounded-lg border border-primary-200 bg-white px-3 text-sm"
          >
            <option value="all">All Periods</option>
            <option value="rolling_30_days">Rolling 30 Days</option>
            <option value="rolling_90_days">Rolling 90 Days</option>
            <option value="rolling_365_days">Rolling 365 Days</option>
          </select>
        </div>
      </Card>

      {/* Baselines List */}
      <div className="space-y-4">
        {Object.entries(groupedBaselines).map(([vendor, baselines]) => (
          <VendorBaselineCard
            key={vendor}
            vendor={vendor}
            baselines={baselines}
            expanded={expandedVendors.has(vendor)}
            onToggle={() => toggleVendor(vendor)}
          />
        ))}

        {Object.keys(groupedBaselines).length === 0 && (
          <Card className="p-8 text-center">
            <TrendingDown className="mx-auto h-8 w-8 text-primary-300" />
            <p className="mt-2 text-primary-500">No baselines found matching your filters.</p>
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

interface VendorBaselineCardProps {
  vendor: string
  baselines: VendorBaseline[]
  expanded: boolean
  onToggle: () => void
}

function VendorBaselineCard({ vendor, baselines, expanded, onToggle }: VendorBaselineCardProps) {
  const invoiceBaseline = baselines.find((b) => b.metric_name === 'invoice_total')

  return (
    <Card>
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
              <CardTitle className="text-lg">{vendor}</CardTitle>
              <p className="text-sm text-primary-500">
                {baselines.length} metric{baselines.length !== 1 ? 's' : ''} tracked
              </p>
            </div>
          </div>
          {invoiceBaseline && (
            <div className="text-right">
              <p className="text-sm text-primary-500">Avg Invoice</p>
              <p className="text-lg font-semibold text-primary-900">
                ${invoiceBaseline.stats.mean.toLocaleString()}
              </p>
            </div>
          )}
        </div>
      </CardHeader>

      {expanded && (
        <CardContent className="pt-0">
          <div className="space-y-4">
            {baselines.map((baseline) => (
              <MetricRow key={baseline.id} baseline={baseline} />
            ))}
          </div>
        </CardContent>
      )}
    </Card>
  )
}

function MetricRow({ baseline }: { baseline: VendorBaseline }) {
  const { stats } = baseline
  const lastValue = baseline.recent_values[baseline.recent_values.length - 1]
  const zScore = Math.abs((lastValue - stats.mean) / stats.stddev)
  const isOutlier = zScore > 2

  // Simple trend calculation
  const recentAvg =
    baseline.recent_values.slice(-4).reduce((a, b) => a + b, 0) /
    Math.min(4, baseline.recent_values.length)
  const trend = recentAvg > stats.mean ? 'up' : recentAvg < stats.mean ? 'down' : 'stable'

  const formatMetricName = (name: string) => {
    return name
      .split('_')
      .map((w) => w.charAt(0).toUpperCase() + w.slice(1))
      .join(' ')
  }

  const periodLabel = {
    rolling_30_days: '30 days',
    rolling_90_days: '90 days',
    rolling_365_days: '365 days',
  }[baseline.period] || baseline.period

  return (
    <div className="rounded-lg border border-primary-100 p-4">
      <div className="flex items-start justify-between">
        <div>
          <div className="flex items-center gap-2">
            <h4 className="font-medium text-primary-900">
              {formatMetricName(baseline.metric_name)}
            </h4>
            <span className="text-xs text-primary-400">({periodLabel})</span>
            {isOutlier && (
              <span className="flex items-center gap-1 text-xs text-yellow-600">
                <AlertTriangle className="h-3 w-3" />
                Outlier detected
              </span>
            )}
          </div>
          <p className="mt-1 text-sm text-primary-500">
            {stats.count} samples · Updated {new Date(baseline.computed_at).toLocaleDateString()}
          </p>
        </div>
        <div className="flex items-center gap-2">
          {trend === 'up' && <TrendingUp className="h-4 w-4 text-red-500" />}
          {trend === 'down' && <TrendingDown className="h-4 w-4 text-green-500" />}
          {trend === 'stable' && <CheckCircle className="h-4 w-4 text-primary-400" />}
        </div>
      </div>

      {/* Stats Grid */}
      <div className="mt-4 grid grid-cols-2 gap-4 sm:grid-cols-4 lg:grid-cols-6">
        <StatValue label="Mean" value={`$${stats.mean.toLocaleString()}`} />
        <StatValue label="Std Dev" value={`$${stats.stddev.toLocaleString()}`} />
        <StatValue label="Min" value={`$${stats.min.toLocaleString()}`} />
        <StatValue label="Max" value={`$${stats.max.toLocaleString()}`} />
        <StatValue label="P50" value={`$${stats.p50.toLocaleString()}`} />
        <StatValue label="P95" value={`$${stats.p95.toLocaleString()}`} />
      </div>

      {/* Mini Sparkline (simplified bar chart) */}
      <div className="mt-4">
        <p className="text-xs text-primary-400 mb-2">Recent Values</p>
        <div className="flex items-end gap-1 h-8">
          {baseline.recent_values.map((value, i) => {
            const height = ((value - stats.min) / (stats.max - stats.min)) * 100
            const isLast = i === baseline.recent_values.length - 1
            return (
              <div
                key={i}
                className={cn(
                  'flex-1 rounded-t transition-all',
                  isLast ? 'bg-primary-600' : 'bg-primary-200',
                  isOutlier && isLast && 'bg-yellow-500'
                )}
                style={{ height: `${Math.max(height, 10)}%` }}
                title={`$${value.toLocaleString()}`}
              />
            )
          })}
        </div>
      </div>
    </div>
  )
}

function StatValue({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <p className="text-xs text-primary-400">{label}</p>
      <p className="text-sm font-medium text-primary-700">{value}</p>
    </div>
  )
}

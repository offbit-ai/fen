import { Card } from '@/components/ui/card'
import { TrendingUp, FileText, AlertTriangle, Shield } from 'lucide-react'

export function ProductShowcase() {
  return (
    <div className="relative w-full max-w-3xl">
      {/* Dashboard Preview */}
      <Card className="rounded-2xl shadow-2xl overflow-hidden bg-white">
        <DashboardPreview />
      </Card>
    </div>
  )
}

function DashboardPreview() {
  return (
    <div className="p-6">
      {/* Header */}
      <div className="flex items-center justify-between mb-6">
        <div>
          <p className="text-sm text-primary-500">Good Morning, Sarah</p>
          <h2 className="text-lg font-semibold text-primary-900">
            Summary of document analysis
          </h2>
        </div>
        <div className="flex items-center gap-2">
          <span className="text-sm text-primary-500">This Month</span>
          <svg
            className="w-4 h-4 text-primary-400"
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24"
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={2}
              d="M19 9l-7 7-7-7"
            />
          </svg>
        </div>
      </div>

      {/* KPI Cards */}
      <div className="grid grid-cols-4 gap-4 mb-6">
        <KPICard
          icon={<FileText className="w-4 h-4" />}
          value="847"
          label="Documents"
          trend="+12%"
        />
        <KPICard
          icon={<TrendingUp className="w-4 h-4" />}
          value="$2.1M"
          label="Total Value"
          trend="+8%"
        />
        <KPICard
          icon={<AlertTriangle className="w-4 h-4" />}
          value="6.5%"
          label="Anomaly Rate"
          trend="-2%"
          trendDown
        />
        <KPICard
          icon={<Shield className="w-4 h-4" />}
          value="127"
          label="Active Rules"
          trend="+5"
        />
      </div>

      {/* Tabs */}
      <div className="flex gap-2 mb-4">
        {['Anomalies', 'Rules', 'Baselines'].map((tab, i) => (
          <button
            key={tab}
            className={`px-4 py-2 text-sm rounded-lg transition-colors ${
              i === 0
                ? 'bg-primary-900 text-white'
                : 'bg-primary-100 text-primary-600 hover:bg-primary-200'
            }`}
          >
            {tab}
          </button>
        ))}
      </div>

      {/* Recent Anomalies */}
      <div className="space-y-3">
        <div className="flex items-center justify-between">
          <span className="text-sm font-medium text-primary-700">
            Recent Anomalies
          </span>
          <button className="text-sm text-primary-500 hover:text-primary-700">
            View All
          </button>
        </div>

        <AnomalyItem
          id="INV-2024-889"
          vendor="TechParts Inc"
          amount="$45,000"
          severity="high"
        />
        <AnomalyItem
          id="INV-2024-887"
          vendor="GlobalSupply Co"
          amount="$12,300"
          severity="medium"
        />
      </div>

      {/* Mini Chart */}
      <div className="mt-6 p-4 bg-primary-50 rounded-lg">
        <div className="flex items-center justify-between mb-2">
          <span className="text-sm font-medium text-primary-700">
            Anomaly Trend
          </span>
        </div>
        <div className="h-20 flex items-end gap-1">
          {[40, 65, 45, 80, 55, 70, 50, 85, 60, 75, 65, 90].map((h, i) => (
            <div
              key={i}
              className="flex-1 bg-primary-300 rounded-t transition-all hover:bg-primary-400"
              style={{ height: `${h}%` }}
            />
          ))}
        </div>
        <div className="flex justify-between mt-2 text-xs text-primary-400">
          <span>Jan</span>
          <span>Feb</span>
          <span>Mar</span>
          <span>Apr</span>
          <span>May</span>
          <span>Jun</span>
        </div>
      </div>
    </div>
  )
}

interface KPICardProps {
  icon: React.ReactNode
  value: string
  label: string
  trend: string
  trendDown?: boolean
}

function KPICard({ icon, value, label, trend, trendDown }: KPICardProps) {
  return (
    <div className="p-4 bg-primary-50 rounded-lg">
      <div className="flex items-center justify-between mb-2">
        <div className="p-2 bg-white rounded-lg text-primary-600">{icon}</div>
        <span
          className={`text-xs font-medium ${
            trendDown ? 'text-success' : 'text-primary-500'
          }`}
        >
          {trend}
        </span>
      </div>
      <p className="text-xl font-bold text-primary-900">{value}</p>
      <p className="text-xs text-primary-500">{label}</p>
    </div>
  )
}

interface AnomalyItemProps {
  id: string
  vendor: string
  amount: string
  severity: 'high' | 'medium' | 'low'
}

function AnomalyItem({ id, vendor, amount, severity }: AnomalyItemProps) {
  const severityStyles = {
    high: 'bg-red-100 text-red-700',
    medium: 'bg-yellow-100 text-yellow-700',
    low: 'bg-green-100 text-green-700',
  }

  const severityLabels = {
    high: 'High Risk',
    medium: 'Medium',
    low: 'Low',
  }

  return (
    <div className="flex items-center justify-between p-3 bg-white border border-primary-200 rounded-lg">
      <div className="flex items-center gap-3">
        <div className="p-2 bg-primary-100 rounded-lg">
          <AlertTriangle className="w-4 h-4 text-primary-600" />
        </div>
        <div>
          <p className="text-sm font-medium text-primary-900">{id}</p>
          <p className="text-xs text-primary-500">{vendor}</p>
        </div>
      </div>
      <div className="flex items-center gap-3">
        <span className="text-sm font-medium text-primary-900">{amount}</span>
        <span
          className={`px-2 py-1 text-xs font-medium rounded-full ${severityStyles[severity]}`}
        >
          {severityLabels[severity]}
        </span>
      </div>
    </div>
  )
}

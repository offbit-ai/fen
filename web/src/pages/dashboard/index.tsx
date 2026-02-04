import { Link } from 'react-router-dom'
import {
  FileText,
  TrendingUp,
  AlertTriangle,
  Shield,
  ArrowUpRight,
  ArrowDownRight,
  ChevronRight,
  CheckCircle,
  Circle,
  Loader2,
  DollarSign,
} from 'lucide-react'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'

export function DashboardPage() {
  return (
    <div className="space-y-6">
      {/* Page Header */}
      <div>
        <h1 className="text-2xl font-bold text-primary-900">Dashboard</h1>
        <p className="text-sm text-primary-500">
          Overview of your document analysis and anomaly detection
        </p>
      </div>

      {/* KPI Cards */}
      <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-4">
        <KPICard
          title="Total Invoices"
          value="12,847"
          change="+12%"
          changeType="increase"
          icon={<FileText className="h-5 w-5" />}
          description="MTD"
        />
        <KPICard
          title="Processed Today"
          value="234"
          change="+8%"
          changeType="increase"
          icon={<TrendingUp className="h-5 w-5" />}
          description="vs avg"
        />
        <KPICard
          title="Anomalies Detected"
          value="47"
          change="-15%"
          changeType="decrease"
          icon={<AlertTriangle className="h-5 w-5" />}
          description="WoW"
          highlight
        />
        <KPICard
          title="At Risk Amount"
          value="$2.4M"
          change="+23%"
          changeType="increase"
          icon={<DollarSign className="h-5 w-5" />}
          description="MoM"
          isRisk
        />
      </div>

      {/* Processing Pipeline & Anomaly Breakdown */}
      <div className="grid gap-6 lg:grid-cols-2">
        {/* Processing Pipeline */}
        <Card>
          <CardHeader>
            <CardTitle className="text-lg">Processing Pipeline</CardTitle>
          </CardHeader>
          <CardContent>
            {/* Progress bar */}
            <div className="mb-6">
              <div className="flex items-center justify-between text-sm mb-2">
                <span className="text-primary-600">Today's Progress</span>
                <span className="font-medium text-primary-900">78%</span>
              </div>
              <div className="h-3 w-full rounded-full bg-primary-100">
                <div className="h-3 rounded-full bg-primary-600" style={{ width: '78%' }} />
              </div>
            </div>
            {/* Pipeline stages */}
            <div className="space-y-3">
              <PipelineStage stage="Ingested" count={234} status="complete" />
              <PipelineStage stage="Extracted" count={228} status="complete" />
              <PipelineStage stage="Validated" count={215} status="processing" />
              <PipelineStage stage="Pending Review" count={19} status="pending" />
            </div>
          </CardContent>
        </Card>

        {/* Anomalies by Category */}
        <Card>
          <CardHeader>
            <CardTitle className="text-lg">Anomalies by Category</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="space-y-4">
              <AnomalyCategoryRow
                category="Price Deviation"
                count={21}
                percentage={45}
                color="bg-red-500"
              />
              <AnomalyCategoryRow
                category="Duplicate Invoice"
                count={13}
                percentage={28}
                color="bg-orange-500"
              />
              <AnomalyCategoryRow
                category="Contract Mismatch"
                count={8}
                percentage={18}
                color="bg-yellow-500"
              />
              <AnomalyCategoryRow
                category="Volume Anomaly"
                count={5}
                percentage={9}
                color="bg-blue-500"
              />
            </div>
            {/* Mini legend */}
            <div className="mt-6 pt-4 border-t border-primary-100">
              <div className="flex items-center justify-between text-sm">
                <span className="text-primary-500">Total Open Anomalies</span>
                <span className="font-bold text-primary-900">47</span>
              </div>
            </div>
          </CardContent>
        </Card>
      </div>

      {/* Main Content Grid */}
      <div className="grid gap-6 lg:grid-cols-3">
        {/* Recent Anomalies - Takes 2 columns */}
        <Card className="lg:col-span-2">
          <CardHeader className="flex flex-row items-center justify-between">
            <CardTitle className="text-lg">Recent Anomalies</CardTitle>
            <Button variant="ghost" size="sm" asChild>
              <Link to="/analysis/anomalies">
                View All
                <ChevronRight className="ml-1 h-4 w-4" />
              </Link>
            </Button>
          </CardHeader>
          <CardContent>
            <div className="space-y-4">
              <AnomalyRow
                id="INV-2024-889"
                vendor="TechParts Inc"
                amount="$45,000"
                type="Price Deviation"
                severity="critical"
                time="2 hours ago"
              />
              <AnomalyRow
                id="INV-2024-887"
                vendor="GlobalSupply Co"
                amount="$12,300"
                type="Duplicate Invoice"
                severity="high"
                time="5 hours ago"
              />
              <AnomalyRow
                id="INV-2024-885"
                vendor="Acme Corp"
                amount="$8,750"
                type="Missing Contract"
                severity="medium"
                time="Yesterday"
              />
              <AnomalyRow
                id="INV-2024-882"
                vendor="Office Supplies Ltd"
                amount="$2,100"
                type="Quantity Mismatch"
                severity="low"
                time="Yesterday"
              />
            </div>
          </CardContent>
        </Card>

        {/* Quick Actions */}
        <Card>
          <CardHeader>
            <CardTitle className="text-lg">Quick Actions</CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            <Button variant="outline" className="w-full justify-start" asChild>
              <Link to="/documents/invoices/upload">
                <FileText className="mr-2 h-4 w-4" />
                Upload Documents
              </Link>
            </Button>
            <Button variant="outline" className="w-full justify-start" asChild>
              <Link to="/analysis/workbench">
                <TrendingUp className="mr-2 h-4 w-4" />
                Open Query Workbench
              </Link>
            </Button>
            <Button variant="outline" className="w-full justify-start" asChild>
              <Link to="/rules/graph">
                <Shield className="mr-2 h-4 w-4" />
                Configure Rules
              </Link>
            </Button>
            <Button variant="outline" className="w-full justify-start" asChild>
              <Link to="/analysis/reports">
                <AlertTriangle className="mr-2 h-4 w-4" />
                Generate Report
              </Link>
            </Button>
          </CardContent>
        </Card>
      </div>

      {/* Charts Row */}
      <div className="grid gap-6 lg:grid-cols-2">
        {/* Anomaly Trend Chart */}
        <Card>
          <CardHeader>
            <CardTitle className="text-lg">Anomaly Trend</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="h-64">
              <SimpleBarChart />
            </div>
          </CardContent>
        </Card>

        {/* Top Vendors by Anomalies */}
        <Card>
          <CardHeader>
            <CardTitle className="text-lg">Top Vendors by Anomalies</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="space-y-4">
              <VendorRow vendor="TechParts Inc" count={23} percentage={28} />
              <VendorRow vendor="GlobalSupply Co" count={18} percentage={22} />
              <VendorRow vendor="Acme Corp" count={15} percentage={18} />
              <VendorRow vendor="MegaLogistics" count={12} percentage={15} />
              <VendorRow vendor="Others" count={14} percentage={17} />
            </div>
          </CardContent>
        </Card>
      </div>
    </div>
  )
}

interface KPICardProps {
  title: string
  value: string
  change: string
  changeType: 'increase' | 'decrease'
  icon: React.ReactNode
  description: string
  highlight?: boolean
  isRisk?: boolean
}

function KPICard({
  title,
  value,
  change,
  changeType,
  icon,
  description,
  highlight,
  isRisk,
}: KPICardProps) {
  // Determine if the change is positive or negative for the business
  const isPositiveChange =
    (changeType === 'increase' && !highlight && !isRisk) ||
    (changeType === 'decrease' && (highlight || title === 'Anomalies Detected'))

  return (
    <Card className={cn(highlight && 'border-red-200 bg-red-50/50')}>
      <CardContent className="p-6">
        <div className="flex items-center justify-between">
          <span className="text-sm font-medium text-primary-500">{title}</span>
          <div
            className={cn(
              'rounded-lg p-2',
              highlight
                ? 'bg-red-100 text-red-600'
                : 'bg-primary-100 text-primary-600'
            )}
          >
            {icon}
          </div>
        </div>
        <div className="mt-4">
          <span
            className={cn(
              'text-3xl font-bold',
              highlight ? 'text-red-700' : 'text-primary-900'
            )}
          >
            {value}
          </span>
          <div className="mt-1 flex items-center gap-2">
            <span
              className={cn(
                'flex items-center text-sm font-medium',
                isPositiveChange ? 'text-success' : 'text-error'
              )}
            >
              {changeType === 'increase' ? (
                <ArrowUpRight className="h-4 w-4" />
              ) : (
                <ArrowDownRight className="h-4 w-4" />
              )}
              {change}
            </span>
            <span className="text-sm text-primary-400">{description}</span>
          </div>
        </div>
      </CardContent>
    </Card>
  )
}

interface PipelineStageProps {
  stage: string
  count: number
  status: 'complete' | 'processing' | 'pending'
}

function PipelineStage({ stage, count, status }: PipelineStageProps) {
  return (
    <div className="flex items-center justify-between">
      <div className="flex items-center gap-3">
        {status === 'complete' && (
          <CheckCircle className="h-5 w-5 text-success" />
        )}
        {status === 'processing' && (
          <Loader2 className="h-5 w-5 text-primary-500 animate-spin" />
        )}
        {status === 'pending' && (
          <Circle className="h-5 w-5 text-primary-300" />
        )}
        <span
          className={cn(
            'text-sm',
            status === 'complete'
              ? 'text-primary-700'
              : status === 'processing'
              ? 'text-primary-900 font-medium'
              : 'text-primary-500'
          )}
        >
          {stage}
        </span>
      </div>
      <span
        className={cn(
          'text-sm font-medium',
          status === 'processing' ? 'text-primary-900' : 'text-primary-600'
        )}
      >
        {count}
      </span>
    </div>
  )
}

interface AnomalyCategoryRowProps {
  category: string
  count: number
  percentage: number
  color: string
}

function AnomalyCategoryRow({
  category,
  count,
  percentage,
  color,
}: AnomalyCategoryRowProps) {
  return (
    <div className="space-y-2">
      <div className="flex items-center justify-between text-sm">
        <div className="flex items-center gap-2">
          <div className={cn('h-3 w-3 rounded-full', color)} />
          <span className="text-primary-700">{category}</span>
        </div>
        <span className="font-medium text-primary-900">{count}</span>
      </div>
      <div className="h-2 w-full rounded-full bg-primary-100">
        <div
          className={cn('h-2 rounded-full transition-all', color)}
          style={{ width: `${percentage}%` }}
        />
      </div>
    </div>
  )
}

interface AnomalyRowProps {
  id: string
  vendor: string
  amount: string
  type: string
  severity: 'critical' | 'high' | 'medium' | 'low'
  time: string
}

function AnomalyRow({
  id,
  vendor,
  amount,
  type,
  severity,
  time,
}: AnomalyRowProps) {
  const severityColors = {
    critical: 'bg-red-100 text-red-700 border-red-200',
    high: 'bg-orange-100 text-orange-700 border-orange-200',
    medium: 'bg-yellow-100 text-yellow-700 border-yellow-200',
    low: 'bg-green-100 text-green-700 border-green-200',
  }

  const severityLabels = {
    critical: 'Critical',
    high: 'High',
    medium: 'Medium',
    low: 'Low',
  }

  return (
    <Link
      to={`/analysis/anomalies/${id}`}
      className="flex items-center justify-between rounded-lg border border-primary-200 p-4 transition-colors hover:bg-primary-50"
    >
      <div className="flex items-center gap-4">
        <div className="rounded-lg bg-primary-100 p-2">
          <AlertTriangle className="h-4 w-4 text-primary-600" />
        </div>
        <div>
          <p className="font-medium text-primary-900">{id}</p>
          <p className="text-sm text-primary-500">{vendor}</p>
        </div>
      </div>
      <div className="flex items-center gap-4">
        <div className="text-right">
          <p className="font-medium text-primary-900">{amount}</p>
          <p className="text-sm text-primary-500">{type}</p>
        </div>
        <span
          className={cn(
            'rounded-full border px-2.5 py-0.5 text-xs font-medium',
            severityColors[severity]
          )}
        >
          {severityLabels[severity]}
        </span>
        <span className="text-sm text-primary-400">{time}</span>
      </div>
    </Link>
  )
}

function SimpleBarChart() {
  // Mock data for the chart
  const data = [
    { month: 'Jan', anomalies: 45, processed: 180 },
    { month: 'Feb', anomalies: 52, processed: 210 },
    { month: 'Mar', anomalies: 38, processed: 190 },
    { month: 'Apr', anomalies: 65, processed: 240 },
    { month: 'May', anomalies: 48, processed: 220 },
    { month: 'Jun', anomalies: 55, processed: 235 },
  ]

  const maxValue = Math.max(...data.map((d) => d.processed))

  return (
    <div className="flex h-full flex-col">
      <div className="flex flex-1 items-end gap-2">
        {data.map((item) => (
          <div key={item.month} className="flex flex-1 flex-col items-center gap-1">
            <div className="relative flex w-full flex-col items-center gap-1">
              {/* Processed bar */}
              <div
                className="w-full rounded-t bg-primary-200 transition-all hover:bg-primary-300"
                style={{
                  height: `${(item.processed / maxValue) * 160}px`,
                }}
              />
              {/* Anomalies bar (overlaid) */}
              <div
                className="absolute bottom-0 w-3/4 rounded-t bg-primary-500"
                style={{
                  height: `${(item.anomalies / maxValue) * 160}px`,
                }}
              />
            </div>
          </div>
        ))}
      </div>
      <div className="mt-2 flex">
        {data.map((item) => (
          <div key={item.month} className="flex-1 text-center text-xs text-primary-500">
            {item.month}
          </div>
        ))}
      </div>
      {/* Legend */}
      <div className="mt-4 flex items-center justify-center gap-6">
        <div className="flex items-center gap-2">
          <div className="h-3 w-3 rounded bg-primary-200" />
          <span className="text-xs text-primary-500">Processed</span>
        </div>
        <div className="flex items-center gap-2">
          <div className="h-3 w-3 rounded bg-primary-500" />
          <span className="text-xs text-primary-500">Anomalies</span>
        </div>
      </div>
    </div>
  )
}

interface VendorRowProps {
  vendor: string
  count: number
  percentage: number
}

function VendorRow({ vendor, count, percentage }: VendorRowProps) {
  return (
    <div className="space-y-2">
      <div className="flex items-center justify-between text-sm">
        <span className="text-primary-700">{vendor}</span>
        <span className="font-medium text-primary-900">{count} anomalies</span>
      </div>
      <div className="h-2 w-full rounded-full bg-primary-100">
        <div
          className="h-2 rounded-full bg-primary-500 transition-all"
          style={{ width: `${percentage}%` }}
        />
      </div>
    </div>
  )
}

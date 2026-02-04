import { useState } from 'react'
import {
  BarChart3,
  Download,
  Calendar,
  Filter,
  FileText,
  TrendingUp,
  AlertTriangle,
  DollarSign,
  Clock,
  ChevronRight,
  RefreshCw,
} from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Card, CardContent } from '@/components/ui/card'
import { cn } from '@/lib/utils'

type ReportType = 'all' | 'anomaly' | 'vendor' | 'financial' | 'compliance'
type ReportStatus = 'ready' | 'generating' | 'scheduled'

interface Report {
  id: string
  name: string
  type: ReportType
  description: string
  lastGenerated?: string
  status: ReportStatus
  schedule?: string
}

const mockReports: Report[] = [
  {
    id: 'rpt-001',
    name: 'Weekly Anomaly Summary',
    type: 'anomaly',
    description: 'Summary of all anomalies detected in the past 7 days',
    lastGenerated: '2024-02-04T08:00:00Z',
    status: 'ready',
    schedule: 'Weekly on Mondays',
  },
  {
    id: 'rpt-002',
    name: 'Vendor Spend Analysis',
    type: 'vendor',
    description: 'Breakdown of spending by vendor with trend analysis',
    lastGenerated: '2024-02-01T00:00:00Z',
    status: 'ready',
    schedule: 'Monthly',
  },
  {
    id: 'rpt-003',
    name: 'Invoice Processing Metrics',
    type: 'financial',
    description: 'Processing volumes, cycle times, and accuracy rates',
    lastGenerated: '2024-02-03T12:00:00Z',
    status: 'ready',
  },
  {
    id: 'rpt-004',
    name: 'Contract Compliance Report',
    type: 'compliance',
    description: 'Contract terms compliance and pricing deviation analysis',
    status: 'generating',
  },
  {
    id: 'rpt-005',
    name: 'Monthly Executive Summary',
    type: 'financial',
    description: 'High-level overview of AP operations and key metrics',
    lastGenerated: '2024-01-31T00:00:00Z',
    status: 'ready',
    schedule: 'Monthly on 1st',
  },
  {
    id: 'rpt-006',
    name: 'Duplicate Detection Report',
    type: 'anomaly',
    description: 'List of potential duplicate invoices requiring review',
    status: 'scheduled',
    schedule: 'Daily at 6:00 AM',
  },
]

const tabs: { id: ReportType; label: string }[] = [
  { id: 'all', label: 'All Reports' },
  { id: 'anomaly', label: 'Anomaly' },
  { id: 'vendor', label: 'Vendor' },
  { id: 'financial', label: 'Financial' },
  { id: 'compliance', label: 'Compliance' },
]

export function ReportsPage() {
  const [selectedTab, setSelectedTab] = useState<ReportType>('all')
  const [dateRange, setDateRange] = useState('last_30')

  const filteredReports = mockReports.filter(
    (r) => selectedTab === 'all' || r.type === selectedTab
  )

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-primary-900">Reports</h1>
          <p className="text-sm text-primary-500">
            Generate and view analysis reports
          </p>
        </div>
        <div className="flex items-center gap-2">
          <Button variant="outline">
            <Calendar className="mr-2 h-4 w-4" />
            Schedule Report
          </Button>
          <Button>
            <BarChart3 className="mr-2 h-4 w-4" />
            Create Report
          </Button>
        </div>
      </div>

      {/* Quick Stats */}
      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
        <QuickStatCard
          title="Reports Generated"
          value="127"
          subtext="This month"
          icon={<FileText className="h-5 w-5" />}
        />
        <QuickStatCard
          title="Scheduled Reports"
          value="8"
          subtext="Active schedules"
          icon={<Clock className="h-5 w-5" />}
        />
        <QuickStatCard
          title="Total Downloads"
          value="342"
          subtext="Last 30 days"
          icon={<Download className="h-5 w-5" />}
        />
        <QuickStatCard
          title="Processing"
          value="2"
          subtext="Reports generating"
          icon={<RefreshCw className="h-5 w-5" />}
          highlight
        />
      </div>

      {/* Filters */}
      <Card className="p-4">
        <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
          <div className="flex gap-1">
            {tabs.map((tab) => (
              <button
                key={tab.id}
                onClick={() => setSelectedTab(tab.id)}
                className={cn(
                  'px-3 py-1.5 text-sm font-medium rounded-lg transition-colors',
                  selectedTab === tab.id
                    ? 'bg-primary-900 text-white'
                    : 'text-primary-600 hover:bg-primary-100'
                )}
              >
                {tab.label}
              </button>
            ))}
          </div>
          <div className="flex items-center gap-2">
            <select
              value={dateRange}
              onChange={(e) => setDateRange(e.target.value)}
              className="h-9 rounded-lg border border-primary-200 bg-white px-3 text-sm"
            >
              <option value="last_7">Last 7 days</option>
              <option value="last_30">Last 30 days</option>
              <option value="last_90">Last 90 days</option>
              <option value="custom">Custom range</option>
            </select>
            <Button variant="outline" size="sm">
              <Filter className="mr-2 h-4 w-4" />
              More Filters
            </Button>
          </div>
        </div>
      </Card>

      {/* Reports List */}
      <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
        {filteredReports.map((report) => (
          <ReportCard key={report.id} report={report} />
        ))}
      </div>

      {filteredReports.length === 0 && (
        <Card className="p-8 text-center">
          <BarChart3 className="mx-auto h-8 w-8 text-primary-300" />
          <p className="mt-2 text-primary-500">No reports found for this category.</p>
        </Card>
      )}
    </div>
  )
}

function QuickStatCard({
  title,
  value,
  subtext,
  icon,
  highlight,
}: {
  title: string
  value: string
  subtext: string
  icon: React.ReactNode
  highlight?: boolean
}) {
  return (
    <Card className={cn(highlight && 'border-primary-300 bg-primary-50')}>
      <CardContent className="p-4">
        <div className="flex items-center justify-between">
          <div>
            <p className="text-sm text-primary-500">{title}</p>
            <p className="text-2xl font-bold text-primary-900">{value}</p>
            <p className="text-xs text-primary-400">{subtext}</p>
          </div>
          <div className={cn('rounded-lg p-2', highlight ? 'bg-primary-200' : 'bg-primary-100')}>
            {icon}
          </div>
        </div>
      </CardContent>
    </Card>
  )
}

function ReportCard({ report }: { report: Report }) {
  const typeIcons: Record<ReportType, React.ReactNode> = {
    all: <FileText className="h-5 w-5" />,
    anomaly: <AlertTriangle className="h-5 w-5" />,
    vendor: <TrendingUp className="h-5 w-5" />,
    financial: <DollarSign className="h-5 w-5" />,
    compliance: <FileText className="h-5 w-5" />,
  }

  const typeColors: Record<ReportType, string> = {
    all: 'bg-primary-100 text-primary-600',
    anomaly: 'bg-red-100 text-red-600',
    vendor: 'bg-blue-100 text-blue-600',
    financial: 'bg-green-100 text-green-600',
    compliance: 'bg-purple-100 text-purple-600',
  }

  const statusBadge: Record<ReportStatus, { label: string; className: string }> = {
    ready: { label: 'Ready', className: 'bg-green-100 text-green-700' },
    generating: { label: 'Generating...', className: 'bg-yellow-100 text-yellow-700' },
    scheduled: { label: 'Scheduled', className: 'bg-blue-100 text-blue-700' },
  }

  return (
    <Card className="hover:shadow-md transition-shadow">
      <CardContent className="p-5">
        <div className="flex items-start justify-between">
          <div className={cn('rounded-lg p-2', typeColors[report.type])}>
            {typeIcons[report.type]}
          </div>
          <span
            className={cn(
              'rounded-full px-2 py-0.5 text-xs font-medium',
              statusBadge[report.status].className
            )}
          >
            {statusBadge[report.status].label}
          </span>
        </div>

        <h3 className="mt-4 font-semibold text-primary-900">{report.name}</h3>
        <p className="mt-1 text-sm text-primary-500 line-clamp-2">{report.description}</p>

        {report.lastGenerated && (
          <p className="mt-3 text-xs text-primary-400">
            Last generated: {new Date(report.lastGenerated).toLocaleDateString()}
          </p>
        )}

        {report.schedule && (
          <p className="mt-1 text-xs text-primary-400">
            Schedule: {report.schedule}
          </p>
        )}

        <div className="mt-4 flex items-center gap-2">
          {report.status === 'ready' && (
            <>
              <Button size="sm" variant="outline" className="flex-1">
                <Download className="mr-2 h-4 w-4" />
                Download
              </Button>
              <Button size="sm" variant="ghost">
                <ChevronRight className="h-4 w-4" />
              </Button>
            </>
          )}
          {report.status === 'generating' && (
            <Button size="sm" variant="outline" className="flex-1" disabled>
              <RefreshCw className="mr-2 h-4 w-4 animate-spin" />
              Generating...
            </Button>
          )}
          {report.status === 'scheduled' && (
            <Button size="sm" variant="outline" className="flex-1">
              Run Now
            </Button>
          )}
        </div>
      </CardContent>
    </Card>
  )
}

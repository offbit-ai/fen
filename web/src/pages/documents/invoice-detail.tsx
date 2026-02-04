import { Link, useParams } from 'react-router-dom'
import {
  ArrowLeft,
  FileText,
  AlertTriangle,
  CheckCircle,
  Clock,
  Download,
  MoreHorizontal,
  Building2,
  Calendar,
  DollarSign,
  Hash,
} from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { cn } from '@/lib/utils'
import type { Invoice, Anomaly, AnomalySeverity } from '@/types/api'

// Mock data for development
const mockInvoice: Invoice = {
  id: 'inv-001',
  tenant_id: 'tenant-1',
  vendor_name: 'TechParts Inc',
  invoice_number: 'INV-2024-889',
  invoice_date: '2024-02-01',
  due_date: '2024-03-01',
  total_amount: 45000,
  currency: 'USD',
  status: 'flagged',
  line_items: [
    { id: 'li-1', description: 'Widget A - Premium', quantity: 50, unit_price: 450, total: 22500, category: 'Parts' },
    { id: 'li-2', description: 'Widget B - Standard', quantity: 100, unit_price: 150, total: 15000, category: 'Parts' },
    { id: 'li-3', description: 'Installation Service', quantity: 1, unit_price: 5000, total: 5000, category: 'Services' },
    { id: 'li-4', description: 'Shipping & Handling', quantity: 1, unit_price: 2500, total: 2500, category: 'Shipping' },
  ],
  anomalies: [],
  metadata: {
    po_number: 'PO-2024-1234',
    payment_terms: 'Net 30',
  },
  created_at: '2024-02-01T10:00:00Z',
  updated_at: '2024-02-01T10:00:00Z',
}

const mockAnomalies: Anomaly[] = [
  {
    id: 'anom-1',
    document_id: 'inv-001',
    document_type: 'invoice',
    tenant_id: 'tenant-1',
    anomaly_type: 'price_deviation',
    severity: 'high',
    confidence: 0.92,
    description: 'Unit price for "Widget A - Premium" is 35% higher than the historical average of $333.33',
    details: {
      field: 'line_items[0].unit_price',
      expected: 333.33,
      actual: 450,
      deviation_percent: 35,
    },
    status: 'open',
    created_at: '2024-02-01T10:05:00Z',
  },
  {
    id: 'anom-2',
    document_id: 'inv-001',
    document_type: 'invoice',
    tenant_id: 'tenant-1',
    anomaly_type: 'statistical_outlier',
    severity: 'medium',
    confidence: 0.78,
    description: 'Total invoice amount exceeds 2 standard deviations from vendor average',
    details: {
      field: 'total_amount',
      vendor_average: 28000,
      std_dev: 8500,
      z_score: 2.0,
    },
    status: 'investigating',
    created_at: '2024-02-01T10:05:00Z',
  },
]

export function InvoiceDetailPage() {
  const { id: _id } = useParams<{ id: string }>()

  // In real app, fetch invoice by id
  const invoice = mockInvoice
  const anomalies = mockAnomalies

  return (
    <div className="space-y-6">
      {/* Breadcrumb */}
      <div className="flex items-center gap-2 text-sm">
        <Link
          to="/documents/invoices"
          className="flex items-center gap-1 text-primary-500 hover:text-primary-700"
        >
          <ArrowLeft className="h-4 w-4" />
          Back to Invoices
        </Link>
      </div>

      {/* Header */}
      <div className="flex items-start justify-between">
        <div className="flex items-center gap-4">
          <div className="flex h-12 w-12 items-center justify-center rounded-lg bg-primary-100">
            <FileText className="h-6 w-6 text-primary-600" />
          </div>
          <div>
            <h1 className="text-2xl font-bold text-primary-900">
              {invoice.invoice_number}
            </h1>
            <p className="text-sm text-primary-500">{invoice.vendor_name}</p>
          </div>
        </div>
        <div className="flex items-center gap-2">
          <Button variant="outline">
            <Download className="mr-2 h-4 w-4" />
            Download PDF
          </Button>
          <Button variant="outline" size="icon">
            <MoreHorizontal className="h-4 w-4" />
          </Button>
        </div>
      </div>

      {/* Anomaly Alert */}
      {anomalies.length > 0 && (
        <Card className="border-red-200 bg-red-50">
          <CardContent className="p-4">
            <div className="flex items-center gap-3">
              <AlertTriangle className="h-5 w-5 text-red-600" />
              <div>
                <p className="font-medium text-red-900">
                  {anomalies.length} anomaly detected
                </p>
                <p className="text-sm text-red-700">
                  Review the anomalies below before approving this invoice.
                </p>
              </div>
            </div>
          </CardContent>
        </Card>
      )}

      <div className="grid gap-6 lg:grid-cols-3">
        {/* Main Content */}
        <div className="lg:col-span-2 space-y-6">
          {/* Invoice Details */}
          <Card>
            <CardHeader>
              <CardTitle>Invoice Details</CardTitle>
            </CardHeader>
            <CardContent>
              <div className="grid gap-4 sm:grid-cols-2">
                <DetailItem
                  icon={<Building2 className="h-4 w-4" />}
                  label="Vendor"
                  value={invoice.vendor_name}
                />
                <DetailItem
                  icon={<Hash className="h-4 w-4" />}
                  label="Invoice Number"
                  value={invoice.invoice_number}
                />
                <DetailItem
                  icon={<Calendar className="h-4 w-4" />}
                  label="Invoice Date"
                  value={new Date(invoice.invoice_date).toLocaleDateString()}
                />
                <DetailItem
                  icon={<Calendar className="h-4 w-4" />}
                  label="Due Date"
                  value={invoice.due_date ? new Date(invoice.due_date).toLocaleDateString() : '—'}
                />
                <DetailItem
                  icon={<DollarSign className="h-4 w-4" />}
                  label="Total Amount"
                  value={formatCurrency(invoice.total_amount, invoice.currency)}
                  highlight
                />
                <DetailItem
                  icon={<Clock className="h-4 w-4" />}
                  label="Status"
                  value={
                    <span
                      className={cn(
                        'inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-xs font-medium',
                        invoice.status === 'flagged'
                          ? 'bg-red-100 text-red-700'
                          : invoice.status === 'approved'
                          ? 'bg-green-100 text-green-700'
                          : 'bg-yellow-100 text-yellow-700'
                      )}
                    >
                      {invoice.status.charAt(0).toUpperCase() + invoice.status.slice(1)}
                    </span>
                  }
                />
              </div>
            </CardContent>
          </Card>

          {/* Line Items */}
          <Card>
            <CardHeader>
              <CardTitle>Line Items</CardTitle>
            </CardHeader>
            <CardContent className="p-0">
              <div className="overflow-x-auto">
                <table className="w-full">
                  <thead>
                    <tr className="border-b border-primary-200 bg-primary-50">
                      <th className="px-4 py-3 text-left text-sm font-medium text-primary-700">
                        Description
                      </th>
                      <th className="px-4 py-3 text-right text-sm font-medium text-primary-700">
                        Qty
                      </th>
                      <th className="px-4 py-3 text-right text-sm font-medium text-primary-700">
                        Unit Price
                      </th>
                      <th className="px-4 py-3 text-right text-sm font-medium text-primary-700">
                        Total
                      </th>
                    </tr>
                  </thead>
                  <tbody>
                    {invoice.line_items.map((item, index) => {
                      const hasAnomaly = anomalies.some(
                        (a) => (a.details as { field?: string })?.field?.includes(`line_items[${index}]`)
                      )
                      return (
                        <tr
                          key={item.id}
                          className={cn(
                            'border-b border-primary-100',
                            hasAnomaly && 'bg-red-50'
                          )}
                        >
                          <td className="px-4 py-3">
                            <div className="flex items-center gap-2">
                              {hasAnomaly && (
                                <AlertTriangle className="h-4 w-4 text-red-500" />
                              )}
                              <span className="text-primary-900">{item.description}</span>
                            </div>
                            {item.category && (
                              <span className="text-xs text-primary-500">
                                {item.category}
                              </span>
                            )}
                          </td>
                          <td className="px-4 py-3 text-right text-primary-700">
                            {item.quantity}
                          </td>
                          <td className="px-4 py-3 text-right text-primary-700">
                            {formatCurrency(item.unit_price, invoice.currency)}
                          </td>
                          <td className="px-4 py-3 text-right font-medium text-primary-900">
                            {formatCurrency(item.total, invoice.currency)}
                          </td>
                        </tr>
                      )
                    })}
                  </tbody>
                  <tfoot>
                    <tr className="bg-primary-50">
                      <td colSpan={3} className="px-4 py-3 text-right font-medium text-primary-700">
                        Total
                      </td>
                      <td className="px-4 py-3 text-right font-bold text-primary-900">
                        {formatCurrency(invoice.total_amount, invoice.currency)}
                      </td>
                    </tr>
                  </tfoot>
                </table>
              </div>
            </CardContent>
          </Card>

          {/* Anomalies */}
          {anomalies.length > 0 && (
            <Card>
              <CardHeader>
                <CardTitle className="flex items-center gap-2">
                  <AlertTriangle className="h-5 w-5 text-red-500" />
                  Detected Anomalies
                </CardTitle>
              </CardHeader>
              <CardContent className="space-y-4">
                {anomalies.map((anomaly) => (
                  <AnomalyCard key={anomaly.id} anomaly={anomaly} />
                ))}
              </CardContent>
            </Card>
          )}
        </div>

        {/* Sidebar */}
        <div className="space-y-6">
          {/* Actions */}
          <Card>
            <CardHeader>
              <CardTitle>Actions</CardTitle>
            </CardHeader>
            <CardContent className="space-y-3">
              <Button className="w-full" variant="default">
                <CheckCircle className="mr-2 h-4 w-4" />
                Approve Invoice
              </Button>
              <Button className="w-full" variant="outline">
                Request More Info
              </Button>
              <Button className="w-full text-red-600 hover:text-red-700" variant="outline">
                Reject Invoice
              </Button>
            </CardContent>
          </Card>

          {/* Metadata */}
          <Card>
            <CardHeader>
              <CardTitle>Additional Info</CardTitle>
            </CardHeader>
            <CardContent>
              <dl className="space-y-3">
                {Object.entries(invoice.metadata).map(([key, value]) => (
                  <div key={key}>
                    <dt className="text-xs font-medium text-primary-500 uppercase">
                      {key.replace(/_/g, ' ')}
                    </dt>
                    <dd className="text-sm text-primary-900">{String(value)}</dd>
                  </div>
                ))}
                <div>
                  <dt className="text-xs font-medium text-primary-500 uppercase">
                    Created
                  </dt>
                  <dd className="text-sm text-primary-900">
                    {new Date(invoice.created_at).toLocaleString()}
                  </dd>
                </div>
              </dl>
            </CardContent>
          </Card>
        </div>
      </div>
    </div>
  )
}

function DetailItem({
  icon,
  label,
  value,
  highlight,
}: {
  icon: React.ReactNode
  label: string
  value: React.ReactNode
  highlight?: boolean
}) {
  return (
    <div className="flex items-start gap-3">
      <div className="mt-0.5 text-primary-400">{icon}</div>
      <div>
        <dt className="text-xs font-medium text-primary-500">{label}</dt>
        <dd className={cn('text-sm', highlight ? 'text-lg font-bold text-primary-900' : 'text-primary-900')}>
          {value}
        </dd>
      </div>
    </div>
  )
}

function AnomalyCard({ anomaly }: { anomaly: Anomaly }) {
  const severityColors: Record<AnomalySeverity, string> = {
    critical: 'border-red-300 bg-red-50',
    high: 'border-orange-300 bg-orange-50',
    medium: 'border-yellow-300 bg-yellow-50',
    low: 'border-green-300 bg-green-50',
  }

  const severityBadge: Record<AnomalySeverity, string> = {
    critical: 'bg-red-100 text-red-700',
    high: 'bg-orange-100 text-orange-700',
    medium: 'bg-yellow-100 text-yellow-700',
    low: 'bg-green-100 text-green-700',
  }

  return (
    <div className={cn('rounded-lg border p-4', severityColors[anomaly.severity])}>
      <div className="flex items-start justify-between">
        <div>
          <div className="flex items-center gap-2">
            <span
              className={cn(
                'rounded-full px-2 py-0.5 text-xs font-medium',
                severityBadge[anomaly.severity]
              )}
            >
              {anomaly.severity.toUpperCase()}
            </span>
            <span className="text-xs text-primary-500">
              {Math.round(anomaly.confidence * 100)}% confidence
            </span>
          </div>
          <p className="mt-2 text-sm text-primary-900">{anomaly.description}</p>
        </div>
      </div>
      <div className="mt-3 flex gap-2">
        <Button size="sm" variant="outline">
          Investigate
        </Button>
        <Button size="sm" variant="ghost">
          Dismiss
        </Button>
      </div>
    </div>
  )
}

function formatCurrency(amount: number, currency: string): string {
  return new Intl.NumberFormat('en-US', {
    style: 'currency',
    currency,
  }).format(amount)
}

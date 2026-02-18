import { useState, useMemo } from 'react'
import { Link } from 'react-router-dom'
import {
  FileText,
  Search,
  Filter,
  Download,
  Upload,
  ChevronLeft,
  ChevronRight,
  AlertTriangle,
  CheckCircle,
  Clock,
  XCircle,
  Loader2,
  WifiOff,
} from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Card } from '@/components/ui/card'
import { cn } from '@/lib/utils'
import { useInvoices } from '@/hooks/use-documents'
import { UploadDialog } from '@/components/upload-dialog'
import type { Invoice, InvoiceStatus } from '@/types/api'

// Mock data fallback when API is unavailable
const MOCK_INVOICES: Invoice[] = [
  {
    id: 'inv-001',
    tenant_id: 'tenant-1',
    vendor_name: 'TechParts Inc',
    invoice_number: 'INV-2024-889',
    invoice_date: '2024-02-01',
    due_date: '2024-03-01',
    total_amount: 45000,
    currency: 'USD',
    status: 'flagged',
    line_items: [],
    anomalies: [{ id: 'a1' }] as never[],
    metadata: {},
    created_at: '2024-02-01T10:00:00Z',
    updated_at: '2024-02-01T10:00:00Z',
  },
  {
    id: 'inv-002',
    tenant_id: 'tenant-1',
    vendor_name: 'GlobalSupply Co',
    invoice_number: 'INV-2024-887',
    invoice_date: '2024-01-28',
    due_date: '2024-02-28',
    total_amount: 12300,
    currency: 'USD',
    status: 'validated',
    line_items: [],
    anomalies: [],
    metadata: {},
    created_at: '2024-01-28T09:00:00Z',
    updated_at: '2024-01-28T09:00:00Z',
  },
  {
    id: 'inv-003',
    tenant_id: 'tenant-1',
    vendor_name: 'Acme Corp',
    invoice_number: 'INV-2024-885',
    invoice_date: '2024-01-25',
    due_date: '2024-02-25',
    total_amount: 8750,
    currency: 'USD',
    status: 'pending',
    line_items: [],
    anomalies: [],
    metadata: {},
    created_at: '2024-01-25T14:00:00Z',
    updated_at: '2024-01-25T14:00:00Z',
  },
  {
    id: 'inv-004',
    tenant_id: 'tenant-1',
    vendor_name: 'Office Supplies Ltd',
    invoice_number: 'INV-2024-882',
    invoice_date: '2024-01-22',
    due_date: '2024-02-22',
    total_amount: 2100,
    currency: 'USD',
    status: 'approved',
    line_items: [],
    anomalies: [],
    metadata: {},
    created_at: '2024-01-22T11:00:00Z',
    updated_at: '2024-01-22T11:00:00Z',
  },
  {
    id: 'inv-005',
    tenant_id: 'tenant-1',
    vendor_name: 'MegaLogistics',
    invoice_number: 'INV-2024-880',
    invoice_date: '2024-01-20',
    due_date: '2024-02-20',
    total_amount: 34500,
    currency: 'USD',
    status: 'rejected',
    line_items: [],
    anomalies: [{ id: 'a2' }, { id: 'a3' }] as never[],
    metadata: {},
    created_at: '2024-01-20T16:00:00Z',
    updated_at: '2024-01-20T16:00:00Z',
  },
]

export function InvoicesPage() {
  const [searchQuery, setSearchQuery] = useState('')
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set())
  const [currentPage, setCurrentPage] = useState(1)
  const [uploadOpen, setUploadOpen] = useState(false)
  const pageSize = 10

  const { data: apiData, isLoading, error, refetch } = useInvoices({
    page: currentPage,
    page_size: pageSize,
    vendor: searchQuery || undefined,
  })

  const isApiConnected = !error
  const invoices = useMemo(() => {
    if (apiData?.items?.length) return apiData.items
    if (isLoading) return []
    return MOCK_INVOICES
  }, [apiData, isLoading])
  const totalItems = apiData?.total ?? MOCK_INVOICES.length
  const totalPages = Math.ceil(totalItems / pageSize)

  const toggleSelection = (id: string) => {
    const newSelection = new Set(selectedIds)
    if (newSelection.has(id)) {
      newSelection.delete(id)
    } else {
      newSelection.add(id)
    }
    setSelectedIds(newSelection)
  }

  const toggleAll = () => {
    if (selectedIds.size === invoices.length) {
      setSelectedIds(new Set())
    } else {
      setSelectedIds(new Set(invoices.map((i) => i.id)))
    }
  }

  return (
    <div className="space-y-6">
      {/* Page Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-primary-900">Invoices</h1>
          <p className="text-sm text-primary-500">
            Manage and analyze your invoice documents
          </p>
        </div>
        <div className="flex items-center gap-3">
          {!isApiConnected && (
            <div className="flex items-center gap-2 rounded-lg border border-amber-200 bg-amber-50 px-3 py-1.5 text-xs text-amber-700">
              <WifiOff className="h-3.5 w-3.5" />
              API offline &middot; showing sample data
            </div>
          )}
          <Button onClick={() => setUploadOpen(true)}>
            <Upload className="mr-2 h-4 w-4" />
            Upload Invoice
          </Button>
        </div>
      </div>

      {/* Filters and Search */}
      <Card className="p-4">
        <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
          <div className="relative flex-1 max-w-md">
            <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-primary-400" />
            <Input
              type="search"
              placeholder="Search invoices..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="pl-10"
            />
          </div>
          <div className="flex items-center gap-2">
            <Button variant="outline" size="sm">
              <Filter className="mr-2 h-4 w-4" />
              Filters
            </Button>
            <Button variant="outline" size="sm">
              <Download className="mr-2 h-4 w-4" />
              Export
            </Button>
          </div>
        </div>
      </Card>

      {/* Bulk Actions */}
      {selectedIds.size > 0 && (
        <Card className="p-4 bg-primary-50">
          <div className="flex items-center justify-between">
            <span className="text-sm text-primary-700">
              {selectedIds.size} invoice(s) selected
            </span>
            <div className="flex items-center gap-2">
              <Button variant="outline" size="sm">
                Approve Selected
              </Button>
              <Button variant="outline" size="sm">
                Export Selected
              </Button>
            </div>
          </div>
        </Card>
      )}

      {/* Data Table */}
      <Card>
        <div className="overflow-x-auto">
          <table className="w-full">
            <thead>
              <tr className="border-b border-primary-200 bg-primary-50">
                <th className="px-4 py-3 text-left">
                  <input
                    type="checkbox"
                    checked={invoices.length > 0 && selectedIds.size === invoices.length}
                    onChange={toggleAll}
                    className="h-4 w-4 rounded border-primary-300"
                  />
                </th>
                <th className="px-4 py-3 text-left text-sm font-medium text-primary-700">
                  Invoice
                </th>
                <th className="px-4 py-3 text-left text-sm font-medium text-primary-700">
                  Vendor
                </th>
                <th className="px-4 py-3 text-left text-sm font-medium text-primary-700">
                  Date
                </th>
                <th className="px-4 py-3 text-right text-sm font-medium text-primary-700">
                  Amount
                </th>
                <th className="px-4 py-3 text-center text-sm font-medium text-primary-700">
                  Status
                </th>
                <th className="px-4 py-3 text-center text-sm font-medium text-primary-700">
                  Anomalies
                </th>
                <th className="px-4 py-3 text-right text-sm font-medium text-primary-700">
                  Actions
                </th>
              </tr>
            </thead>
            <tbody>
              {isLoading ? (
                <tr>
                  <td colSpan={8} className="px-4 py-12 text-center">
                    <Loader2 className="mx-auto h-6 w-6 animate-spin text-primary-400" />
                    <p className="mt-2 text-sm text-primary-500">Loading invoices...</p>
                  </td>
                </tr>
              ) : invoices.length === 0 ? (
                <tr>
                  <td colSpan={8} className="px-4 py-12 text-center">
                    <FileText className="mx-auto h-8 w-8 text-primary-300" />
                    <p className="mt-2 text-sm text-primary-500">No invoices found</p>
                  </td>
                </tr>
              ) : (
                invoices.map((invoice) => (
                  <tr
                    key={invoice.id}
                    className="border-b border-primary-100 hover:bg-primary-50 transition-colors"
                  >
                    <td className="px-4 py-3">
                      <input
                        type="checkbox"
                        checked={selectedIds.has(invoice.id)}
                        onChange={() => toggleSelection(invoice.id)}
                        className="h-4 w-4 rounded border-primary-300"
                      />
                    </td>
                    <td className="px-4 py-3">
                      <Link
                        to={`/app/documents/invoices/${invoice.id}`}
                        className="flex items-center gap-2 hover:text-accent"
                      >
                        <FileText className="h-4 w-4 text-primary-400" />
                        <span className="font-medium text-primary-900">
                          {invoice.invoice_number}
                        </span>
                      </Link>
                    </td>
                    <td className="px-4 py-3 text-primary-700">
                      {invoice.vendor_name ?? invoice.vendor?.name ?? '—'}
                    </td>
                    <td className="px-4 py-3 text-primary-500 text-sm">
                      {new Date(invoice.invoice_date).toLocaleDateString()}
                    </td>
                    <td className="px-4 py-3 text-right font-medium text-primary-900">
                      {formatCurrency(Number(invoice.total_amount), typeof invoice.currency === 'string' ? invoice.currency : 'USD')}
                    </td>
                    <td className="px-4 py-3">
                      <div className="flex justify-center">
                        <StatusBadge status={invoice.status ?? mapValidationStatus(invoice.validation_status)} />
                      </div>
                    </td>
                    <td className="px-4 py-3">
                      <div className="flex justify-center">
                        {(invoice.anomalies?.length ?? 0) > 0 ? (
                          <span className="flex items-center gap-1 text-sm text-error">
                            <AlertTriangle className="h-4 w-4" />
                            {invoice.anomalies?.length ?? 0}
                          </span>
                        ) : (
                          <span className="text-sm text-primary-400">—</span>
                        )}
                      </div>
                    </td>
                    <td className="px-4 py-3">
                      <div className="flex justify-end gap-1">
                        <Button variant="ghost" size="sm" asChild>
                          <Link to={`/app/documents/invoices/${invoice.id}`}>
                            View
                          </Link>
                        </Button>
                      </div>
                    </td>
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>

        {/* Pagination */}
        <div className="flex items-center justify-between border-t border-primary-200 px-4 py-3">
          <div className="text-sm text-primary-500">
            Showing {invoices.length === 0 ? 0 : (currentPage - 1) * pageSize + 1} to{' '}
            {Math.min(currentPage * pageSize, totalItems)} of {totalItems} invoices
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
      </Card>

      <UploadDialog
        open={uploadOpen}
        onOpenChange={setUploadOpen}
        onUploadComplete={() => refetch()}
      />
    </div>
  )
}

function mapValidationStatus(vs?: string): InvoiceStatus {
  switch (vs) {
    case 'passed': return 'validated'
    case 'passed_with_warnings': return 'flagged'
    case 'failed': return 'rejected'
    default: return 'pending'
  }
}

function StatusBadge({ status }: { status: InvoiceStatus }) {
  const config: Record<
    InvoiceStatus,
    { icon: React.ReactNode; label: string; className: string }
  > = {
    pending: {
      icon: <Clock className="h-3 w-3" />,
      label: 'Pending',
      className: 'bg-yellow-100 text-yellow-700 border-yellow-200',
    },
    validated: {
      icon: <CheckCircle className="h-3 w-3" />,
      label: 'Validated',
      className: 'bg-blue-100 text-blue-700 border-blue-200',
    },
    flagged: {
      icon: <AlertTriangle className="h-3 w-3" />,
      label: 'Flagged',
      className: 'bg-red-100 text-red-700 border-red-200',
    },
    approved: {
      icon: <CheckCircle className="h-3 w-3" />,
      label: 'Approved',
      className: 'bg-green-100 text-green-700 border-green-200',
    },
    rejected: {
      icon: <XCircle className="h-3 w-3" />,
      label: 'Rejected',
      className: 'bg-red-100 text-red-700 border-red-200',
    },
  }

  const { icon, label, className } = config[status]

  return (
    <span
      className={cn(
        'inline-flex items-center gap-1 rounded-full border px-2 py-0.5 text-xs font-medium',
        className
      )}
    >
      {icon}
      {label}
    </span>
  )
}

function formatCurrency(amount: number, currency: string): string {
  return new Intl.NumberFormat('en-US', {
    style: 'currency',
    currency,
  }).format(amount)
}

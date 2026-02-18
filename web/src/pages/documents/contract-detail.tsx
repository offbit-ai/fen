import { useParams, Link } from 'react-router-dom'
import {
  ArrowLeft,
  FileText,
  Calendar,
  DollarSign,
  Building2,
  Clock,
  CheckCircle,
  AlertTriangle,
  Download,
  Edit,
  ExternalLink,
  Scale,
  Shield,
  FileCheck,
} from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { cn } from '@/lib/utils'
import type { Contract, ContractClause, ContractStatus, ContractType, Party, PricingTerm } from '@/types/api'

// Mock contract data
const mockContract: Contract & {
  parties: Party[]
  clauses: ContractClause[]
  pricing_terms: PricingTerm[]
  linked_invoices: { id: string; invoice_number: string; amount: number; date: string }[]
} = {
  id: 'ctr-001',
  document_id: 'doc-ctr-001',
  tenant_id: 'tenant-1',
  contract_number: 'CTR-2024-0156',
  title: 'Master Service Agreement - IT Services',
  contract_type: 'master_service_agreement',
  effective_date: '2024-01-01',
  expiration_date: '2025-12-31',
  execution_date: '2023-12-15',
  total_value: 500000,
  currency: 'USD',
  validation_status: 'passed',
  confidence_score: 0.96,
  vendor_name: 'TechCorp Solutions',
  status: 'active',
  parties: [
    {
      id: 'party-1',
      name: 'Acme Corporation',
      tax_id: '12-3456789',
      address: {
        street: '100 Main Street',
        city: 'New York',
        state: 'NY',
        postal_code: '10001',
        country: 'USA',
      },
      contact: {
        name: 'John Smith',
        email: 'john.smith@acme.com',
        phone: '+1 (555) 123-4567',
      },
    },
    {
      id: 'party-2',
      name: 'TechCorp Solutions',
      tax_id: '98-7654321',
      address: {
        street: '200 Tech Park Drive',
        city: 'San Francisco',
        state: 'CA',
        postal_code: '94102',
        country: 'USA',
      },
      contact: {
        name: 'Sarah Johnson',
        email: 'sarah.johnson@techcorp.com',
        phone: '+1 (555) 987-6543',
      },
    },
  ],
  clauses: [
    {
      clause_id: 'cl-001',
      clause_type: 'payment_terms',
      title: 'Payment Terms',
      text: 'Payment due within Net 30 days of invoice date. Late payments subject to 1.5% monthly interest.',
    },
    {
      clause_id: 'cl-002',
      clause_type: 'termination',
      title: 'Termination',
      text: 'Either party may terminate with 90 days written notice. Immediate termination for material breach.',
    },
    {
      clause_id: 'cl-003',
      clause_type: 'confidentiality',
      title: 'Confidentiality',
      text: 'All proprietary information shared remains confidential for 5 years after contract termination.',
    },
    {
      clause_id: 'cl-004',
      clause_type: 'liability',
      title: 'Limitation of Liability',
      text: 'Total liability limited to fees paid in the 12 months preceding the claim.',
    },
  ],
  pricing_terms: [
    { item_category: 'Software Development', unit_price: '150.00', min_quantity: 10, discount_percentage: 5 },
    { item_category: 'System Administration', unit_price: '125.00', min_quantity: 20, discount_percentage: 10 },
    { item_category: 'Consulting Services', unit_price: '200.00', min_quantity: 5, discount_percentage: 0 },
    { item_category: 'Support & Maintenance', unit_price: '100.00', min_quantity: 40, discount_percentage: 15 },
  ],
  linked_invoices: [
    { id: 'inv-001', invoice_number: 'INV-2024-0892', amount: 45890, date: '2024-02-01' },
    { id: 'inv-002', invoice_number: 'INV-2024-0756', amount: 32500, date: '2024-01-15' },
    { id: 'inv-003', invoice_number: 'INV-2024-0612', amount: 28750, date: '2024-01-01' },
  ],
  created_at: '2023-12-15T10:00:00Z',
  updated_at: '2024-02-01T14:30:00Z',
}

const statusConfig: Record<ContractStatus, { label: string; className: string; icon: React.ReactNode }> = {
  draft: { label: 'Draft', className: 'bg-gray-100 text-gray-700', icon: <FileText className="h-4 w-4" /> },
  active: { label: 'Active', className: 'bg-green-100 text-green-700', icon: <CheckCircle className="h-4 w-4" /> },
  expiring_soon: { label: 'Expiring Soon', className: 'bg-yellow-100 text-yellow-700', icon: <Clock className="h-4 w-4" /> },
  expired: { label: 'Expired', className: 'bg-red-100 text-red-700', icon: <AlertTriangle className="h-4 w-4" /> },
}

const typeLabels: Record<ContractType, string> = {
  service_agreement: 'Service Agreement',
  purchase_order: 'Purchase Order',
  master_service_agreement: 'Master Service Agreement',
  statement_of_work: 'Statement of Work',
  amendment: 'Amendment',
  other: 'Other',
}

const clauseIcons: Record<string, React.ReactNode> = {
  payment_terms: <DollarSign className="h-4 w-4" />,
  termination: <AlertTriangle className="h-4 w-4" />,
  confidentiality: <Shield className="h-4 w-4" />,
  liability: <Scale className="h-4 w-4" />,
  intellectual_property: <FileCheck className="h-4 w-4" />,
}

export function ContractDetailPage() {
  const { id: _id } = useParams<{ id: string }>()
  const contract = mockContract // In real app, fetch by _id

  const status = statusConfig[contract.status ?? 'draft']
  const daysRemaining = contract.expiration_date
    ? Math.ceil((new Date(contract.expiration_date).getTime() - Date.now()) / (1000 * 60 * 60 * 24))
    : null

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-4">
          <Link to="/app/documents/contracts">
            <Button variant="ghost" size="icon">
              <ArrowLeft className="h-5 w-5" />
            </Button>
          </Link>
          <div>
            <div className="flex items-center gap-3">
              <h1 className="text-2xl font-bold text-primary-900">{contract.contract_number}</h1>
              <span className={cn('inline-flex items-center gap-1 rounded-full px-3 py-1 text-sm font-medium', status.className)}>
                {status.icon}
                {status.label}
              </span>
            </div>
            <p className="text-sm text-primary-500 mt-1">{contract.title}</p>
          </div>
        </div>
        <div className="flex items-center gap-2">
          <Button variant="outline">
            <Download className="mr-2 h-4 w-4" />
            Export
          </Button>
          <Button variant="outline">
            <Edit className="mr-2 h-4 w-4" />
            Edit
          </Button>
          <Button>Renew Contract</Button>
        </div>
      </div>

      <div className="grid gap-6 lg:grid-cols-3">
        {/* Main Content */}
        <div className="lg:col-span-2 space-y-6">
          {/* Contract Overview */}
          <Card>
            <CardHeader>
              <CardTitle>Contract Overview</CardTitle>
            </CardHeader>
            <CardContent>
              <div className="grid gap-6 sm:grid-cols-2">
                <div className="space-y-4">
                  <DetailItem
                    icon={<FileText className="h-4 w-4" />}
                    label="Contract Type"
                    value={typeLabels[contract.contract_type ?? 'other']}
                  />
                  <DetailItem
                    icon={<Calendar className="h-4 w-4" />}
                    label="Effective Date"
                    value={contract.effective_date ? new Date(contract.effective_date).toLocaleDateString() : '—'}
                  />
                  <DetailItem
                    icon={<Calendar className="h-4 w-4" />}
                    label="Expiration Date"
                    value={contract.expiration_date ? new Date(contract.expiration_date).toLocaleDateString() : '—'}
                    highlight={daysRemaining !== null && daysRemaining < 90}
                  />
                </div>
                <div className="space-y-4">
                  <DetailItem
                    icon={<DollarSign className="h-4 w-4" />}
                    label="Total Value"
                    value={formatCurrency(Number(contract.total_value), contract.currency ?? 'USD')}
                    highlight
                  />
                  <DetailItem
                    icon={<Clock className="h-4 w-4" />}
                    label="Days Remaining"
                    value={daysRemaining !== null ? `${daysRemaining} days` : '—'}
                    highlight={daysRemaining !== null && daysRemaining < 90}
                  />
                  <DetailItem
                    icon={<CheckCircle className="h-4 w-4" />}
                    label="Confidence Score"
                    value={`${((contract.confidence_score ?? 0) * 100).toFixed(0)}%`}
                  />
                </div>
              </div>
            </CardContent>
          </Card>

          {/* Parties */}
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <Building2 className="h-5 w-5" />
                Parties
              </CardTitle>
            </CardHeader>
            <CardContent>
              <div className="grid gap-4 sm:grid-cols-2">
                {contract.parties.map((party, index) => (
                  <PartyCard key={party.id} party={party} role={index === 0 ? 'Client' : 'Vendor'} />
                ))}
              </div>
            </CardContent>
          </Card>

          {/* Pricing Terms */}
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <DollarSign className="h-5 w-5" />
                Pricing Terms
              </CardTitle>
            </CardHeader>
            <CardContent>
              <div className="overflow-x-auto">
                <table className="w-full">
                  <thead>
                    <tr className="border-b border-primary-200 bg-primary-50">
                      <th className="px-4 py-3 text-left text-sm font-medium text-primary-700">Category</th>
                      <th className="px-4 py-3 text-right text-sm font-medium text-primary-700">Unit Price</th>
                      <th className="px-4 py-3 text-right text-sm font-medium text-primary-700">Min Qty</th>
                      <th className="px-4 py-3 text-right text-sm font-medium text-primary-700">Discount</th>
                    </tr>
                  </thead>
                  <tbody>
                    {contract.pricing_terms.map((term, i) => (
                      <tr key={i} className="border-b border-primary-100">
                        <td className="px-4 py-3 text-sm text-primary-900">{term.item_category}</td>
                        <td className="px-4 py-3 text-right text-sm text-primary-700">
                          ${parseFloat(term.unit_price).toFixed(2)}
                        </td>
                        <td className="px-4 py-3 text-right text-sm text-primary-700">
                          {term.min_quantity ?? '—'}
                        </td>
                        <td className="px-4 py-3 text-right text-sm text-primary-700">
                          {term.discount_percentage ? `${term.discount_percentage}%` : '—'}
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </CardContent>
          </Card>

          {/* Key Clauses */}
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <Scale className="h-5 w-5" />
                Key Clauses
              </CardTitle>
            </CardHeader>
            <CardContent>
              <div className="space-y-4">
                {contract.clauses.map((clause) => (
                  <ClauseCard key={clause.clause_id} clause={clause} />
                ))}
              </div>
            </CardContent>
          </Card>
        </div>

        {/* Sidebar */}
        <div className="space-y-6">
          {/* Linked Invoices */}
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <FileText className="h-5 w-5" />
                Linked Invoices
              </CardTitle>
            </CardHeader>
            <CardContent>
              <div className="space-y-3">
                {contract.linked_invoices.map((invoice) => (
                  <Link
                    key={invoice.id}
                    to={`/documents/invoices/${invoice.id}`}
                    className="flex items-center justify-between rounded-lg border border-primary-100 p-3 hover:bg-primary-50 transition-colors"
                  >
                    <div>
                      <p className="text-sm font-medium text-primary-900">{invoice.invoice_number}</p>
                      <p className="text-xs text-primary-500">{new Date(invoice.date).toLocaleDateString()}</p>
                    </div>
                    <div className="flex items-center gap-2">
                      <span className="text-sm font-semibold text-primary-900">
                        ${invoice.amount.toLocaleString()}
                      </span>
                      <ExternalLink className="h-4 w-4 text-primary-400" />
                    </div>
                  </Link>
                ))}
              </div>
              <div className="mt-4 pt-4 border-t border-primary-100">
                <div className="flex justify-between text-sm">
                  <span className="text-primary-500">Total Invoiced</span>
                  <span className="font-semibold text-primary-900">
                    ${contract.linked_invoices.reduce((sum, inv) => sum + inv.amount, 0).toLocaleString()}
                  </span>
                </div>
                <div className="flex justify-between text-sm mt-2">
                  <span className="text-primary-500">Remaining Value</span>
                  <span className="font-semibold text-green-700">
                    ${(Number(contract.total_value) - contract.linked_invoices.reduce((sum, inv) => sum + inv.amount, 0)).toLocaleString()}
                  </span>
                </div>
              </div>
            </CardContent>
          </Card>

          {/* Timeline */}
          <Card>
            <CardHeader>
              <CardTitle>Contract Timeline</CardTitle>
            </CardHeader>
            <CardContent>
              <div className="space-y-4">
                <TimelineItem
                  date={contract.execution_date}
                  label="Executed"
                  icon={<FileCheck className="h-4 w-4" />}
                  completed
                />
                <TimelineItem
                  date={contract.effective_date}
                  label="Effective"
                  icon={<CheckCircle className="h-4 w-4" />}
                  completed
                />
                <TimelineItem
                  date={contract.expiration_date}
                  label="Expires"
                  icon={<Clock className="h-4 w-4" />}
                  completed={false}
                  warning={daysRemaining !== null && daysRemaining < 90}
                />
              </div>
            </CardContent>
          </Card>

          {/* Metadata */}
          <Card>
            <CardHeader>
              <CardTitle>Metadata</CardTitle>
            </CardHeader>
            <CardContent>
              <dl className="space-y-3">
                <div>
                  <dt className="text-xs font-medium text-primary-500 uppercase">Document ID</dt>
                  <dd className="text-sm text-primary-900 font-mono">{contract.document_id}</dd>
                </div>
                <div>
                  <dt className="text-xs font-medium text-primary-500 uppercase">Created</dt>
                  <dd className="text-sm text-primary-900">
                    {contract.created_at ? new Date(contract.created_at).toLocaleString() : '—'}
                  </dd>
                </div>
                <div>
                  <dt className="text-xs font-medium text-primary-500 uppercase">Last Updated</dt>
                  <dd className="text-sm text-primary-900">
                    {contract.updated_at ? new Date(contract.updated_at).toLocaleString() : '—'}
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
      <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-primary-100 text-primary-600">
        {icon}
      </div>
      <div>
        <p className="text-xs font-medium text-primary-500 uppercase">{label}</p>
        <p className={cn('text-sm font-medium', highlight ? 'text-primary-900 text-lg' : 'text-primary-700')}>
          {value}
        </p>
      </div>
    </div>
  )
}

function PartyCard({ party, role }: { party: Party; role: string }) {
  return (
    <div className="rounded-lg border border-primary-100 p-4">
      <div className="flex items-center justify-between mb-3">
        <span className="text-xs font-medium text-primary-500 uppercase">{role}</span>
        {party.tax_id && (
          <span className="text-xs text-primary-400">Tax ID: {party.tax_id}</span>
        )}
      </div>
      <p className="font-semibold text-primary-900">{party.name}</p>
      {party.address && (
        <p className="text-sm text-primary-600 mt-2">
          {party.address.street}<br />
          {party.address.city}, {party.address.state} {party.address.postal_code}
        </p>
      )}
      {party.contact && (
        <div className="mt-3 pt-3 border-t border-primary-100">
          <p className="text-sm text-primary-700">{party.contact.name}</p>
          <p className="text-xs text-primary-500">{party.contact.email}</p>
          <p className="text-xs text-primary-500">{party.contact.phone}</p>
        </div>
      )}
    </div>
  )
}

function ClauseCard({ clause }: { clause: ContractClause }) {
  const icon = clauseIcons[clause.clause_type] || <FileText className="h-4 w-4" />

  return (
    <div className="rounded-lg border border-primary-100 p-4">
      <div className="flex items-center gap-2 mb-2">
        <div className="flex h-6 w-6 items-center justify-center rounded bg-primary-100 text-primary-600">
          {icon}
        </div>
        <span className="font-medium text-primary-900">{clause.title}</span>
      </div>
      <p className="text-sm text-primary-600 leading-relaxed">{clause.text}</p>
    </div>
  )
}

function TimelineItem({
  date,
  label,
  icon,
  completed,
  warning,
}: {
  date?: string
  label: string
  icon: React.ReactNode
  completed: boolean
  warning?: boolean
}) {
  return (
    <div className="flex items-center gap-3">
      <div className={cn(
        'flex h-8 w-8 items-center justify-center rounded-full',
        completed ? 'bg-green-100 text-green-600' : warning ? 'bg-yellow-100 text-yellow-600' : 'bg-primary-100 text-primary-400'
      )}>
        {icon}
      </div>
      <div className="flex-1">
        <p className={cn('text-sm font-medium', warning ? 'text-yellow-700' : 'text-primary-900')}>{label}</p>
        <p className="text-xs text-primary-500">
          {date ? new Date(date).toLocaleDateString() : '—'}
        </p>
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

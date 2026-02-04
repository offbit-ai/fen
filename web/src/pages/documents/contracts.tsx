import { useState } from 'react'
import { Link } from 'react-router-dom'
import {
  FileCheck,
  Search,
  Plus,
  Download,
  MoreHorizontal,
  Calendar,
  AlertTriangle,
  CheckCircle,
  Clock,
  ChevronLeft,
  ChevronRight,
} from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Card, CardContent } from '@/components/ui/card'
import { cn } from '@/lib/utils'
import type { Contract, ContractStatus } from '@/types/api'

// Mock data for development
const mockContracts: Contract[] = [
  {
    id: 'ctr-001',
    tenant_id: 'tenant-1',
    vendor_name: 'Acme Corp',
    contract_number: 'CTR-2023-045',
    start_date: '2023-01-01',
    end_date: '2025-12-31',
    total_value: 2500000,
    currency: 'USD',
    status: 'active',
    pricing_terms: [],
    metadata: { payment_terms: 'Net 30' },
    created_at: '2023-01-01T10:00:00Z',
    updated_at: '2024-01-15T14:30:00Z',
  },
  {
    id: 'ctr-002',
    tenant_id: 'tenant-1',
    vendor_name: 'Widget Inc',
    contract_number: 'CTR-2022-012',
    start_date: '2022-06-01',
    end_date: '2024-05-31',
    total_value: 1200000,
    currency: 'USD',
    status: 'expiring_soon',
    pricing_terms: [],
    metadata: { payment_terms: 'Net 45' },
    created_at: '2022-06-01T10:00:00Z',
    updated_at: '2024-01-10T09:00:00Z',
  },
  {
    id: 'ctr-003',
    tenant_id: 'tenant-1',
    vendor_name: 'TechParts Inc',
    contract_number: 'CTR-2024-001',
    start_date: '2024-01-01',
    end_date: '2026-12-31',
    total_value: 500000,
    currency: 'USD',
    status: 'active',
    pricing_terms: [],
    metadata: { payment_terms: 'Net 30' },
    created_at: '2024-01-01T10:00:00Z',
    updated_at: '2024-01-01T10:00:00Z',
  },
  {
    id: 'ctr-004',
    tenant_id: 'tenant-1',
    vendor_name: 'GlobalSupply Co',
    contract_number: 'CTR-2021-089',
    start_date: '2021-03-01',
    end_date: '2023-12-31',
    total_value: 850000,
    currency: 'USD',
    status: 'expired',
    pricing_terms: [],
    metadata: { payment_terms: 'Net 60' },
    created_at: '2021-03-01T10:00:00Z',
    updated_at: '2024-01-05T12:00:00Z',
  },
  {
    id: 'ctr-005',
    tenant_id: 'tenant-1',
    vendor_name: 'Office Supplies Ltd',
    contract_number: 'CTR-2024-002',
    start_date: '2024-02-01',
    end_date: '2024-02-01',
    total_value: 0,
    currency: 'USD',
    status: 'draft',
    pricing_terms: [],
    metadata: {},
    created_at: '2024-02-01T10:00:00Z',
    updated_at: '2024-02-01T10:00:00Z',
  },
]

type TabType = 'all' | 'active' | 'expiring_soon' | 'expired' | 'draft'

const tabs: { id: TabType; label: string; count: number }[] = [
  { id: 'all', label: 'All', count: 156 },
  { id: 'active', label: 'Active', count: 142 },
  { id: 'expiring_soon', label: 'Expiring Soon', count: 12 },
  { id: 'expired', label: 'Expired', count: 8 },
  { id: 'draft', label: 'Draft', count: 2 },
]

export function ContractsPage() {
  const [searchQuery, setSearchQuery] = useState('')
  const [selectedTab, setSelectedTab] = useState<TabType>('all')
  const [selectedContracts, setSelectedContracts] = useState<Set<string>>(new Set())

  const filteredContracts = mockContracts.filter((c) => {
    if (selectedTab !== 'all' && c.status !== selectedTab) return false
    if (searchQuery) {
      const query = searchQuery.toLowerCase()
      return (
        (c.vendor_name ?? '').toLowerCase().includes(query) ||
        (c.contract_number ?? '').toLowerCase().includes(query)
      )
    }
    return true
  })

  const toggleContract = (id: string) => {
    const newSelected = new Set(selectedContracts)
    if (newSelected.has(id)) {
      newSelected.delete(id)
    } else {
      newSelected.add(id)
    }
    setSelectedContracts(newSelected)
  }

  const toggleAll = () => {
    if (selectedContracts.size === filteredContracts.length) {
      setSelectedContracts(new Set())
    } else {
      setSelectedContracts(new Set(filteredContracts.map((c) => c.id)))
    }
  }

  return (
    <div className="space-y-6">
      {/* Page Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-primary-900">Contracts</h1>
          <p className="text-sm text-primary-500">
            Manage vendor contracts and pricing terms
          </p>
        </div>
        <div className="flex items-center gap-2">
          <Button variant="outline">
            <Download className="mr-2 h-4 w-4" />
            Export
          </Button>
          <Button>
            <Plus className="mr-2 h-4 w-4" />
            Add Contract
          </Button>
        </div>
      </div>

      {/* Tabs */}
      <div className="flex gap-1 border-b border-primary-200">
        {tabs.map((tab) => (
          <button
            key={tab.id}
            onClick={() => setSelectedTab(tab.id)}
            className={cn(
              'flex items-center gap-2 px-4 py-2 text-sm font-medium border-b-2 -mb-px transition-colors',
              selectedTab === tab.id
                ? 'border-primary-900 text-primary-900'
                : 'border-transparent text-primary-500 hover:text-primary-700'
            )}
          >
            {tab.label}
            <span
              className={cn(
                'rounded-full px-2 py-0.5 text-xs',
                selectedTab === tab.id
                  ? 'bg-primary-900 text-white'
                  : 'bg-primary-100 text-primary-600'
              )}
            >
              {tab.count}
            </span>
          </button>
        ))}
      </div>

      {/* Search & Filters */}
      <div className="flex items-center gap-4">
        <div className="relative flex-1 max-w-md">
          <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-primary-400" />
          <Input
            type="search"
            placeholder="Search contracts..."
            className="pl-10"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
          />
        </div>
      </div>

      {/* Bulk Actions */}
      {selectedContracts.size > 0 && (
        <Card className="bg-primary-50">
          <CardContent className="py-3 px-4 flex items-center justify-between">
            <span className="text-sm text-primary-700">
              {selectedContracts.size} contract{selectedContracts.size > 1 ? 's' : ''} selected
            </span>
            <div className="flex items-center gap-2">
              <Button variant="outline" size="sm">
                <Download className="mr-2 h-4 w-4" />
                Export
              </Button>
            </div>
          </CardContent>
        </Card>
      )}

      {/* Contracts Table */}
      <Card>
        <CardContent className="p-0">
          <div className="overflow-x-auto">
            <table className="w-full">
              <thead>
                <tr className="border-b border-primary-200 bg-primary-50">
                  <th className="w-12 px-4 py-3">
                    <input
                      type="checkbox"
                      checked={selectedContracts.size === filteredContracts.length && filteredContracts.length > 0}
                      onChange={toggleAll}
                      className="h-4 w-4 rounded border-primary-300"
                    />
                  </th>
                  <th className="px-4 py-3 text-left text-sm font-medium text-primary-700">
                    Status
                  </th>
                  <th className="px-4 py-3 text-left text-sm font-medium text-primary-700">
                    Contract #
                  </th>
                  <th className="px-4 py-3 text-left text-sm font-medium text-primary-700">
                    Vendor
                  </th>
                  <th className="px-4 py-3 text-left text-sm font-medium text-primary-700">
                    Start Date
                  </th>
                  <th className="px-4 py-3 text-left text-sm font-medium text-primary-700">
                    End Date
                  </th>
                  <th className="px-4 py-3 text-right text-sm font-medium text-primary-700">
                    Value
                  </th>
                  <th className="w-12 px-4 py-3"></th>
                </tr>
              </thead>
              <tbody>
                {filteredContracts.map((contract) => (
                  <ContractRow
                    key={contract.id}
                    contract={contract}
                    selected={selectedContracts.has(contract.id)}
                    onToggle={() => toggleContract(contract.id)}
                  />
                ))}
              </tbody>
            </table>
          </div>
        </CardContent>
      </Card>

      {/* Pagination */}
      <div className="flex items-center justify-between">
        <p className="text-sm text-primary-500">
          Showing 1-{filteredContracts.length} of {tabs[0].count} contracts
        </p>
        <div className="flex items-center gap-2">
          <Button variant="outline" size="sm" disabled>
            <ChevronLeft className="h-4 w-4" />
          </Button>
          <Button variant="outline" size="sm" className="bg-primary-100">
            1
          </Button>
          <Button variant="outline" size="sm">
            2
          </Button>
          <Button variant="outline" size="sm">
            3
          </Button>
          <Button variant="outline" size="sm">
            <ChevronRight className="h-4 w-4" />
          </Button>
        </div>
      </div>
    </div>
  )
}

interface ContractRowProps {
  contract: Contract
  selected: boolean
  onToggle: () => void
}

function ContractRow({ contract, selected, onToggle }: ContractRowProps) {
  const statusConfig: Record<ContractStatus, { icon: React.ReactNode; className: string; label: string }> = {
    active: {
      icon: <CheckCircle className="h-4 w-4" />,
      className: 'bg-green-100 text-green-700',
      label: 'Active',
    },
    expiring_soon: {
      icon: <AlertTriangle className="h-4 w-4" />,
      className: 'bg-yellow-100 text-yellow-700',
      label: 'Expiring',
    },
    expired: {
      icon: <Clock className="h-4 w-4" />,
      className: 'bg-red-100 text-red-700',
      label: 'Expired',
    },
    draft: {
      icon: <FileCheck className="h-4 w-4" />,
      className: 'bg-primary-100 text-primary-700',
      label: 'Draft',
    },
  }

  const status = statusConfig[contract.status ?? 'draft']
  const daysUntilExpiry = contract.end_date
    ? Math.ceil((new Date(contract.end_date).getTime() - Date.now()) / (1000 * 60 * 60 * 24))
    : 0

  return (
    <tr className={cn('border-b border-primary-100 hover:bg-primary-50', selected && 'bg-primary-50')}>
      <td className="px-4 py-3">
        <input
          type="checkbox"
          checked={selected}
          onChange={onToggle}
          className="h-4 w-4 rounded border-primary-300"
        />
      </td>
      <td className="px-4 py-3">
        <span
          className={cn(
            'inline-flex items-center gap-1 rounded-full px-2.5 py-0.5 text-xs font-medium',
            status.className
          )}
        >
          {status.icon}
          {status.label}
        </span>
      </td>
      <td className="px-4 py-3">
        <Link
          to={`/documents/contracts/${contract.id}`}
          className="font-medium text-primary-900 hover:text-primary-700 hover:underline"
        >
          {contract.contract_number}
        </Link>
      </td>
      <td className="px-4 py-3">
        <span className="text-primary-700">{contract.vendor_name}</span>
      </td>
      <td className="px-4 py-3">
        <div className="flex items-center gap-2 text-sm text-primary-600">
          <Calendar className="h-4 w-4" />
          {contract.start_date ? new Date(contract.start_date).toLocaleDateString() : '—'}
        </div>
      </td>
      <td className="px-4 py-3">
        <div className="flex flex-col">
          <div className="flex items-center gap-2 text-sm text-primary-600">
            <Calendar className="h-4 w-4" />
            {contract.end_date ? new Date(contract.end_date).toLocaleDateString() : '—'}
          </div>
          {contract.status === 'expiring_soon' && (
            <span className="text-xs text-yellow-600">
              {daysUntilExpiry} days left
            </span>
          )}
        </div>
      </td>
      <td className="px-4 py-3 text-right">
        <span className="font-medium text-primary-900">
          {new Intl.NumberFormat('en-US', {
            style: 'currency',
            currency: contract.currency ?? 'USD',
            maximumFractionDigits: 0,
          }).format(Number(contract.total_value ?? 0))}
        </span>
      </td>
      <td className="px-4 py-3">
        <Button variant="ghost" size="icon" className="h-8 w-8">
          <MoreHorizontal className="h-4 w-4" />
        </Button>
      </td>
    </tr>
  )
}

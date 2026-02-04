import { useState } from 'react'
import { Link } from 'react-router-dom'
import {
  Paperclip,
  Search,
  Upload,
  Download,
  FileText,
  Image,
  File,
  FileSpreadsheet,
  Trash2,
  Eye,
  MoreHorizontal,
  ChevronLeft,
  ChevronRight,
  Calendar,
  HardDrive,
  FolderOpen,
} from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Card, CardContent } from '@/components/ui/card'
import { cn } from '@/lib/utils'

type AttachmentType = 'pdf' | 'image' | 'spreadsheet' | 'document' | 'other'
type AttachmentSource = 'invoice' | 'contract' | 'manual'

interface Attachment {
  id: string
  filename: string
  file_type: AttachmentType
  mime_type: string
  size_bytes: number
  source: AttachmentSource
  source_id?: string
  source_name?: string
  uploaded_by: string
  uploaded_at: string
  tags?: string[]
}

const mockAttachments: Attachment[] = [
  {
    id: 'att-001',
    filename: 'invoice_INV-2024-0892.pdf',
    file_type: 'pdf',
    mime_type: 'application/pdf',
    size_bytes: 245760,
    source: 'invoice',
    source_id: 'inv-001',
    source_name: 'INV-2024-0892',
    uploaded_by: 'Sarah Anderson',
    uploaded_at: '2024-02-04T10:30:00Z',
    tags: ['original', 'processed'],
  },
  {
    id: 'att-002',
    filename: 'contract_CTR-2024-0156_signed.pdf',
    file_type: 'pdf',
    mime_type: 'application/pdf',
    size_bytes: 1048576,
    source: 'contract',
    source_id: 'ctr-001',
    source_name: 'CTR-2024-0156',
    uploaded_by: 'John Smith',
    uploaded_at: '2024-02-03T14:00:00Z',
    tags: ['signed', 'legal'],
  },
  {
    id: 'att-003',
    filename: 'vendor_pricing_sheet.xlsx',
    file_type: 'spreadsheet',
    mime_type: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
    size_bytes: 524288,
    source: 'manual',
    uploaded_by: 'Emily Chen',
    uploaded_at: '2024-02-02T09:15:00Z',
    tags: ['pricing', 'reference'],
  },
  {
    id: 'att-004',
    filename: 'delivery_receipt_scan.jpg',
    file_type: 'image',
    mime_type: 'image/jpeg',
    size_bytes: 2097152,
    source: 'invoice',
    source_id: 'inv-002',
    source_name: 'INV-2024-0756',
    uploaded_by: 'Sarah Anderson',
    uploaded_at: '2024-02-01T16:45:00Z',
    tags: ['proof', 'delivery'],
  },
  {
    id: 'att-005',
    filename: 'purchase_order_amendment.docx',
    file_type: 'document',
    mime_type: 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
    size_bytes: 102400,
    source: 'contract',
    source_id: 'ctr-002',
    source_name: 'CTR-2024-0098',
    uploaded_by: 'John Smith',
    uploaded_at: '2024-01-30T11:00:00Z',
    tags: ['amendment'],
  },
  {
    id: 'att-006',
    filename: 'tax_certificate_2024.pdf',
    file_type: 'pdf',
    mime_type: 'application/pdf',
    size_bytes: 358400,
    source: 'manual',
    uploaded_by: 'Emily Chen',
    uploaded_at: '2024-01-28T10:00:00Z',
    tags: ['tax', 'compliance'],
  },
]

const fileTypeConfig: Record<AttachmentType, { icon: React.ReactNode; className: string }> = {
  pdf: { icon: <FileText className="h-5 w-5" />, className: 'bg-red-100 text-red-600' },
  image: { icon: <Image className="h-5 w-5" />, className: 'bg-purple-100 text-purple-600' },
  spreadsheet: { icon: <FileSpreadsheet className="h-5 w-5" />, className: 'bg-green-100 text-green-600' },
  document: { icon: <FileText className="h-5 w-5" />, className: 'bg-blue-100 text-blue-600' },
  other: { icon: <File className="h-5 w-5" />, className: 'bg-gray-100 text-gray-600' },
}

const sourceConfig: Record<AttachmentSource, { label: string; path: string }> = {
  invoice: { label: 'Invoice', path: '/documents/invoices' },
  contract: { label: 'Contract', path: '/documents/contracts' },
  manual: { label: 'Manual Upload', path: '' },
}

export function AttachmentsPage() {
  const [searchQuery, setSearchQuery] = useState('')
  const [typeFilter, setTypeFilter] = useState<AttachmentType | 'all'>('all')
  const [sourceFilter, setSourceFilter] = useState<AttachmentSource | 'all'>('all')
  const [selectedAttachments, setSelectedAttachments] = useState<Set<string>>(new Set())

  const filteredAttachments = mockAttachments.filter((a) => {
    if (typeFilter !== 'all' && a.file_type !== typeFilter) return false
    if (sourceFilter !== 'all' && a.source !== sourceFilter) return false
    if (searchQuery) {
      const query = searchQuery.toLowerCase()
      return (
        a.filename.toLowerCase().includes(query) ||
        a.tags?.some((t) => t.toLowerCase().includes(query))
      )
    }
    return true
  })

  const toggleAttachment = (id: string) => {
    const newSelected = new Set(selectedAttachments)
    if (newSelected.has(id)) {
      newSelected.delete(id)
    } else {
      newSelected.add(id)
    }
    setSelectedAttachments(newSelected)
  }

  const toggleAll = () => {
    if (selectedAttachments.size === filteredAttachments.length) {
      setSelectedAttachments(new Set())
    } else {
      setSelectedAttachments(new Set(filteredAttachments.map((a) => a.id)))
    }
  }

  const formatFileSize = (bytes: number) => {
    if (bytes >= 1048576) return `${(bytes / 1048576).toFixed(1)} MB`
    if (bytes >= 1024) return `${(bytes / 1024).toFixed(0)} KB`
    return `${bytes} B`
  }

  // Stats
  const totalSize = mockAttachments.reduce((sum, a) => sum + a.size_bytes, 0)
  const typeBreakdown = mockAttachments.reduce((acc, a) => {
    acc[a.file_type] = (acc[a.file_type] || 0) + 1
    return acc
  }, {} as Record<string, number>)

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-primary-900">Attachments</h1>
          <p className="text-sm text-primary-500">
            Manage document attachments and supporting files
          </p>
        </div>
        <Button>
          <Upload className="mr-2 h-4 w-4" />
          Upload Files
        </Button>
      </div>

      {/* Stats Cards */}
      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
        <Card>
          <CardContent className="p-4">
            <div className="flex items-center gap-3">
              <div className="rounded-lg bg-primary-100 p-2">
                <Paperclip className="h-5 w-5 text-primary-600" />
              </div>
              <div>
                <p className="text-sm text-primary-500">Total Files</p>
                <p className="text-2xl font-bold text-primary-900">{mockAttachments.length}</p>
              </div>
            </div>
          </CardContent>
        </Card>
        <Card>
          <CardContent className="p-4">
            <div className="flex items-center gap-3">
              <div className="rounded-lg bg-blue-100 p-2">
                <HardDrive className="h-5 w-5 text-blue-600" />
              </div>
              <div>
                <p className="text-sm text-primary-500">Total Size</p>
                <p className="text-2xl font-bold text-primary-900">{formatFileSize(totalSize)}</p>
              </div>
            </div>
          </CardContent>
        </Card>
        <Card>
          <CardContent className="p-4">
            <div className="flex items-center gap-3">
              <div className="rounded-lg bg-red-100 p-2">
                <FileText className="h-5 w-5 text-red-600" />
              </div>
              <div>
                <p className="text-sm text-primary-500">PDFs</p>
                <p className="text-2xl font-bold text-primary-900">{typeBreakdown.pdf || 0}</p>
              </div>
            </div>
          </CardContent>
        </Card>
        <Card>
          <CardContent className="p-4">
            <div className="flex items-center gap-3">
              <div className="rounded-lg bg-green-100 p-2">
                <Calendar className="h-5 w-5 text-green-600" />
              </div>
              <div>
                <p className="text-sm text-primary-500">This Week</p>
                <p className="text-2xl font-bold text-primary-900">4</p>
              </div>
            </div>
          </CardContent>
        </Card>
      </div>

      {/* Filters */}
      <Card className="p-4">
        <div className="flex flex-col gap-4 sm:flex-row sm:items-center">
          <div className="relative flex-1 max-w-md">
            <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-primary-400" />
            <Input
              type="search"
              placeholder="Search files..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="pl-10"
            />
          </div>
          <div className="flex items-center gap-2">
            <select
              value={typeFilter}
              onChange={(e) => setTypeFilter(e.target.value as AttachmentType | 'all')}
              className="h-10 rounded-lg border border-primary-200 bg-white px-3 text-sm"
            >
              <option value="all">All Types</option>
              <option value="pdf">PDF</option>
              <option value="image">Images</option>
              <option value="spreadsheet">Spreadsheets</option>
              <option value="document">Documents</option>
              <option value="other">Other</option>
            </select>
            <select
              value={sourceFilter}
              onChange={(e) => setSourceFilter(e.target.value as AttachmentSource | 'all')}
              className="h-10 rounded-lg border border-primary-200 bg-white px-3 text-sm"
            >
              <option value="all">All Sources</option>
              <option value="invoice">Invoice</option>
              <option value="contract">Contract</option>
              <option value="manual">Manual Upload</option>
            </select>
          </div>
        </div>
      </Card>

      {/* Bulk Actions */}
      {selectedAttachments.size > 0 && (
        <Card className="bg-primary-50">
          <CardContent className="py-3 px-4 flex items-center justify-between">
            <span className="text-sm text-primary-700">
              {selectedAttachments.size} file{selectedAttachments.size > 1 ? 's' : ''} selected
            </span>
            <div className="flex items-center gap-2">
              <Button variant="outline" size="sm">
                <Download className="mr-2 h-4 w-4" />
                Download
              </Button>
              <Button variant="outline" size="sm" className="text-red-600 border-red-200 hover:bg-red-50">
                <Trash2 className="mr-2 h-4 w-4" />
                Delete
              </Button>
            </div>
          </CardContent>
        </Card>
      )}

      {/* Attachments Table */}
      <Card>
        <CardContent className="p-0">
          <div className="overflow-x-auto">
            <table className="w-full">
              <thead>
                <tr className="border-b border-primary-200 bg-primary-50">
                  <th className="w-12 px-4 py-3">
                    <input
                      type="checkbox"
                      checked={selectedAttachments.size === filteredAttachments.length && filteredAttachments.length > 0}
                      onChange={toggleAll}
                      className="h-4 w-4 rounded border-primary-300"
                    />
                  </th>
                  <th className="px-4 py-3 text-left text-sm font-medium text-primary-700">File</th>
                  <th className="px-4 py-3 text-left text-sm font-medium text-primary-700">Source</th>
                  <th className="px-4 py-3 text-left text-sm font-medium text-primary-700">Size</th>
                  <th className="px-4 py-3 text-left text-sm font-medium text-primary-700">Uploaded</th>
                  <th className="px-4 py-3 text-left text-sm font-medium text-primary-700">Tags</th>
                  <th className="w-12 px-4 py-3"></th>
                </tr>
              </thead>
              <tbody>
                {filteredAttachments.map((attachment) => (
                  <AttachmentRow
                    key={attachment.id}
                    attachment={attachment}
                    selected={selectedAttachments.has(attachment.id)}
                    onToggle={() => toggleAttachment(attachment.id)}
                    formatFileSize={formatFileSize}
                  />
                ))}
              </tbody>
            </table>
          </div>
        </CardContent>
      </Card>

      {filteredAttachments.length === 0 && (
        <Card className="p-8 text-center">
          <FolderOpen className="mx-auto h-8 w-8 text-primary-300" />
          <p className="mt-2 text-primary-500">No attachments found matching your criteria.</p>
        </Card>
      )}

      {/* Pagination */}
      <div className="flex items-center justify-between">
        <p className="text-sm text-primary-500">
          Showing 1-{filteredAttachments.length} of {mockAttachments.length} files
        </p>
        <div className="flex items-center gap-2">
          <Button variant="outline" size="sm" disabled>
            <ChevronLeft className="h-4 w-4" />
          </Button>
          <Button variant="outline" size="sm" className="bg-primary-100">
            1
          </Button>
          <Button variant="outline" size="sm" disabled>
            <ChevronRight className="h-4 w-4" />
          </Button>
        </div>
      </div>
    </div>
  )
}

function AttachmentRow({
  attachment,
  selected,
  onToggle,
  formatFileSize,
}: {
  attachment: Attachment
  selected: boolean
  onToggle: () => void
  formatFileSize: (bytes: number) => string
}) {
  const fileType = fileTypeConfig[attachment.file_type]
  const source = sourceConfig[attachment.source]

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
        <div className="flex items-center gap-3">
          <div className={cn('rounded-lg p-2', fileType.className)}>
            {fileType.icon}
          </div>
          <div>
            <p className="font-medium text-primary-900">{attachment.filename}</p>
            <p className="text-xs text-primary-500">{attachment.mime_type}</p>
          </div>
        </div>
      </td>
      <td className="px-4 py-3">
        {attachment.source_id ? (
          <Link
            to={`${source.path}/${attachment.source_id}`}
            className="text-sm text-primary-600 hover:text-primary-800 hover:underline"
          >
            {source.label}: {attachment.source_name}
          </Link>
        ) : (
          <span className="text-sm text-primary-500">{source.label}</span>
        )}
      </td>
      <td className="px-4 py-3 text-sm text-primary-600">
        {formatFileSize(attachment.size_bytes)}
      </td>
      <td className="px-4 py-3">
        <div>
          <p className="text-sm text-primary-700">
            {new Date(attachment.uploaded_at).toLocaleDateString()}
          </p>
          <p className="text-xs text-primary-500">{attachment.uploaded_by}</p>
        </div>
      </td>
      <td className="px-4 py-3">
        <div className="flex flex-wrap gap-1">
          {attachment.tags?.map((tag) => (
            <span
              key={tag}
              className="rounded-full bg-primary-100 px-2 py-0.5 text-xs font-medium text-primary-700"
            >
              {tag}
            </span>
          ))}
        </div>
      </td>
      <td className="px-4 py-3">
        <div className="flex items-center gap-1">
          <Button variant="ghost" size="icon" className="h-8 w-8">
            <Eye className="h-4 w-4" />
          </Button>
          <Button variant="ghost" size="icon" className="h-8 w-8">
            <Download className="h-4 w-4" />
          </Button>
          <Button variant="ghost" size="icon" className="h-8 w-8">
            <MoreHorizontal className="h-4 w-4" />
          </Button>
        </div>
      </td>
    </tr>
  )
}

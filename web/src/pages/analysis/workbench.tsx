import { useState } from 'react'
import {
  Play,
  Save,
  History,
  Download,
  Copy,
  Table,
  BarChart3,
  ChevronDown,
  ChevronRight,
  Database,
  Clock,
  Trash2,
} from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { cn } from '@/lib/utils'

// Database schema with types (matching backend fen-core domain)
const schema = [
  {
    name: 'invoices',
    columns: [
      { name: 'id', type: 'uuid', key: true },
      { name: 'document_id', type: 'uuid' },
      { name: 'tenant_id', type: 'uuid' },
      { name: 'invoice_number', type: 'text' },
      { name: 'invoice_date', type: 'date' },
      { name: 'due_date', type: 'date?' },
      { name: 'po_number', type: 'text?' },
      { name: 'contract_id', type: 'uuid?' },
      { name: 'vendor_name', type: 'text' },
      { name: 'currency', type: 'enum' },
      { name: 'subtotal', type: 'decimal' },
      { name: 'tax_amount', type: 'decimal' },
      { name: 'total_amount', type: 'decimal' },
      { name: 'validation_status', type: 'enum' },
      { name: 'confidence_score', type: 'float' },
    ],
  },
  {
    name: 'line_items',
    columns: [
      { name: 'invoice_id', type: 'uuid', fk: 'invoices.id' },
      { name: 'line_number', type: 'integer' },
      { name: 'description', type: 'text' },
      { name: 'quantity', type: 'decimal' },
      { name: 'unit_price', type: 'decimal' },
      { name: 'total', type: 'decimal' },
      { name: 'item_code', type: 'text?' },
    ],
  },
  {
    name: 'anomalies',
    columns: [
      { name: 'document_id', type: 'uuid', fk: 'invoices.id' },
      { name: 'anomaly_type', type: 'enum' },
      { name: 'severity', type: 'enum' },
      { name: 'confidence', type: 'float' },
      { name: 'description', type: 'text' },
      { name: 'field_path', type: 'text?' },
      { name: 'expected_value', type: 'text?' },
      { name: 'actual_value', type: 'text?' },
      { name: 'detected_at', type: 'timestamp' },
    ],
  },
  {
    name: 'contracts',
    columns: [
      { name: 'id', type: 'uuid', key: true },
      { name: 'document_id', type: 'uuid' },
      { name: 'tenant_id', type: 'uuid' },
      { name: 'contract_number', type: 'text?' },
      { name: 'title', type: 'text' },
      { name: 'contract_type', type: 'enum' },
      { name: 'effective_date', type: 'date' },
      { name: 'expiration_date', type: 'date?' },
      { name: 'total_value', type: 'decimal?' },
      { name: 'validation_status', type: 'enum' },
      { name: 'confidence_score', type: 'float' },
    ],
  },
  {
    name: 'baselines',
    columns: [
      { name: 'id', type: 'uuid', key: true },
      { name: 'vendor_name', type: 'text' },
      { name: 'metric_name', type: 'text' },
      { name: 'period', type: 'enum' },
      { name: 'mean', type: 'float' },
      { name: 'stddev', type: 'float' },
      { name: 'sample_count', type: 'integer' },
      { name: 'computed_at', type: 'timestamp' },
    ],
  },
]

// Sample queries for quick access
const sampleQueries = [
  {
    name: 'Anomalies by Vendor',
    description: 'Count anomalies per vendor with average amount',
    sql: `SELECT
  vendor_name,
  COUNT(*) as anomaly_count,
  AVG(total_amount) as avg_amount
FROM invoices
JOIN anomalies ON invoices.id = anomalies.document_id
GROUP BY vendor_name
ORDER BY anomaly_count DESC
LIMIT 10;`,
  },
  {
    name: 'High Value Invoices',
    description: 'Invoices over $10,000',
    sql: `SELECT
  invoice_number,
  vendor_name,
  total_amount,
  status
FROM invoices
WHERE total_amount > 10000
ORDER BY total_amount DESC
LIMIT 20;`,
  },
  {
    name: 'Monthly Summary',
    description: 'Invoice count and value by month',
    sql: `SELECT
  DATE_TRUNC('month', invoice_date) as month,
  COUNT(*) as invoice_count,
  SUM(total_amount) as total_value
FROM invoices
GROUP BY month
ORDER BY month DESC;`,
  },
  {
    name: 'Price Deviation Check',
    description: 'Find invoices with prices above baseline',
    sql: `SELECT
  i.invoice_number,
  i.vendor_name,
  i.total_amount,
  b.mean as baseline_avg,
  BASELINE_DEVIATION(i.total_amount, i.vendor_name, 'price') as deviation
FROM invoices i
JOIN baselines b ON i.vendor_name = b.vendor_name
WHERE deviation > 2.0
ORDER BY deviation DESC;`,
  },
]

// Mock query results
const mockResults = {
  columns: ['vendor_name', 'anomaly_count', 'avg_amount'],
  rows: [
    ['TechParts Inc', 23, 34500],
    ['GlobalSupply Co', 18, 28200],
    ['Acme Corp', 15, 22100],
    ['MegaLogistics', 12, 41800],
    ['Office Supplies Ltd', 8, 4500],
  ],
  row_count: 5,
  execution_time_ms: 23,
}

interface QueryHistoryItem {
  id: string
  query: string
  executedAt: Date
  rowCount: number
  executionTime: number
}

export function QueryWorkbenchPage() {
  const [query, setQuery] = useState(sampleQueries[0].sql)
  const [results, setResults] = useState<typeof mockResults | null>(null)
  const [isRunning, setIsRunning] = useState(false)
  const [viewMode, setViewMode] = useState<'table' | 'chart'>('table')
  const [showHistory, setShowHistory] = useState(false)
  const [queryHistory, setQueryHistory] = useState<QueryHistoryItem[]>([
    {
      id: '1',
      query: 'SELECT * FROM invoices LIMIT 10',
      executedAt: new Date(Date.now() - 1000 * 60 * 5),
      rowCount: 10,
      executionTime: 12,
    },
    {
      id: '2',
      query: 'SELECT vendor_name, COUNT(*) FROM anomalies GROUP BY vendor_name',
      executedAt: new Date(Date.now() - 1000 * 60 * 30),
      rowCount: 8,
      executionTime: 45,
    },
  ])

  const runQuery = async () => {
    setIsRunning(true)
    // Simulate API call
    await new Promise((resolve) => setTimeout(resolve, 500))
    setResults(mockResults)
    setIsRunning(false)

    // Add to history
    const historyItem: QueryHistoryItem = {
      id: Date.now().toString(),
      query: query.trim(),
      executedAt: new Date(),
      rowCount: mockResults.row_count,
      executionTime: mockResults.execution_time_ms,
    }
    setQueryHistory((prev) => [historyItem, ...prev.slice(0, 19)])
  }

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if ((e.metaKey || e.ctrlKey) && e.key === 'Enter') {
      e.preventDefault()
      runQuery()
    }
  }

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-primary-900">Query Workbench</h1>
          <p className="text-sm text-primary-500">
            Write SQL-like queries to analyze your documents
          </p>
        </div>
        <div className="flex items-center gap-2">
          <Button
            variant="outline"
            size="sm"
            onClick={() => setShowHistory(!showHistory)}
            className={cn(showHistory && 'bg-primary-100')}
          >
            <History className="mr-2 h-4 w-4" />
            History
          </Button>
          <Button variant="outline" size="sm">
            <Save className="mr-2 h-4 w-4" />
            Save Query
          </Button>
        </div>
      </div>

      <div className="grid gap-6 lg:grid-cols-4">
        {/* Query Panel */}
        <div className={cn('space-y-4', showHistory ? 'lg:col-span-2' : 'lg:col-span-3')}>
          {/* Editor */}
          <Card>
            <CardHeader className="flex flex-row items-center justify-between py-3">
              <CardTitle className="text-sm font-medium">SQL Editor</CardTitle>
              <div className="flex items-center gap-2">
                <kbd className="hidden sm:inline-flex items-center gap-1 rounded bg-primary-100 px-2 py-1 text-xs text-primary-600">
                  <span className="text-[10px]">⌘</span>Enter to run
                </kbd>
                <Button size="sm" onClick={runQuery} disabled={isRunning}>
                  <Play className="mr-2 h-4 w-4" />
                  {isRunning ? 'Running...' : 'Run Query'}
                </Button>
              </div>
            </CardHeader>
            <CardContent className="p-0">
              <div className="relative">
                <textarea
                  value={query}
                  onChange={(e) => setQuery(e.target.value)}
                  onKeyDown={handleKeyDown}
                  className="w-full h-64 p-4 font-mono text-sm bg-primary-950 text-primary-100 rounded-b-lg resize-none focus:outline-none focus:ring-2 focus:ring-primary-500 focus:ring-inset"
                  placeholder="SELECT * FROM invoices..."
                  spellCheck={false}
                />
              </div>
            </CardContent>
          </Card>

          {/* Results */}
          {results && (
            <Card>
              <CardHeader className="flex flex-row items-center justify-between py-3">
                <div className="flex items-center gap-4">
                  <CardTitle className="text-sm font-medium">
                    Results ({results.row_count} rows, {results.execution_time_ms}ms)
                  </CardTitle>
                  <div className="flex items-center rounded-lg border border-primary-200 p-1">
                    <button
                      onClick={() => setViewMode('table')}
                      className={cn(
                        'flex items-center gap-1 rounded px-2 py-1 text-xs transition-colors',
                        viewMode === 'table'
                          ? 'bg-primary-100 text-primary-900'
                          : 'text-primary-500 hover:text-primary-700'
                      )}
                    >
                      <Table className="h-3 w-3" />
                      Table
                    </button>
                    <button
                      onClick={() => setViewMode('chart')}
                      className={cn(
                        'flex items-center gap-1 rounded px-2 py-1 text-xs transition-colors',
                        viewMode === 'chart'
                          ? 'bg-primary-100 text-primary-900'
                          : 'text-primary-500 hover:text-primary-700'
                      )}
                    >
                      <BarChart3 className="h-3 w-3" />
                      Chart
                    </button>
                  </div>
                </div>
                <div className="flex items-center gap-2">
                  <Button variant="ghost" size="sm">
                    <Copy className="mr-2 h-4 w-4" />
                    Copy
                  </Button>
                  <Button variant="ghost" size="sm">
                    <Download className="mr-2 h-4 w-4" />
                    Export
                  </Button>
                </div>
              </CardHeader>
              <CardContent className="p-0">
                {viewMode === 'table' ? (
                  <div className="overflow-x-auto">
                    <table className="w-full">
                      <thead>
                        <tr className="border-b border-primary-200 bg-primary-50">
                          {results.columns.map((col) => (
                            <th
                              key={col}
                              className="px-4 py-2 text-left text-xs font-medium text-primary-700"
                            >
                              {col}
                            </th>
                          ))}
                        </tr>
                      </thead>
                      <tbody>
                        {results.rows.map((row, i) => (
                          <tr
                            key={i}
                            className="border-b border-primary-100 hover:bg-primary-50"
                          >
                            {row.map((cell, j) => (
                              <td key={j} className="px-4 py-2 text-sm text-primary-700">
                                {typeof cell === 'number'
                                  ? j === 2
                                    ? `$${cell.toLocaleString()}`
                                    : cell.toLocaleString()
                                  : cell}
                              </td>
                            ))}
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  </div>
                ) : (
                  <div className="p-6">
                    <div className="h-64 flex items-end gap-2">
                      {results.rows.map((row, i) => (
                        <div key={i} className="flex-1 flex flex-col items-center gap-2">
                          <div
                            className="w-full bg-primary-500 rounded-t transition-all hover:bg-primary-600"
                            style={{
                              height: `${(Number(row[1]) / 25) * 100}%`,
                            }}
                          />
                          <span className="text-xs text-primary-500 truncate max-w-full">
                            {String(row[0]).split(' ')[0]}
                          </span>
                        </div>
                      ))}
                    </div>
                  </div>
                )}
              </CardContent>
            </Card>
          )}
        </div>

        {/* History Panel (conditional) */}
        {showHistory && (
          <div className="space-y-4">
            <Card>
              <CardHeader className="flex flex-row items-center justify-between py-3">
                <CardTitle className="text-sm font-medium flex items-center gap-2">
                  <Clock className="h-4 w-4" />
                  Query History
                </CardTitle>
                {queryHistory.length > 0 && (
                  <Button
                    variant="ghost"
                    size="sm"
                    className="h-7 px-2 text-primary-500"
                    onClick={() => setQueryHistory([])}
                  >
                    <Trash2 className="h-3 w-3" />
                  </Button>
                )}
              </CardHeader>
              <CardContent className="space-y-2 max-h-[500px] overflow-y-auto">
                {queryHistory.length === 0 ? (
                  <p className="text-sm text-primary-400 text-center py-4">
                    No queries yet
                  </p>
                ) : (
                  queryHistory.map((item) => (
                    <button
                      key={item.id}
                      onClick={() => setQuery(item.query)}
                      className="w-full text-left p-3 rounded-lg border border-primary-100 hover:bg-primary-50 transition-colors"
                    >
                      <p className="text-xs text-primary-400 mb-1">
                        {formatRelativeTime(item.executedAt)} · {item.rowCount} rows · {item.executionTime}ms
                      </p>
                      <p className="text-xs font-mono text-primary-700 line-clamp-2">
                        {item.query}
                      </p>
                    </button>
                  ))
                )}
              </CardContent>
            </Card>
          </div>
        )}

        {/* Sidebar */}
        <div className="space-y-4">
          {/* Sample Queries */}
          <Card>
            <CardHeader className="py-3">
              <CardTitle className="text-sm font-medium">Sample Queries</CardTitle>
            </CardHeader>
            <CardContent className="space-y-1">
              {sampleQueries.map((sq, i) => (
                <button
                  key={i}
                  onClick={() => setQuery(sq.sql)}
                  className="w-full text-left px-3 py-2 rounded-lg hover:bg-primary-100 transition-colors group"
                >
                  <p className="text-sm font-medium text-primary-700 group-hover:text-primary-900">
                    {sq.name}
                  </p>
                  <p className="text-xs text-primary-400">{sq.description}</p>
                </button>
              ))}
            </CardContent>
          </Card>

          {/* Schema Browser */}
          <Card>
            <CardHeader className="py-3">
              <CardTitle className="text-sm font-medium flex items-center gap-2">
                <Database className="h-4 w-4" />
                Schema Browser
              </CardTitle>
            </CardHeader>
            <CardContent className="space-y-1">
              {schema.map((table) => (
                <SchemaTable key={table.name} table={table} onInsert={setQuery} currentQuery={query} />
              ))}
            </CardContent>
          </Card>
        </div>
      </div>
    </div>
  )
}

interface SchemaTableProps {
  table: (typeof schema)[0]
  onInsert: (query: string) => void
  currentQuery: string
}

function SchemaTable({ table, onInsert, currentQuery }: SchemaTableProps) {
  const [expanded, setExpanded] = useState(false)

  const insertTableName = () => {
    // Insert table name at cursor or append
    onInsert(currentQuery + (currentQuery.endsWith(' ') ? '' : ' ') + table.name)
  }

  return (
    <div className="border-b border-primary-100 last:border-0">
      <button
        onClick={() => setExpanded(!expanded)}
        className="w-full flex items-center gap-2 py-2 text-left hover:bg-primary-50 rounded transition-colors"
      >
        {expanded ? (
          <ChevronDown className="h-3 w-3 text-primary-400" />
        ) : (
          <ChevronRight className="h-3 w-3 text-primary-400" />
        )}
        <span className="text-sm font-medium text-primary-700">{table.name}</span>
        <span className="text-xs text-primary-400">({table.columns.length})</span>
      </button>
      {expanded && (
        <div className="pl-5 pb-2 space-y-1">
          {table.columns.map((col) => (
            <div
              key={col.name}
              className="flex items-center justify-between text-xs py-0.5 group"
            >
              <span className="font-mono text-primary-600">
                {'key' in col && col.key && '🔑 '}
                {col.name}
              </span>
              <span className="text-primary-400">{col.type}</span>
            </div>
          ))}
          <button
            onClick={insertTableName}
            className="mt-2 text-xs text-primary-500 hover:text-primary-700 hover:underline"
          >
            Insert table name
          </button>
        </div>
      )}
    </div>
  )
}

function formatRelativeTime(date: Date): string {
  const now = new Date()
  const diffMs = now.getTime() - date.getTime()
  const diffMins = Math.floor(diffMs / 60000)

  if (diffMins < 1) return 'Just now'
  if (diffMins < 60) return `${diffMins}m ago`

  const diffHours = Math.floor(diffMins / 60)
  if (diffHours < 24) return `${diffHours}h ago`

  const diffDays = Math.floor(diffHours / 24)
  return `${diffDays}d ago`
}

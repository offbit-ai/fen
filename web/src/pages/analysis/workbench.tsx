import { useState } from 'react'
import {
  Play,
  Save,
  History,
  Download,
  Copy,
  Table,
  BarChart3,
} from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { cn } from '@/lib/utils'

// Sample queries for quick access
const sampleQueries = [
  {
    name: 'Anomalies by Vendor',
    sql: `SELECT vendor_name, COUNT(*) as anomaly_count, AVG(total_amount) as avg_amount
FROM invoices
JOIN anomalies ON invoices.id = anomalies.document_id
GROUP BY vendor_name
ORDER BY anomaly_count DESC
LIMIT 10;`,
  },
  {
    name: 'High Value Invoices',
    sql: `SELECT invoice_number, vendor_name, total_amount, status
FROM invoices
WHERE total_amount > 10000
ORDER BY total_amount DESC
LIMIT 20;`,
  },
  {
    name: 'Monthly Summary',
    sql: `SELECT
  DATE_TRUNC('month', invoice_date) as month,
  COUNT(*) as invoice_count,
  SUM(total_amount) as total_value
FROM invoices
GROUP BY month
ORDER BY month DESC;`,
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

export function QueryWorkbenchPage() {
  const [query, setQuery] = useState(sampleQueries[0].sql)
  const [results, setResults] = useState<typeof mockResults | null>(null)
  const [isRunning, setIsRunning] = useState(false)
  const [viewMode, setViewMode] = useState<'table' | 'chart'>('table')

  const runQuery = async () => {
    setIsRunning(true)
    // Simulate API call
    await new Promise((resolve) => setTimeout(resolve, 500))
    setResults(mockResults)
    setIsRunning(false)
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
          <Button variant="outline" size="sm">
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
        <div className="lg:col-span-3 space-y-4">
          {/* Editor */}
          <Card>
            <CardHeader className="flex flex-row items-center justify-between py-3">
              <CardTitle className="text-sm font-medium">SQL Editor</CardTitle>
              <div className="flex items-center gap-2">
                <Button
                  size="sm"
                  onClick={runQuery}
                  disabled={isRunning}
                >
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
                  className="w-full h-64 p-4 font-mono text-sm bg-primary-950 text-primary-100 rounded-b-lg resize-none focus:outline-none"
                  placeholder="SELECT * FROM invoices..."
                  spellCheck={false}
                />
                {/* Line numbers would go here in a real implementation */}
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
                              <td
                                key={j}
                                className="px-4 py-2 text-sm text-primary-700"
                              >
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

        {/* Sidebar */}
        <div className="space-y-4">
          {/* Sample Queries */}
          <Card>
            <CardHeader>
              <CardTitle className="text-sm">Sample Queries</CardTitle>
            </CardHeader>
            <CardContent className="space-y-2">
              {sampleQueries.map((sq, i) => (
                <button
                  key={i}
                  onClick={() => setQuery(sq.sql)}
                  className="w-full text-left px-3 py-2 text-sm rounded-lg hover:bg-primary-100 transition-colors"
                >
                  {sq.name}
                </button>
              ))}
            </CardContent>
          </Card>

          {/* Schema Reference */}
          <Card>
            <CardHeader>
              <CardTitle className="text-sm">Schema</CardTitle>
            </CardHeader>
            <CardContent>
              <div className="space-y-3 text-xs">
                <SchemaTable
                  name="invoices"
                  columns={['id', 'vendor_name', 'invoice_number', 'total_amount', 'status']}
                />
                <SchemaTable
                  name="anomalies"
                  columns={['id', 'document_id', 'anomaly_type', 'severity', 'status']}
                />
                <SchemaTable
                  name="contracts"
                  columns={['id', 'vendor_name', 'contract_number', 'total_value']}
                />
              </div>
            </CardContent>
          </Card>
        </div>
      </div>
    </div>
  )
}

function SchemaTable({ name, columns }: { name: string; columns: string[] }) {
  return (
    <div>
      <div className="font-medium text-primary-900 mb-1">{name}</div>
      <div className="pl-3 space-y-0.5">
        {columns.map((col) => (
          <div key={col} className="text-primary-500 font-mono">
            {col}
          </div>
        ))}
      </div>
    </div>
  )
}

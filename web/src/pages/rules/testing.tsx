import { useState } from 'react'
import {
  Play,
  FileText,
  CheckCircle,
  XCircle,
  AlertTriangle,
  Clock,
  Upload,
  Code,
  RotateCcw,
  Download,
  ChevronDown,
  ChevronRight,
  Zap,
  Calculator,
  FileSearch,
} from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { cn } from '@/lib/utils'
import type { AnomalyType, AnomalySeverity } from '@/types/api'

type TestStatus = 'idle' | 'running' | 'passed' | 'failed' | 'partial'
type ValidationEngine = 'structural' | 'zen' | 'statistical' | 'all'

interface TestResult {
  engine: ValidationEngine
  status: 'passed' | 'failed' | 'warning'
  duration_ms: number
  rules_evaluated: number
  anomalies: {
    type: AnomalyType
    severity: AnomalySeverity
    message: string
    field_path?: string
  }[]
}

// Sample invoice JSON for testing
const sampleInvoice = {
  invoice_number: 'INV-2024-TEST-001',
  invoice_date: '2024-02-04',
  due_date: '2024-03-05',
  vendor: {
    name: 'Test Vendor Inc',
    tax_id: '12-3456789',
  },
  line_items: [
    { description: 'Software License', quantity: 10, unit_price: 150.0, total: 1500.0 },
    { description: 'Support Hours', quantity: 20, unit_price: 125.0, total: 2500.0 },
  ],
  subtotal: 4000.0,
  tax_amount: 320.0,
  total_amount: 4320.0,
  currency: 'USD',
}

export function RuleTestingPage() {
  const [testInput, setTestInput] = useState(JSON.stringify(sampleInvoice, null, 2))
  const [testStatus, setTestStatus] = useState<TestStatus>('idle')
  const [selectedEngines, setSelectedEngines] = useState<ValidationEngine[]>(['all'])
  const [testResults, setTestResults] = useState<TestResult[] | null>(null)
  const [expandedResults, setExpandedResults] = useState<Set<string>>(new Set(['structural', 'zen', 'statistical']))

  const runTests = () => {
    setTestStatus('running')
    setTestResults(null)

    // Simulate test execution
    setTimeout(() => {
      const results: TestResult[] = [
        {
          engine: 'structural',
          status: 'passed',
          duration_ms: 12,
          rules_evaluated: 15,
          anomalies: [],
        },
        {
          engine: 'zen',
          status: 'warning',
          duration_ms: 45,
          rules_evaluated: 8,
          anomalies: [
            {
              type: 'missing_field',
              severity: 'low',
              message: 'Optional field "po_number" is missing',
              field_path: 'po_number',
            },
          ],
        },
        {
          engine: 'statistical',
          status: 'failed',
          duration_ms: 89,
          rules_evaluated: 5,
          anomalies: [
            {
              type: 'statistical_outlier',
              severity: 'high',
              message: 'Total amount is 2.3σ above vendor baseline',
              field_path: 'total_amount',
            },
            {
              type: 'out_of_range',
              severity: 'medium',
              message: 'Unit price for "Software License" exceeds typical range',
              field_path: 'line_items[0].unit_price',
            },
          ],
        },
      ]

      setTestResults(results)
      const hasFailures = results.some((r) => r.status === 'failed')
      const hasWarnings = results.some((r) => r.status === 'warning')
      setTestStatus(hasFailures ? 'failed' : hasWarnings ? 'partial' : 'passed')
    }, 1500)
  }

  const toggleEngine = (engine: ValidationEngine) => {
    if (engine === 'all') {
      setSelectedEngines(['all'])
    } else {
      const newEngines = selectedEngines.filter((e) => e !== 'all')
      if (newEngines.includes(engine)) {
        const filtered = newEngines.filter((e) => e !== engine)
        setSelectedEngines(filtered.length === 0 ? ['all'] : filtered)
      } else {
        setSelectedEngines([...newEngines, engine])
      }
    }
  }

  const toggleResultExpanded = (engine: string) => {
    const newExpanded = new Set(expandedResults)
    if (newExpanded.has(engine)) {
      newExpanded.delete(engine)
    } else {
      newExpanded.add(engine)
    }
    setExpandedResults(newExpanded)
  }

  const statusConfig: Record<TestStatus, { label: string; className: string; icon: React.ReactNode }> = {
    idle: { label: 'Ready', className: 'text-primary-500', icon: <Clock className="h-4 w-4" /> },
    running: { label: 'Running...', className: 'text-blue-600', icon: <RotateCcw className="h-4 w-4 animate-spin" /> },
    passed: { label: 'All Passed', className: 'text-green-600', icon: <CheckCircle className="h-4 w-4" /> },
    failed: { label: 'Failed', className: 'text-red-600', icon: <XCircle className="h-4 w-4" /> },
    partial: { label: 'Passed with Warnings', className: 'text-yellow-600', icon: <AlertTriangle className="h-4 w-4" /> },
  }

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-primary-900">Rule Testing</h1>
          <p className="text-sm text-primary-500">
            Test validation rules against sample documents
          </p>
        </div>
        <div className="flex items-center gap-2">
          <div className={cn('flex items-center gap-2 px-3 py-1.5 rounded-lg', testStatus === 'idle' ? 'bg-primary-100' : testStatus === 'running' ? 'bg-blue-100' : testStatus === 'passed' ? 'bg-green-100' : testStatus === 'failed' ? 'bg-red-100' : 'bg-yellow-100')}>
            {statusConfig[testStatus].icon}
            <span className={cn('text-sm font-medium', statusConfig[testStatus].className)}>
              {statusConfig[testStatus].label}
            </span>
          </div>
        </div>
      </div>

      <div className="grid gap-6 lg:grid-cols-2">
        {/* Input Panel */}
        <div className="space-y-6">
          {/* Engine Selection */}
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <Zap className="h-5 w-5" />
                Validation Engines
              </CardTitle>
            </CardHeader>
            <CardContent>
              <div className="flex flex-wrap gap-2">
                <EngineToggle
                  label="All Engines"
                  active={selectedEngines.includes('all')}
                  onClick={() => toggleEngine('all')}
                  icon={<Zap className="h-4 w-4" />}
                />
                <EngineToggle
                  label="Structural"
                  active={selectedEngines.includes('structural') || selectedEngines.includes('all')}
                  onClick={() => toggleEngine('structural')}
                  icon={<FileSearch className="h-4 w-4" />}
                  disabled={selectedEngines.includes('all')}
                />
                <EngineToggle
                  label="Zen (GoRules)"
                  active={selectedEngines.includes('zen') || selectedEngines.includes('all')}
                  onClick={() => toggleEngine('zen')}
                  icon={<Code className="h-4 w-4" />}
                  disabled={selectedEngines.includes('all')}
                />
                <EngineToggle
                  label="Statistical"
                  active={selectedEngines.includes('statistical') || selectedEngines.includes('all')}
                  onClick={() => toggleEngine('statistical')}
                  icon={<Calculator className="h-4 w-4" />}
                  disabled={selectedEngines.includes('all')}
                />
              </div>
            </CardContent>
          </Card>

          {/* Test Input */}
          <Card>
            <CardHeader>
              <div className="flex items-center justify-between">
                <CardTitle className="flex items-center gap-2">
                  <FileText className="h-5 w-5" />
                  Test Document
                </CardTitle>
                <div className="flex items-center gap-2">
                  <Button variant="outline" size="sm">
                    <Upload className="mr-2 h-4 w-4" />
                    Load File
                  </Button>
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={() => setTestInput(JSON.stringify(sampleInvoice, null, 2))}
                  >
                    <RotateCcw className="mr-2 h-4 w-4" />
                    Reset
                  </Button>
                </div>
              </div>
            </CardHeader>
            <CardContent>
              <textarea
                value={testInput}
                onChange={(e) => setTestInput(e.target.value)}
                className="w-full h-96 p-4 font-mono text-sm rounded-lg border border-primary-200 bg-primary-50 text-primary-900 focus:outline-none focus:ring-2 focus:ring-primary-500"
                placeholder="Paste JSON document here..."
              />
              <div className="mt-4 flex justify-end">
                <Button
                  onClick={runTests}
                  disabled={testStatus === 'running'}
                  className="min-w-32"
                >
                  {testStatus === 'running' ? (
                    <>
                      <RotateCcw className="mr-2 h-4 w-4 animate-spin" />
                      Running...
                    </>
                  ) : (
                    <>
                      <Play className="mr-2 h-4 w-4" />
                      Run Tests
                    </>
                  )}
                </Button>
              </div>
            </CardContent>
          </Card>
        </div>

        {/* Results Panel */}
        <div className="space-y-6">
          <Card>
            <CardHeader>
              <div className="flex items-center justify-between">
                <CardTitle>Test Results</CardTitle>
                {testResults && (
                  <Button variant="outline" size="sm">
                    <Download className="mr-2 h-4 w-4" />
                    Export
                  </Button>
                )}
              </div>
            </CardHeader>
            <CardContent>
              {!testResults ? (
                <div className="flex flex-col items-center justify-center py-12 text-primary-400">
                  <Play className="h-12 w-12 mb-4" />
                  <p className="text-sm">Run tests to see results</p>
                </div>
              ) : (
                <div className="space-y-4">
                  {/* Summary */}
                  <div className="grid grid-cols-3 gap-4">
                    <SummaryCard
                      label="Total Rules"
                      value={testResults.reduce((sum, r) => sum + r.rules_evaluated, 0)}
                      icon={<FileText className="h-4 w-4" />}
                    />
                    <SummaryCard
                      label="Anomalies Found"
                      value={testResults.reduce((sum, r) => sum + r.anomalies.length, 0)}
                      icon={<AlertTriangle className="h-4 w-4" />}
                      highlight={testResults.reduce((sum, r) => sum + r.anomalies.length, 0) > 0}
                    />
                    <SummaryCard
                      label="Total Time"
                      value={`${testResults.reduce((sum, r) => sum + r.duration_ms, 0)}ms`}
                      icon={<Clock className="h-4 w-4" />}
                    />
                  </div>

                  {/* Engine Results */}
                  <div className="space-y-3">
                    {testResults.map((result) => (
                      <EngineResult
                        key={result.engine}
                        result={result}
                        expanded={expandedResults.has(result.engine)}
                        onToggle={() => toggleResultExpanded(result.engine)}
                      />
                    ))}
                  </div>
                </div>
              )}
            </CardContent>
          </Card>

          {/* Quick Reference */}
          <Card>
            <CardHeader>
              <CardTitle>Quick Reference</CardTitle>
            </CardHeader>
            <CardContent>
              <div className="space-y-3 text-sm">
                <div className="flex items-start gap-3">
                  <FileSearch className="h-4 w-4 mt-0.5 text-blue-600" />
                  <div>
                    <p className="font-medium text-primary-900">Structural Validation</p>
                    <p className="text-primary-500">Schema validation, required fields, data types</p>
                  </div>
                </div>
                <div className="flex items-start gap-3">
                  <Code className="h-4 w-4 mt-0.5 text-purple-600" />
                  <div>
                    <p className="font-medium text-primary-900">Zen (GoRules) Engine</p>
                    <p className="text-primary-500">Business rules, decision tables, custom logic</p>
                  </div>
                </div>
                <div className="flex items-start gap-3">
                  <Calculator className="h-4 w-4 mt-0.5 text-green-600" />
                  <div>
                    <p className="font-medium text-primary-900">Statistical Analysis</p>
                    <p className="text-primary-500">Baseline comparisons, outlier detection, trends</p>
                  </div>
                </div>
              </div>
            </CardContent>
          </Card>
        </div>
      </div>
    </div>
  )
}

function EngineToggle({
  label,
  active,
  onClick,
  icon,
  disabled,
}: {
  label: string
  active: boolean
  onClick: () => void
  icon: React.ReactNode
  disabled?: boolean
}) {
  return (
    <button
      onClick={onClick}
      disabled={disabled}
      className={cn(
        'flex items-center gap-2 px-3 py-2 rounded-lg border text-sm font-medium transition-colors',
        active
          ? 'bg-primary-900 text-white border-primary-900'
          : 'bg-white text-primary-700 border-primary-200 hover:bg-primary-50',
        disabled && 'opacity-50 cursor-not-allowed'
      )}
    >
      {icon}
      {label}
    </button>
  )
}

function SummaryCard({
  label,
  value,
  icon,
  highlight,
}: {
  label: string
  value: string | number
  icon: React.ReactNode
  highlight?: boolean
}) {
  return (
    <div className={cn(
      'rounded-lg border p-3',
      highlight ? 'border-yellow-200 bg-yellow-50' : 'border-primary-100'
    )}>
      <div className="flex items-center gap-2 text-primary-500">
        {icon}
        <span className="text-xs">{label}</span>
      </div>
      <p className={cn(
        'text-xl font-bold mt-1',
        highlight ? 'text-yellow-700' : 'text-primary-900'
      )}>
        {value}
      </p>
    </div>
  )
}

function EngineResult({
  result,
  expanded,
  onToggle,
}: {
  result: TestResult
  expanded: boolean
  onToggle: () => void
}) {
  const engineLabels: Record<ValidationEngine, string> = {
    structural: 'Structural Validation',
    zen: 'Zen (GoRules) Engine',
    statistical: 'Statistical Analysis',
    all: 'All Engines',
  }

  const engineIcons: Record<ValidationEngine, React.ReactNode> = {
    structural: <FileSearch className="h-4 w-4" />,
    zen: <Code className="h-4 w-4" />,
    statistical: <Calculator className="h-4 w-4" />,
    all: <Zap className="h-4 w-4" />,
  }

  const statusIcons: Record<string, { icon: React.ReactNode; className: string }> = {
    passed: { icon: <CheckCircle className="h-5 w-5" />, className: 'text-green-600' },
    failed: { icon: <XCircle className="h-5 w-5" />, className: 'text-red-600' },
    warning: { icon: <AlertTriangle className="h-5 w-5" />, className: 'text-yellow-600' },
  }

  const severityColors: Record<AnomalySeverity, string> = {
    critical: 'bg-red-100 text-red-700 border-red-200',
    high: 'bg-orange-100 text-orange-700 border-orange-200',
    medium: 'bg-yellow-100 text-yellow-700 border-yellow-200',
    low: 'bg-blue-100 text-blue-700 border-blue-200',
  }

  return (
    <div className="rounded-lg border border-primary-200">
      <button
        onClick={onToggle}
        className="w-full flex items-center justify-between p-4 hover:bg-primary-50 transition-colors"
      >
        <div className="flex items-center gap-3">
          <div className={cn('p-2 rounded-lg', result.status === 'passed' ? 'bg-green-100' : result.status === 'failed' ? 'bg-red-100' : 'bg-yellow-100')}>
            {engineIcons[result.engine]}
          </div>
          <div className="text-left">
            <p className="font-medium text-primary-900">{engineLabels[result.engine]}</p>
            <p className="text-xs text-primary-500">
              {result.rules_evaluated} rules • {result.duration_ms}ms
            </p>
          </div>
        </div>
        <div className="flex items-center gap-3">
          <div className={statusIcons[result.status].className}>
            {statusIcons[result.status].icon}
          </div>
          {expanded ? (
            <ChevronDown className="h-4 w-4 text-primary-400" />
          ) : (
            <ChevronRight className="h-4 w-4 text-primary-400" />
          )}
        </div>
      </button>

      {expanded && result.anomalies.length > 0 && (
        <div className="border-t border-primary-200 p-4 space-y-2">
          {result.anomalies.map((anomaly, i) => (
            <div
              key={i}
              className={cn(
                'rounded-lg border p-3',
                severityColors[anomaly.severity]
              )}
            >
              <div className="flex items-center justify-between">
                <span className="text-sm font-medium">{anomaly.message}</span>
                <span className="text-xs font-medium uppercase">{anomaly.severity}</span>
              </div>
              {anomaly.field_path && (
                <code className="text-xs mt-1 block opacity-75">{anomaly.field_path}</code>
              )}
            </div>
          ))}
        </div>
      )}

      {expanded && result.anomalies.length === 0 && (
        <div className="border-t border-primary-200 p-4 text-center text-sm text-primary-500">
          No anomalies detected
        </div>
      )}
    </div>
  )
}

import { useState } from 'react'
import {
  GitBranch,
  ZoomIn,
  ZoomOut,
  Maximize2,
  RefreshCw,
  Play,
  AlertTriangle,
  CheckCircle,
  Calculator,
  FileSearch,
  TrendingUp,
  Shield,
  ChevronRight,
} from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { cn } from '@/lib/utils'

// Node types matching backend fen-rules engine
type NodeType = 'input' | 'structural' | 'zen_rule' | 'statistical' | 'action' | 'output'
type ValidationEngine = 'structural' | 'zen' | 'statistical'

// AnomalyType matching backend fen-core/src/domain/anomaly.rs
type AnomalyType =
  | 'math_mismatch'
  | 'missing_field'
  | 'invalid_format'
  | 'out_of_range'
  | 'potential_duplicate'
  | 'date_inconsistency'
  | 'contract_violation'
  | 'validation_failure'
  | 'statistical_outlier'

// Severity matching backend
type Severity = 'low' | 'medium' | 'high' | 'critical'

interface RuleNode {
  id: string
  type: NodeType
  engine?: ValidationEngine
  name: string
  description: string
  anomalyType?: AnomalyType
  severity?: Severity
  position: { x: number; y: number }
  connections: string[]
  metrics?: string[] // For statistical nodes
  enabled: boolean
  executionCount?: number
  matchRate?: number
}

// Mock rule graph matching backend validation flow
const mockNodes: RuleNode[] = [
  // Input Node
  {
    id: 'input',
    type: 'input',
    name: 'Invoice Input',
    description: 'Incoming invoice documents',
    position: { x: 50, y: 200 },
    connections: ['structural-math', 'structural-dates', 'structural-fields', 'zen-pricing', 'zen-duplicate', 'statistical-total'],
    enabled: true,
    executionCount: 12847,
  },

  // Structural Validation Nodes (built-in validators)
  {
    id: 'structural-math',
    type: 'structural',
    engine: 'structural',
    name: 'Math Validation',
    description: 'Line items sum = subtotal, total = subtotal + tax - discount',
    anomalyType: 'math_mismatch',
    severity: 'high',
    position: { x: 250, y: 50 },
    connections: ['action-flag'],
    enabled: true,
    executionCount: 12847,
    matchRate: 0.8,
  },
  {
    id: 'structural-dates',
    type: 'structural',
    engine: 'structural',
    name: 'Date Validation',
    description: 'Due date must be after invoice date',
    anomalyType: 'date_inconsistency',
    severity: 'medium',
    position: { x: 250, y: 130 },
    connections: ['action-flag'],
    enabled: true,
    executionCount: 12847,
    matchRate: 0.2,
  },
  {
    id: 'structural-fields',
    type: 'structural',
    engine: 'structural',
    name: 'Required Fields',
    description: 'Invoice number, vendor info, positive amounts',
    anomalyType: 'missing_field',
    severity: 'high',
    position: { x: 250, y: 210 },
    connections: ['action-flag'],
    enabled: true,
    executionCount: 12847,
    matchRate: 1.2,
  },

  // Zen Engine Nodes (GoRules decision tables)
  {
    id: 'zen-pricing',
    type: 'zen_rule',
    engine: 'zen',
    name: 'Contract Pricing',
    description: 'Validate prices against contract terms',
    anomalyType: 'contract_violation',
    severity: 'high',
    position: { x: 250, y: 290 },
    connections: ['action-flag', 'action-alert'],
    enabled: true,
    executionCount: 8543,
    matchRate: 1.0,
  },
  {
    id: 'zen-duplicate',
    type: 'zen_rule',
    engine: 'zen',
    name: 'Duplicate Detection',
    description: 'Check for potential duplicate invoices',
    anomalyType: 'potential_duplicate',
    severity: 'critical',
    position: { x: 250, y: 370 },
    connections: ['action-flag', 'action-alert'],
    enabled: true,
    executionCount: 12847,
    matchRate: 0.4,
  },

  // Statistical Analysis Node
  {
    id: 'statistical-total',
    type: 'statistical',
    engine: 'statistical',
    name: 'Statistical Outlier',
    description: 'Z-score analysis for total_amount, subtotal, line_item_count',
    anomalyType: 'statistical_outlier',
    severity: 'medium',
    metrics: ['total_amount', 'subtotal', 'tax_amount', 'line_item_count'],
    position: { x: 250, y: 450 },
    connections: ['action-flag'],
    enabled: true,
    executionCount: 12847,
    matchRate: 2.1,
  },

  // Action Nodes
  {
    id: 'action-flag',
    type: 'action',
    name: 'Flag Anomaly',
    description: 'Create anomaly record in database',
    position: { x: 500, y: 200 },
    connections: ['output'],
    enabled: true,
    executionCount: 547,
  },
  {
    id: 'action-alert',
    type: 'action',
    name: 'Send Alert',
    description: 'Notify via email/Slack for critical issues',
    position: { x: 500, y: 320 },
    connections: ['output'],
    enabled: true,
    executionCount: 45,
  },

  // Output Node
  {
    id: 'output',
    type: 'output',
    name: 'Validation Result',
    description: 'ValidationResult with anomalies and is_valid status',
    position: { x: 700, y: 260 },
    connections: [],
    enabled: true,
    executionCount: 12847,
  },
]

export function RuleGraphPage() {
  const [selectedNode, setSelectedNode] = useState<RuleNode | null>(null)
  const [zoom, setZoom] = useState(100)
  const [filterEngine, setFilterEngine] = useState<ValidationEngine | 'all'>('all')

  const filteredNodes = mockNodes.filter((n) => {
    if (filterEngine === 'all') return true
    return n.engine === filterEngine || !n.engine
  })

  const stats = {
    totalNodes: mockNodes.filter((n) => n.type !== 'input' && n.type !== 'output').length,
    structural: mockNodes.filter((n) => n.engine === 'structural').length,
    zenRules: mockNodes.filter((n) => n.engine === 'zen').length,
    statistical: mockNodes.filter((n) => n.engine === 'statistical').length,
  }

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-primary-900">Rule Graph</h1>
          <p className="text-sm text-primary-500">
            Visualize validation rule dependencies and data flow
          </p>
        </div>
        <div className="flex items-center gap-2">
          <Button variant="outline" size="sm">
            <RefreshCw className="mr-2 h-4 w-4" />
            Refresh
          </Button>
          <Button>
            <Play className="mr-2 h-4 w-4" />
            Run All Rules
          </Button>
        </div>
      </div>

      {/* Stats */}
      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
        <StatCard
          title="Total Rules"
          value={stats.totalNodes}
          icon={<GitBranch className="h-5 w-5" />}
        />
        <StatCard
          title="Structural"
          value={stats.structural}
          description="Built-in validators"
          icon={<Calculator className="h-5 w-5" />}
          color="blue"
        />
        <StatCard
          title="Zen Rules"
          value={stats.zenRules}
          description="GoRules decision tables"
          icon={<FileSearch className="h-5 w-5" />}
          color="purple"
        />
        <StatCard
          title="Statistical"
          value={stats.statistical}
          description="Baseline analysis"
          icon={<TrendingUp className="h-5 w-5" />}
          color="green"
        />
      </div>

      <div className="grid gap-6 lg:grid-cols-3">
        {/* Graph Canvas */}
        <Card className="lg:col-span-2">
          <CardHeader className="flex flex-row items-center justify-between pb-2">
            <CardTitle className="text-lg">Validation Flow</CardTitle>
            <div className="flex items-center gap-2">
              <select
                value={filterEngine}
                onChange={(e) => setFilterEngine(e.target.value as ValidationEngine | 'all')}
                className="h-8 rounded-lg border border-primary-200 bg-white px-2 text-sm"
              >
                <option value="all">All Engines</option>
                <option value="structural">Structural</option>
                <option value="zen">Zen (GoRules)</option>
                <option value="statistical">Statistical</option>
              </select>
              <Button variant="outline" size="sm" onClick={() => setZoom(Math.max(50, zoom - 10))}>
                <ZoomOut className="h-4 w-4" />
              </Button>
              <span className="text-sm text-primary-500 w-12 text-center">{zoom}%</span>
              <Button variant="outline" size="sm" onClick={() => setZoom(Math.min(150, zoom + 10))}>
                <ZoomIn className="h-4 w-4" />
              </Button>
              <Button variant="outline" size="sm">
                <Maximize2 className="h-4 w-4" />
              </Button>
            </div>
          </CardHeader>
          <CardContent>
            <div
              className="relative bg-primary-50 rounded-lg overflow-hidden"
              style={{
                height: '500px',
                transform: `scale(${zoom / 100})`,
                transformOrigin: 'top left',
              }}
            >
              {/* SVG Connections */}
              <svg className="absolute inset-0 w-full h-full pointer-events-none">
                {filteredNodes.map((node) =>
                  node.connections
                    .filter((targetId) => filteredNodes.some((n) => n.id === targetId))
                    .map((targetId) => {
                      const target = filteredNodes.find((n) => n.id === targetId)
                      if (!target) return null
                      return (
                        <line
                          key={`${node.id}-${targetId}`}
                          x1={node.position.x + 80}
                          y1={node.position.y + 30}
                          x2={target.position.x}
                          y2={target.position.y + 30}
                          stroke="#94a3b8"
                          strokeWidth="2"
                          markerEnd="url(#arrowhead)"
                        />
                      )
                    })
                )}
                <defs>
                  <marker
                    id="arrowhead"
                    markerWidth="10"
                    markerHeight="7"
                    refX="9"
                    refY="3.5"
                    orient="auto"
                  >
                    <polygon points="0 0, 10 3.5, 0 7" fill="#94a3b8" />
                  </marker>
                </defs>
              </svg>

              {/* Nodes */}
              {filteredNodes.map((node) => (
                <GraphNode
                  key={node.id}
                  node={node}
                  selected={selectedNode?.id === node.id}
                  onClick={() => setSelectedNode(node)}
                />
              ))}
            </div>

            {/* Legend */}
            <div className="flex items-center gap-6 mt-4 text-sm">
              <div className="flex items-center gap-2">
                <div className="w-4 h-4 rounded bg-primary-200" />
                <span className="text-primary-600">Input/Output</span>
              </div>
              <div className="flex items-center gap-2">
                <div className="w-4 h-4 rounded bg-blue-200" />
                <span className="text-primary-600">Structural</span>
              </div>
              <div className="flex items-center gap-2">
                <div className="w-4 h-4 rounded bg-purple-200" />
                <span className="text-primary-600">Zen (GoRules)</span>
              </div>
              <div className="flex items-center gap-2">
                <div className="w-4 h-4 rounded bg-green-200" />
                <span className="text-primary-600">Statistical</span>
              </div>
              <div className="flex items-center gap-2">
                <div className="w-4 h-4 rounded bg-orange-200" />
                <span className="text-primary-600">Action</span>
              </div>
            </div>
          </CardContent>
        </Card>

        {/* Node Details Panel */}
        <Card>
          <CardHeader>
            <CardTitle className="text-lg">Node Details</CardTitle>
          </CardHeader>
          <CardContent>
            {selectedNode ? (
              <NodeDetails node={selectedNode} />
            ) : (
              <div className="text-center py-12 text-primary-400">
                <GitBranch className="mx-auto h-8 w-8 mb-2" />
                <p>Select a node to view details</p>
              </div>
            )}
          </CardContent>
        </Card>
      </div>
    </div>
  )
}

function StatCard({
  title,
  value,
  description,
  icon,
  color = 'gray',
}: {
  title: string
  value: number
  description?: string
  icon: React.ReactNode
  color?: 'gray' | 'blue' | 'purple' | 'green'
}) {
  const colorClasses = {
    gray: 'bg-primary-100 text-primary-600',
    blue: 'bg-blue-100 text-blue-600',
    purple: 'bg-purple-100 text-purple-600',
    green: 'bg-green-100 text-green-600',
  }

  return (
    <Card>
      <CardContent className="p-4">
        <div className="flex items-center gap-3">
          <div className={cn('rounded-lg p-2', colorClasses[color])}>{icon}</div>
          <div>
            <p className="text-sm text-primary-500">{title}</p>
            <p className="text-2xl font-bold text-primary-900">{value}</p>
            {description && <p className="text-xs text-primary-400">{description}</p>}
          </div>
        </div>
      </CardContent>
    </Card>
  )
}

interface GraphNodeProps {
  node: RuleNode
  selected: boolean
  onClick: () => void
}

function GraphNode({ node, selected, onClick }: GraphNodeProps) {
  const nodeColors: Record<NodeType, string> = {
    input: 'bg-primary-100 border-primary-300',
    structural: 'bg-blue-100 border-blue-300',
    zen_rule: 'bg-purple-100 border-purple-300',
    statistical: 'bg-green-100 border-green-300',
    action: 'bg-orange-100 border-orange-300',
    output: 'bg-primary-100 border-primary-300',
  }

  const nodeIcons: Record<NodeType, React.ReactNode> = {
    input: <ChevronRight className="h-4 w-4" />,
    structural: <Calculator className="h-4 w-4" />,
    zen_rule: <FileSearch className="h-4 w-4" />,
    statistical: <TrendingUp className="h-4 w-4" />,
    action: <AlertTriangle className="h-4 w-4" />,
    output: <CheckCircle className="h-4 w-4" />,
  }

  return (
    <div
      className={cn(
        'absolute cursor-pointer rounded-lg border-2 p-3 transition-all hover:shadow-md',
        nodeColors[node.type],
        selected && 'ring-2 ring-primary-500 ring-offset-2',
        !node.enabled && 'opacity-50'
      )}
      style={{
        left: node.position.x,
        top: node.position.y,
        width: '160px',
      }}
      onClick={onClick}
    >
      <div className="flex items-center gap-2">
        {nodeIcons[node.type]}
        <span className="text-sm font-medium truncate">{node.name}</span>
      </div>
      {node.matchRate !== undefined && (
        <div className="mt-1 text-xs text-primary-500">
          {node.matchRate.toFixed(1)}% match rate
        </div>
      )}
    </div>
  )
}

function NodeDetails({ node }: { node: RuleNode }) {
  const severityColors: Record<Severity, string> = {
    low: 'bg-green-100 text-green-700',
    medium: 'bg-yellow-100 text-yellow-700',
    high: 'bg-orange-100 text-orange-700',
    critical: 'bg-red-100 text-red-700',
  }

  const engineLabels: Record<ValidationEngine, string> = {
    structural: 'Structural Validator',
    zen: 'Zen Engine (GoRules)',
    statistical: 'Statistical Analyzer',
  }

  const anomalyLabels: Record<AnomalyType, string> = {
    math_mismatch: 'Math Mismatch',
    missing_field: 'Missing Field',
    invalid_format: 'Invalid Format',
    out_of_range: 'Out of Range',
    potential_duplicate: 'Potential Duplicate',
    date_inconsistency: 'Date Inconsistency',
    contract_violation: 'Contract Violation',
    validation_failure: 'Validation Failure',
    statistical_outlier: 'Statistical Outlier',
  }

  return (
    <div className="space-y-4">
      <div>
        <div className="flex items-center gap-2 mb-2">
          <h3 className="font-semibold text-primary-900">{node.name}</h3>
          {node.enabled ? (
            <span className="inline-flex items-center gap-1 rounded-full bg-green-100 px-2 py-0.5 text-xs text-green-700">
              <CheckCircle className="h-3 w-3" />
              Enabled
            </span>
          ) : (
            <span className="inline-flex items-center gap-1 rounded-full bg-gray-100 px-2 py-0.5 text-xs text-gray-700">
              Disabled
            </span>
          )}
        </div>
        <p className="text-sm text-primary-500">{node.description}</p>
      </div>

      <div className="space-y-3">
        <DetailRow label="Type" value={node.type.replace('_', ' ').toUpperCase()} />

        {node.engine && (
          <DetailRow label="Engine" value={engineLabels[node.engine]} />
        )}

        {node.anomalyType && (
          <DetailRow label="Anomaly Type" value={anomalyLabels[node.anomalyType]} />
        )}

        {node.severity && (
          <div className="flex items-center justify-between text-sm">
            <span className="text-primary-500">Severity</span>
            <span className={cn('rounded-full px-2 py-0.5 text-xs font-medium', severityColors[node.severity])}>
              {node.severity.toUpperCase()}
            </span>
          </div>
        )}

        {node.metrics && node.metrics.length > 0 && (
          <div>
            <span className="text-sm text-primary-500">Metrics Analyzed</span>
            <div className="mt-1 flex flex-wrap gap-1">
              {node.metrics.map((metric) => (
                <span
                  key={metric}
                  className="rounded bg-primary-100 px-2 py-0.5 text-xs font-mono text-primary-700"
                >
                  {metric}
                </span>
              ))}
            </div>
          </div>
        )}

        {node.executionCount !== undefined && (
          <DetailRow
            label="Executions"
            value={node.executionCount.toLocaleString()}
          />
        )}

        {node.matchRate !== undefined && (
          <DetailRow
            label="Match Rate"
            value={`${node.matchRate.toFixed(2)}%`}
          />
        )}

        {node.connections.length > 0 && (
          <div>
            <span className="text-sm text-primary-500">Connects To</span>
            <div className="mt-1 flex flex-wrap gap-1">
              {node.connections.map((conn) => (
                <span
                  key={conn}
                  className="rounded bg-primary-100 px-2 py-0.5 text-xs text-primary-700"
                >
                  {conn}
                </span>
              ))}
            </div>
          </div>
        )}
      </div>

      <div className="flex gap-2 pt-4 border-t border-primary-100">
        <Button variant="outline" size="sm" className="flex-1">
          <Shield className="mr-2 h-4 w-4" />
          Edit Rule
        </Button>
        <Button variant="outline" size="sm" className="flex-1">
          <Play className="mr-2 h-4 w-4" />
          Test
        </Button>
      </div>
    </div>
  )
}

function DetailRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-center justify-between text-sm">
      <span className="text-primary-500">{label}</span>
      <span className="text-primary-900 font-medium">{value}</span>
    </div>
  )
}

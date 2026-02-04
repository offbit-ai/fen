import { useState } from 'react'
import {
  Plug,
  Database,
  Cloud,
  FileText,
  Lock,
  CheckCircle,
  XCircle,
  RefreshCw,
  Settings,
  Plus,
  AlertTriangle,
} from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Card, CardContent } from '@/components/ui/card'
import { cn } from '@/lib/utils'

type IntegrationStatus = 'connected' | 'disconnected' | 'error' | 'pending'
type IntegrationCategory = 'storage' | 'erp' | 'authentication' | 'analytics'

interface Integration {
  id: string
  name: string
  description: string
  category: IntegrationCategory
  status: IntegrationStatus
  icon: string
  last_sync?: string
  config?: Record<string, string>
  features: string[]
}

const mockIntegrations: Integration[] = [
  {
    id: 'int-001',
    name: 'Amazon S3',
    description: 'Cloud storage for document archival',
    category: 'storage',
    status: 'connected',
    icon: 'aws',
    last_sync: '2024-02-04T10:30:00Z',
    config: { bucket: 'fen-documents-prod', region: 'us-east-1' },
    features: ['Document storage', 'Backup', 'Archive'],
  },
  {
    id: 'int-002',
    name: 'SAP ERP',
    description: 'Enterprise resource planning integration',
    category: 'erp',
    status: 'connected',
    icon: 'sap',
    last_sync: '2024-02-04T09:00:00Z',
    config: { instance: 'S4HANA', client: '100' },
    features: ['Invoice sync', 'Vendor data', 'PO matching'],
  },
  {
    id: 'int-003',
    name: 'Okta SSO',
    description: 'Single sign-on authentication',
    category: 'authentication',
    status: 'connected',
    icon: 'okta',
    features: ['SSO', 'MFA', 'User provisioning'],
  },
  {
    id: 'int-004',
    name: 'NetSuite',
    description: 'Cloud ERP and accounting',
    category: 'erp',
    status: 'disconnected',
    icon: 'netsuite',
    features: ['Invoice sync', 'GL posting', 'Vendor management'],
  },
  {
    id: 'int-005',
    name: 'Google Cloud Storage',
    description: 'Alternative cloud storage provider',
    category: 'storage',
    status: 'disconnected',
    icon: 'gcs',
    features: ['Document storage', 'Backup', 'Archive'],
  },
  {
    id: 'int-006',
    name: 'Snowflake',
    description: 'Data warehouse for analytics',
    category: 'analytics',
    status: 'error',
    icon: 'snowflake',
    last_sync: '2024-02-03T15:00:00Z',
    features: ['Data export', 'BI integration', 'Historical analysis'],
  },
]

const availableIntegrations: Omit<Integration, 'status' | 'last_sync' | 'config'>[] = [
  {
    id: 'avail-001',
    name: 'Oracle ERP Cloud',
    description: 'Enterprise cloud ERP solution',
    category: 'erp',
    icon: 'oracle',
    features: ['Invoice sync', 'AP automation', 'Vendor management'],
  },
  {
    id: 'avail-002',
    name: 'Azure AD',
    description: 'Microsoft identity platform',
    category: 'authentication',
    icon: 'azure',
    features: ['SSO', 'MFA', 'Conditional access'],
  },
  {
    id: 'avail-003',
    name: 'Tableau',
    description: 'Business intelligence and analytics',
    category: 'analytics',
    icon: 'tableau',
    features: ['Dashboards', 'Data visualization', 'Reporting'],
  },
]

const categoryConfig: Record<IntegrationCategory, { label: string; icon: React.ReactNode }> = {
  storage: { label: 'Storage', icon: <Cloud className="h-4 w-4" /> },
  erp: { label: 'ERP Systems', icon: <Database className="h-4 w-4" /> },
  authentication: { label: 'Authentication', icon: <Lock className="h-4 w-4" /> },
  analytics: { label: 'Analytics', icon: <FileText className="h-4 w-4" /> },
}

const statusConfig: Record<IntegrationStatus, { label: string; icon: React.ReactNode; className: string }> = {
  connected: { label: 'Connected', icon: <CheckCircle className="h-4 w-4" />, className: 'text-green-600 bg-green-100' },
  disconnected: { label: 'Disconnected', icon: <XCircle className="h-4 w-4" />, className: 'text-primary-500 bg-primary-100' },
  error: { label: 'Error', icon: <AlertTriangle className="h-4 w-4" />, className: 'text-red-600 bg-red-100' },
  pending: { label: 'Pending', icon: <RefreshCw className="h-4 w-4 animate-spin" />, className: 'text-yellow-600 bg-yellow-100' },
}

export function IntegrationsPage() {
  const [integrations] = useState(mockIntegrations)
  const [selectedCategory, setSelectedCategory] = useState<IntegrationCategory | 'all'>('all')

  const filteredIntegrations = integrations.filter(
    (i) => selectedCategory === 'all' || i.category === selectedCategory
  )

  const connectedCount = integrations.filter((i) => i.status === 'connected').length
  const errorCount = integrations.filter((i) => i.status === 'error').length

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-primary-900">Integrations</h1>
          <p className="text-sm text-primary-500">
            Connect Fen with your existing tools and services
          </p>
        </div>
        <Button>
          <Plus className="mr-2 h-4 w-4" />
          Add Integration
        </Button>
      </div>

      {/* Stats */}
      <div className="grid gap-4 sm:grid-cols-3">
        <Card>
          <CardContent className="p-4">
            <div className="flex items-center gap-3">
              <div className="rounded-lg bg-green-100 p-2">
                <CheckCircle className="h-5 w-5 text-green-600" />
              </div>
              <div>
                <p className="text-sm text-primary-500">Connected</p>
                <p className="text-2xl font-bold text-primary-900">{connectedCount}</p>
              </div>
            </div>
          </CardContent>
        </Card>
        <Card>
          <CardContent className="p-4">
            <div className="flex items-center gap-3">
              <div className="rounded-lg bg-red-100 p-2">
                <AlertTriangle className="h-5 w-5 text-red-600" />
              </div>
              <div>
                <p className="text-sm text-primary-500">Errors</p>
                <p className="text-2xl font-bold text-primary-900">{errorCount}</p>
              </div>
            </div>
          </CardContent>
        </Card>
        <Card>
          <CardContent className="p-4">
            <div className="flex items-center gap-3">
              <div className="rounded-lg bg-primary-100 p-2">
                <Plug className="h-5 w-5 text-primary-600" />
              </div>
              <div>
                <p className="text-sm text-primary-500">Available</p>
                <p className="text-2xl font-bold text-primary-900">{availableIntegrations.length}</p>
              </div>
            </div>
          </CardContent>
        </Card>
      </div>

      {/* Filter Tabs */}
      <Card className="p-4">
        <div className="flex flex-wrap gap-2">
          <FilterTab
            label="All"
            active={selectedCategory === 'all'}
            onClick={() => setSelectedCategory('all')}
          />
          {Object.entries(categoryConfig).map(([key, config]) => (
            <FilterTab
              key={key}
              label={config.label}
              icon={config.icon}
              active={selectedCategory === key}
              onClick={() => setSelectedCategory(key as IntegrationCategory)}
            />
          ))}
        </div>
      </Card>

      {/* Connected Integrations */}
      <div>
        <h2 className="text-lg font-semibold text-primary-900 mb-4">Your Integrations</h2>
        <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
          {filteredIntegrations.map((integration) => (
            <IntegrationCard key={integration.id} integration={integration} />
          ))}
        </div>
      </div>

      {/* Available Integrations */}
      <div>
        <h2 className="text-lg font-semibold text-primary-900 mb-4">Available Integrations</h2>
        <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
          {availableIntegrations
            .filter((i) => selectedCategory === 'all' || i.category === selectedCategory)
            .map((integration) => (
              <AvailableIntegrationCard key={integration.id} integration={integration} />
            ))}
        </div>
      </div>
    </div>
  )
}

function FilterTab({
  label,
  icon,
  active,
  onClick,
}: {
  label: string
  icon?: React.ReactNode
  active: boolean
  onClick: () => void
}) {
  return (
    <button
      onClick={onClick}
      className={cn(
        'flex items-center gap-2 px-3 py-1.5 rounded-lg text-sm font-medium transition-colors',
        active
          ? 'bg-primary-900 text-white'
          : 'text-primary-600 hover:bg-primary-100'
      )}
    >
      {icon}
      {label}
    </button>
  )
}

function IntegrationCard({ integration }: { integration: Integration }) {
  const status = statusConfig[integration.status]
  const category = categoryConfig[integration.category]

  return (
    <Card className={cn(integration.status === 'error' && 'border-red-200')}>
      <CardContent className="p-5">
        <div className="flex items-start justify-between">
          <div className="flex items-center gap-3">
            <div className="flex h-12 w-12 items-center justify-center rounded-lg bg-primary-100">
              <IntegrationIcon name={integration.icon} />
            </div>
            <div>
              <p className="font-semibold text-primary-900">{integration.name}</p>
              <div className="flex items-center gap-2 mt-0.5">
                {category.icon}
                <span className="text-xs text-primary-500">{category.label}</span>
              </div>
            </div>
          </div>
          <span className={cn('inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-xs font-medium', status.className)}>
            {status.icon}
            {status.label}
          </span>
        </div>

        <p className="mt-3 text-sm text-primary-600">{integration.description}</p>

        {integration.config && (
          <div className="mt-3 rounded-lg bg-primary-50 p-2">
            {Object.entries(integration.config).map(([key, value]) => (
              <p key={key} className="text-xs text-primary-600">
                <span className="font-medium">{key}:</span> {value}
              </p>
            ))}
          </div>
        )}

        {integration.last_sync && (
          <p className="mt-3 text-xs text-primary-400">
            Last sync: {new Date(integration.last_sync).toLocaleString()}
          </p>
        )}

        {integration.status === 'error' && (
          <div className="mt-3 flex items-center gap-2 text-sm text-red-600">
            <AlertTriangle className="h-4 w-4" />
            Connection failed. Check credentials.
          </div>
        )}

        <div className="mt-4 flex gap-2">
          <Button variant="outline" size="sm" className="flex-1">
            <Settings className="mr-2 h-4 w-4" />
            Configure
          </Button>
          {integration.status === 'connected' && (
            <Button variant="outline" size="sm">
              <RefreshCw className="h-4 w-4" />
            </Button>
          )}
        </div>
      </CardContent>
    </Card>
  )
}

function AvailableIntegrationCard({
  integration,
}: {
  integration: Omit<Integration, 'status' | 'last_sync' | 'config'>
}) {
  const category = categoryConfig[integration.category]

  return (
    <Card className="border-dashed">
      <CardContent className="p-5">
        <div className="flex items-start gap-3">
          <div className="flex h-12 w-12 items-center justify-center rounded-lg bg-primary-50">
            <IntegrationIcon name={integration.icon} />
          </div>
          <div className="flex-1">
            <p className="font-semibold text-primary-900">{integration.name}</p>
            <div className="flex items-center gap-2 mt-0.5">
              {category.icon}
              <span className="text-xs text-primary-500">{category.label}</span>
            </div>
          </div>
        </div>

        <p className="mt-3 text-sm text-primary-600">{integration.description}</p>

        <div className="mt-3 flex flex-wrap gap-1">
          {integration.features.map((feature) => (
            <span
              key={feature}
              className="rounded-full bg-primary-100 px-2 py-0.5 text-xs text-primary-600"
            >
              {feature}
            </span>
          ))}
        </div>

        <div className="mt-4">
          <Button variant="outline" size="sm" className="w-full">
            <Plus className="mr-2 h-4 w-4" />
            Connect
          </Button>
        </div>
      </CardContent>
    </Card>
  )
}

function IntegrationIcon({ name }: { name: string }) {
  // In a real app, these would be actual brand icons
  const iconMap: Record<string, string> = {
    aws: 'S3',
    sap: 'SAP',
    okta: 'OK',
    netsuite: 'NS',
    gcs: 'GCS',
    snowflake: 'SF',
    oracle: 'ORC',
    azure: 'AD',
    tableau: 'TBL',
  }

  return (
    <span className="text-sm font-bold text-primary-600">
      {iconMap[name] || name.substring(0, 2).toUpperCase()}
    </span>
  )
}

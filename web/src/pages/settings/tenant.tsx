import { useState } from 'react'
import {
  Building2,
  Save,
  Shield,
  Database,
  Zap,
  Clock,
  HardDrive,
  FileText,
  Users,
  AlertTriangle,
} from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { cn } from '@/lib/utils'
import type { TenantConfig, TenantStatus } from '@/types/api'

// Mock data matching backend TenantConfig type
const mockTenantConfig: TenantConfig = {
  tenant_id: 'tenant-1',
  name: 'Acme Corporation',
  status: 'active',
  created_at: '2023-01-15T10:00:00Z',
  updated_at: '2024-02-01T14:30:00Z',
  retention_policies: {
    invoices: {
      hot_retention_days: 60,
      warm_retention_days: 365,
      archive_retention_days: 2555,
      hard_delete: false,
    },
    contracts: {
      hot_retention_days: 90,
      warm_retention_days: 730,
      archive_retention_days: 3650,
      hard_delete: false,
    },
  },
  rate_limits: {
    requests_per_minute: 1000,
    uploads_per_hour: 500,
    max_concurrent_jobs: 10,
    max_batch_size: 100,
  },
  quotas: {
    max_storage_bytes: 10737418240, // 10 GB
    max_documents: 100000,
    max_rules: 1000,
    max_users: 50,
  },
  features: {
    ml_anomaly_detection: true,
    statistical_baselines: true,
    real_time_notifications: true,
    api_webhooks: false,
    custom_rules: true,
    export_enabled: true,
    zip_queries: false,
    contract_matching: true,
  },
  metadata: {
    industry: 'Manufacturing',
    timezone: 'America/New_York',
  },
}

// Current usage stats
const currentUsage = {
  storage_bytes: 4294967296, // 4 GB
  documents: 12847,
  rules: 156,
  users: 6,
}

export function TenantSettingsPage() {
  const [config, setConfig] = useState(mockTenantConfig)
  const [hasChanges, setHasChanges] = useState(false)

  const formatBytes = (bytes: number) => {
    const gb = bytes / (1024 * 1024 * 1024)
    return gb >= 1 ? `${gb.toFixed(1)} GB` : `${(bytes / (1024 * 1024)).toFixed(0)} MB`
  }

  const updateFeature = (key: keyof typeof config.features, value: boolean) => {
    setConfig({
      ...config,
      features: { ...config.features, [key]: value },
    })
    setHasChanges(true)
  }

  const statusConfig: Record<TenantStatus, { label: string; className: string }> = {
    active: { label: 'Active', className: 'bg-green-100 text-green-700' },
    trial: { label: 'Trial', className: 'bg-blue-100 text-blue-700' },
    suspended: { label: 'Suspended', className: 'bg-red-100 text-red-700' },
    deprovisioning: { label: 'Deprovisioning', className: 'bg-yellow-100 text-yellow-700' },
    archived: { label: 'Archived', className: 'bg-gray-100 text-gray-700' },
  }

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-primary-900">Tenant Settings</h1>
          <p className="text-sm text-primary-500">
            Configure organization settings and policies
          </p>
        </div>
        <Button disabled={!hasChanges}>
          <Save className="mr-2 h-4 w-4" />
          Save Changes
        </Button>
      </div>

      {/* Organization Info */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Building2 className="h-5 w-5" />
            Organization
          </CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="grid gap-4 sm:grid-cols-2">
            <div>
              <label className="text-sm font-medium text-primary-700">Organization Name</label>
              <Input
                value={config.name}
                onChange={(e) => {
                  setConfig({ ...config, name: e.target.value })
                  setHasChanges(true)
                }}
                className="mt-1"
              />
            </div>
            <div>
              <label className="text-sm font-medium text-primary-700">Status</label>
              <div className="mt-1 flex items-center gap-2">
                <span
                  className={cn(
                    'rounded-full px-3 py-1 text-sm font-medium',
                    statusConfig[config.status].className
                  )}
                >
                  {statusConfig[config.status].label}
                </span>
                <span className="text-sm text-primary-500">
                  Since {new Date(config.created_at).toLocaleDateString()}
                </span>
              </div>
            </div>
          </div>
        </CardContent>
      </Card>

      {/* Usage & Quotas */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Database className="h-5 w-5" />
            Usage & Quotas
          </CardTitle>
        </CardHeader>
        <CardContent>
          <div className="grid gap-6 sm:grid-cols-2 lg:grid-cols-4">
            <UsageCard
              icon={<HardDrive className="h-5 w-5" />}
              title="Storage"
              current={formatBytes(currentUsage.storage_bytes)}
              max={formatBytes(config.quotas.max_storage_bytes)}
              percentage={(currentUsage.storage_bytes / config.quotas.max_storage_bytes) * 100}
            />
            <UsageCard
              icon={<FileText className="h-5 w-5" />}
              title="Documents"
              current={currentUsage.documents.toLocaleString()}
              max={config.quotas.max_documents.toLocaleString()}
              percentage={(currentUsage.documents / config.quotas.max_documents) * 100}
            />
            <UsageCard
              icon={<Zap className="h-5 w-5" />}
              title="Rules"
              current={currentUsage.rules.toString()}
              max={config.quotas.max_rules.toString()}
              percentage={(currentUsage.rules / config.quotas.max_rules) * 100}
            />
            <UsageCard
              icon={<Users className="h-5 w-5" />}
              title="Users"
              current={currentUsage.users.toString()}
              max={config.quotas.max_users.toString()}
              percentage={(currentUsage.users / config.quotas.max_users) * 100}
            />
          </div>
        </CardContent>
      </Card>

      {/* Rate Limits */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Clock className="h-5 w-5" />
            Rate Limits
          </CardTitle>
        </CardHeader>
        <CardContent>
          <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
            <RateLimitItem
              label="API Requests"
              value={config.rate_limits.requests_per_minute}
              unit="/min"
            />
            <RateLimitItem
              label="Uploads"
              value={config.rate_limits.uploads_per_hour}
              unit="/hour"
            />
            <RateLimitItem
              label="Concurrent Jobs"
              value={config.rate_limits.max_concurrent_jobs}
              unit="max"
            />
            <RateLimitItem
              label="Batch Size"
              value={config.rate_limits.max_batch_size}
              unit="docs"
            />
          </div>
        </CardContent>
      </Card>

      {/* Retention Policies */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Shield className="h-5 w-5" />
            Data Retention Policies
          </CardTitle>
        </CardHeader>
        <CardContent>
          <div className="space-y-6">
            {Object.entries(config.retention_policies).map(([docType, policy]) => (
              <div key={docType} className="rounded-lg border border-primary-100 p-4">
                <h4 className="font-medium text-primary-900 capitalize mb-4">{docType}</h4>
                <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
                  <div>
                    <label className="text-xs text-primary-500">Hot Storage</label>
                    <p className="text-sm font-medium text-primary-700">{policy.hot_retention_days} days</p>
                  </div>
                  <div>
                    <label className="text-xs text-primary-500">Warm Storage</label>
                    <p className="text-sm font-medium text-primary-700">{policy.warm_retention_days} days</p>
                  </div>
                  <div>
                    <label className="text-xs text-primary-500">Archive</label>
                    <p className="text-sm font-medium text-primary-700">{policy.archive_retention_days} days</p>
                  </div>
                  <div>
                    <label className="text-xs text-primary-500">Hard Delete</label>
                    <p className="text-sm font-medium text-primary-700">{policy.hard_delete ? 'Yes' : 'No'}</p>
                  </div>
                </div>
              </div>
            ))}
          </div>
        </CardContent>
      </Card>

      {/* Feature Flags */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Zap className="h-5 w-5" />
            Features
          </CardTitle>
        </CardHeader>
        <CardContent>
          <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
            <FeatureToggle
              label="ML Anomaly Detection"
              description="AI-powered anomaly detection"
              enabled={config.features.ml_anomaly_detection}
              onChange={(v) => updateFeature('ml_anomaly_detection', v)}
            />
            <FeatureToggle
              label="Statistical Baselines"
              description="Vendor spending baselines"
              enabled={config.features.statistical_baselines}
              onChange={(v) => updateFeature('statistical_baselines', v)}
            />
            <FeatureToggle
              label="Real-time Notifications"
              description="Live alerts and updates"
              enabled={config.features.real_time_notifications}
              onChange={(v) => updateFeature('real_time_notifications', v)}
            />
            <FeatureToggle
              label="API Webhooks"
              description="External integrations"
              enabled={config.features.api_webhooks}
              onChange={(v) => updateFeature('api_webhooks', v)}
            />
            <FeatureToggle
              label="Custom Rules"
              description="User-defined validation rules"
              enabled={config.features.custom_rules}
              onChange={(v) => updateFeature('custom_rules', v)}
            />
            <FeatureToggle
              label="Export"
              description="Data export functionality"
              enabled={config.features.export_enabled}
              onChange={(v) => updateFeature('export_enabled', v)}
            />
            <FeatureToggle
              label="Contract Matching"
              description="Invoice-contract matching"
              enabled={config.features.contract_matching}
              onChange={(v) => updateFeature('contract_matching', v)}
            />
            <FeatureToggle
              label="ZIP Queries"
              description="Advanced SQL queries"
              enabled={config.features.zip_queries}
              onChange={(v) => updateFeature('zip_queries', v)}
            />
          </div>
        </CardContent>
      </Card>

      {/* Danger Zone */}
      <Card className="border-red-200">
        <CardHeader>
          <CardTitle className="flex items-center gap-2 text-red-700">
            <AlertTriangle className="h-5 w-5" />
            Danger Zone
          </CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="flex items-center justify-between rounded-lg border border-red-200 p-4">
            <div>
              <p className="font-medium text-primary-900">Export All Data</p>
              <p className="text-sm text-primary-500">Download a complete export of all tenant data</p>
            </div>
            <Button variant="outline" className="border-red-200 text-red-600 hover:bg-red-50">
              Export Data
            </Button>
          </div>
          <div className="flex items-center justify-between rounded-lg border border-red-200 p-4">
            <div>
              <p className="font-medium text-primary-900">Delete Tenant</p>
              <p className="text-sm text-primary-500">Permanently delete this tenant and all data</p>
            </div>
            <Button variant="outline" className="border-red-200 text-red-600 hover:bg-red-50">
              Delete Tenant
            </Button>
          </div>
        </CardContent>
      </Card>
    </div>
  )
}

function UsageCard({
  icon,
  title,
  current,
  max,
  percentage,
}: {
  icon: React.ReactNode
  title: string
  current: string
  max: string
  percentage: number
}) {
  const isWarning = percentage > 80
  const isCritical = percentage > 95

  return (
    <div className="rounded-lg border border-primary-100 p-4">
      <div className="flex items-center gap-2 text-primary-600">
        {icon}
        <span className="text-sm font-medium">{title}</span>
      </div>
      <div className="mt-2">
        <span className="text-2xl font-bold text-primary-900">{current}</span>
        <span className="text-sm text-primary-500"> / {max}</span>
      </div>
      <div className="mt-2">
        <div className="h-2 w-full rounded-full bg-primary-100">
          <div
            className={cn(
              'h-2 rounded-full transition-all',
              isCritical ? 'bg-red-500' : isWarning ? 'bg-yellow-500' : 'bg-green-500'
            )}
            style={{ width: `${Math.min(percentage, 100)}%` }}
          />
        </div>
        <p className="mt-1 text-xs text-primary-400">{percentage.toFixed(1)}% used</p>
      </div>
    </div>
  )
}

function RateLimitItem({
  label,
  value,
  unit,
}: {
  label: string
  value: number
  unit: string
}) {
  return (
    <div className="rounded-lg border border-primary-100 p-4">
      <p className="text-sm text-primary-500">{label}</p>
      <p className="mt-1">
        <span className="text-2xl font-bold text-primary-900">{value.toLocaleString()}</span>
        <span className="text-sm text-primary-500 ml-1">{unit}</span>
      </p>
    </div>
  )
}

function FeatureToggle({
  label,
  description,
  enabled,
  onChange,
}: {
  label: string
  description: string
  enabled: boolean
  onChange: (value: boolean) => void
}) {
  return (
    <div className="flex items-center justify-between rounded-lg border border-primary-100 p-4">
      <div>
        <p className="font-medium text-primary-900">{label}</p>
        <p className="text-sm text-primary-500">{description}</p>
      </div>
      <button
        onClick={() => onChange(!enabled)}
        className={cn(
          'relative h-6 w-11 rounded-full transition-colors',
          enabled ? 'bg-primary-600' : 'bg-primary-200'
        )}
      >
        <span
          className={cn(
            'absolute top-0.5 h-5 w-5 rounded-full bg-white shadow transition-transform',
            enabled ? 'left-5' : 'left-0.5'
          )}
        />
      </button>
    </div>
  )
}

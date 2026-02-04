import { useState } from 'react'
import {
  Mail,
  MessageSquare,
  Webhook,
  Save,
  Plus,
  Trash2,
  TestTube,
  CheckCircle,
  AlertTriangle,
  XCircle,
  Clock,
} from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { cn } from '@/lib/utils'

type NotificationChannel = 'email' | 'slack' | 'webhook' | 'in_app'
type NotificationEvent = 'anomaly_detected' | 'document_processed' | 'rule_failure' | 'baseline_updated' | 'system_alert'
type Severity = 'all' | 'critical' | 'high' | 'medium' | 'low'

interface NotificationPreference {
  event: NotificationEvent
  channels: NotificationChannel[]
  severity_threshold: Severity
  enabled: boolean
}

interface WebhookConfig {
  id: string
  name: string
  url: string
  secret?: string
  events: NotificationEvent[]
  status: 'active' | 'inactive' | 'error'
  last_triggered?: string
}

// Mock data
const mockPreferences: NotificationPreference[] = [
  { event: 'anomaly_detected', channels: ['email', 'slack', 'in_app'], severity_threshold: 'medium', enabled: true },
  { event: 'document_processed', channels: ['in_app'], severity_threshold: 'all', enabled: true },
  { event: 'rule_failure', channels: ['email', 'slack'], severity_threshold: 'all', enabled: true },
  { event: 'baseline_updated', channels: ['in_app'], severity_threshold: 'all', enabled: false },
  { event: 'system_alert', channels: ['email', 'slack', 'in_app'], severity_threshold: 'all', enabled: true },
]

const mockWebhooks: WebhookConfig[] = [
  {
    id: 'wh-001',
    name: 'PagerDuty Integration',
    url: 'https://events.pagerduty.com/integration/abc123/enqueue',
    events: ['anomaly_detected', 'system_alert'],
    status: 'active',
    last_triggered: '2024-02-04T10:30:00Z',
  },
  {
    id: 'wh-002',
    name: 'Custom Analytics',
    url: 'https://analytics.company.com/api/webhooks/fen',
    events: ['document_processed', 'anomaly_detected'],
    status: 'error',
    last_triggered: '2024-02-03T15:45:00Z',
  },
]

const eventLabels: Record<NotificationEvent, string> = {
  anomaly_detected: 'Anomaly Detected',
  document_processed: 'Document Processed',
  rule_failure: 'Rule Failure',
  baseline_updated: 'Baseline Updated',
  system_alert: 'System Alert',
}

export function NotificationsPage() {
  const [preferences, setPreferences] = useState(mockPreferences)
  const [webhooks] = useState(mockWebhooks)
  const [hasChanges, setHasChanges] = useState(false)
  const [emailConfig, setEmailConfig] = useState({
    digest_enabled: true,
    digest_frequency: 'daily',
    email_address: 'alerts@company.com',
  })
  const [slackConfig, setSlackConfig] = useState({
    enabled: true,
    channel: '#fen-alerts',
    webhook_url: 'https://hooks.slack.com/services/T00000000/B00000000/XXXX',
  })

  const toggleChannel = (eventIndex: number, channel: NotificationChannel) => {
    const newPrefs = [...preferences]
    const channels = newPrefs[eventIndex].channels
    if (channels.includes(channel)) {
      newPrefs[eventIndex].channels = channels.filter((c) => c !== channel)
    } else {
      newPrefs[eventIndex].channels = [...channels, channel]
    }
    setPreferences(newPrefs)
    setHasChanges(true)
  }

  const toggleEnabled = (eventIndex: number) => {
    const newPrefs = [...preferences]
    newPrefs[eventIndex].enabled = !newPrefs[eventIndex].enabled
    setPreferences(newPrefs)
    setHasChanges(true)
  }

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-primary-900">Notifications</h1>
          <p className="text-sm text-primary-500">
            Configure how and when you receive alerts
          </p>
        </div>
        <Button disabled={!hasChanges}>
          <Save className="mr-2 h-4 w-4" />
          Save Changes
        </Button>
      </div>

      {/* Channel Configuration */}
      <div className="grid gap-6 lg:grid-cols-2">
        {/* Email Settings */}
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <Mail className="h-5 w-5" />
              Email Notifications
            </CardTitle>
          </CardHeader>
          <CardContent className="space-y-4">
            <div>
              <label className="text-sm font-medium text-primary-700">Email Address</label>
              <Input
                value={emailConfig.email_address}
                onChange={(e) => {
                  setEmailConfig({ ...emailConfig, email_address: e.target.value })
                  setHasChanges(true)
                }}
                className="mt-1"
                placeholder="alerts@company.com"
              />
            </div>
            <div className="flex items-center justify-between">
              <div>
                <p className="font-medium text-primary-900">Daily Digest</p>
                <p className="text-sm text-primary-500">Receive a summary of all alerts</p>
              </div>
              <ToggleSwitch
                enabled={emailConfig.digest_enabled}
                onChange={(v) => {
                  setEmailConfig({ ...emailConfig, digest_enabled: v })
                  setHasChanges(true)
                }}
              />
            </div>
            <div>
              <label className="text-sm font-medium text-primary-700">Digest Frequency</label>
              <select
                value={emailConfig.digest_frequency}
                onChange={(e) => {
                  setEmailConfig({ ...emailConfig, digest_frequency: e.target.value })
                  setHasChanges(true)
                }}
                className="mt-1 w-full h-10 rounded-lg border border-primary-200 bg-white px-3 text-sm"
                disabled={!emailConfig.digest_enabled}
              >
                <option value="hourly">Hourly</option>
                <option value="daily">Daily</option>
                <option value="weekly">Weekly</option>
              </select>
            </div>
          </CardContent>
        </Card>

        {/* Slack Settings */}
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <MessageSquare className="h-5 w-5" />
              Slack Integration
            </CardTitle>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="flex items-center justify-between">
              <div>
                <p className="font-medium text-primary-900">Enable Slack</p>
                <p className="text-sm text-primary-500">Send alerts to Slack channel</p>
              </div>
              <ToggleSwitch
                enabled={slackConfig.enabled}
                onChange={(v) => {
                  setSlackConfig({ ...slackConfig, enabled: v })
                  setHasChanges(true)
                }}
              />
            </div>
            <div>
              <label className="text-sm font-medium text-primary-700">Channel</label>
              <Input
                value={slackConfig.channel}
                onChange={(e) => {
                  setSlackConfig({ ...slackConfig, channel: e.target.value })
                  setHasChanges(true)
                }}
                className="mt-1"
                placeholder="#alerts"
                disabled={!slackConfig.enabled}
              />
            </div>
            <div>
              <label className="text-sm font-medium text-primary-700">Webhook URL</label>
              <Input
                value={slackConfig.webhook_url}
                onChange={(e) => {
                  setSlackConfig({ ...slackConfig, webhook_url: e.target.value })
                  setHasChanges(true)
                }}
                className="mt-1"
                placeholder="https://hooks.slack.com/..."
                type="password"
                disabled={!slackConfig.enabled}
              />
            </div>
            <Button variant="outline" size="sm" disabled={!slackConfig.enabled}>
              <TestTube className="mr-2 h-4 w-4" />
              Send Test Message
            </Button>
          </CardContent>
        </Card>
      </div>

      {/* Event Preferences */}
      <Card>
        <CardHeader>
          <CardTitle>Notification Preferences</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="overflow-x-auto">
            <table className="w-full">
              <thead>
                <tr className="border-b border-primary-200 bg-primary-50">
                  <th className="px-4 py-3 text-left text-sm font-medium text-primary-700">Event</th>
                  <th className="px-4 py-3 text-center text-sm font-medium text-primary-700">Email</th>
                  <th className="px-4 py-3 text-center text-sm font-medium text-primary-700">Slack</th>
                  <th className="px-4 py-3 text-center text-sm font-medium text-primary-700">In-App</th>
                  <th className="px-4 py-3 text-center text-sm font-medium text-primary-700">Severity</th>
                  <th className="px-4 py-3 text-center text-sm font-medium text-primary-700">Enabled</th>
                </tr>
              </thead>
              <tbody>
                {preferences.map((pref, index) => (
                  <tr key={pref.event} className="border-b border-primary-100">
                    <td className="px-4 py-3">
                      <span className="font-medium text-primary-900">{eventLabels[pref.event]}</span>
                    </td>
                    <td className="px-4 py-3 text-center">
                      <ChannelCheckbox
                        checked={pref.channels.includes('email')}
                        onChange={() => toggleChannel(index, 'email')}
                        disabled={!pref.enabled}
                      />
                    </td>
                    <td className="px-4 py-3 text-center">
                      <ChannelCheckbox
                        checked={pref.channels.includes('slack')}
                        onChange={() => toggleChannel(index, 'slack')}
                        disabled={!pref.enabled}
                      />
                    </td>
                    <td className="px-4 py-3 text-center">
                      <ChannelCheckbox
                        checked={pref.channels.includes('in_app')}
                        onChange={() => toggleChannel(index, 'in_app')}
                        disabled={!pref.enabled}
                      />
                    </td>
                    <td className="px-4 py-3 text-center">
                      <select
                        value={pref.severity_threshold}
                        onChange={(e) => {
                          const newPrefs = [...preferences]
                          newPrefs[index].severity_threshold = e.target.value as Severity
                          setPreferences(newPrefs)
                          setHasChanges(true)
                        }}
                        className="h-8 rounded border border-primary-200 bg-white px-2 text-xs"
                        disabled={!pref.enabled}
                      >
                        <option value="all">All</option>
                        <option value="low">Low+</option>
                        <option value="medium">Medium+</option>
                        <option value="high">High+</option>
                        <option value="critical">Critical</option>
                      </select>
                    </td>
                    <td className="px-4 py-3 text-center">
                      <ToggleSwitch
                        enabled={pref.enabled}
                        onChange={() => toggleEnabled(index)}
                      />
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </CardContent>
      </Card>

      {/* Webhooks */}
      <Card>
        <CardHeader>
          <div className="flex items-center justify-between">
            <CardTitle className="flex items-center gap-2">
              <Webhook className="h-5 w-5" />
              Webhooks
            </CardTitle>
            <Button variant="outline" size="sm">
              <Plus className="mr-2 h-4 w-4" />
              Add Webhook
            </Button>
          </div>
        </CardHeader>
        <CardContent>
          {webhooks.length === 0 ? (
            <p className="text-center text-sm text-primary-500 py-8">
              No webhooks configured. Add one to receive notifications via HTTP.
            </p>
          ) : (
            <div className="space-y-4">
              {webhooks.map((webhook) => (
                <WebhookCard key={webhook.id} webhook={webhook} />
              ))}
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  )
}

function ToggleSwitch({
  enabled,
  onChange,
}: {
  enabled: boolean
  onChange: (value: boolean) => void
}) {
  return (
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
  )
}

function ChannelCheckbox({
  checked,
  onChange,
  disabled,
}: {
  checked: boolean
  onChange: () => void
  disabled?: boolean
}) {
  return (
    <button
      onClick={onChange}
      disabled={disabled}
      className={cn(
        'h-5 w-5 rounded border-2 flex items-center justify-center transition-colors',
        checked ? 'bg-primary-600 border-primary-600' : 'bg-white border-primary-300',
        disabled && 'opacity-50 cursor-not-allowed'
      )}
    >
      {checked && <CheckCircle className="h-3 w-3 text-white" />}
    </button>
  )
}

function WebhookCard({ webhook }: { webhook: WebhookConfig }) {
  const statusConfig: Record<string, { icon: React.ReactNode; className: string }> = {
    active: { icon: <CheckCircle className="h-4 w-4" />, className: 'text-green-600' },
    inactive: { icon: <Clock className="h-4 w-4" />, className: 'text-primary-400' },
    error: { icon: <XCircle className="h-4 w-4" />, className: 'text-red-600' },
  }

  const status = statusConfig[webhook.status]

  return (
    <div className="rounded-lg border border-primary-200 p-4">
      <div className="flex items-start justify-between">
        <div className="flex items-center gap-3">
          <div className={cn('p-2 rounded-lg', webhook.status === 'error' ? 'bg-red-100' : 'bg-primary-100')}>
            <Webhook className="h-5 w-5 text-primary-600" />
          </div>
          <div>
            <div className="flex items-center gap-2">
              <p className="font-medium text-primary-900">{webhook.name}</p>
              <span className={cn('flex items-center gap-1', status.className)}>
                {status.icon}
              </span>
            </div>
            <p className="text-sm text-primary-500 font-mono truncate max-w-md">{webhook.url}</p>
          </div>
        </div>
        <div className="flex items-center gap-2">
          <Button variant="outline" size="sm">
            <TestTube className="mr-2 h-4 w-4" />
            Test
          </Button>
          <Button variant="ghost" size="icon" className="h-8 w-8 text-red-600">
            <Trash2 className="h-4 w-4" />
          </Button>
        </div>
      </div>

      <div className="mt-3 flex items-center gap-4">
        <div className="flex flex-wrap gap-1">
          {webhook.events.map((event) => (
            <span
              key={event}
              className="rounded-full bg-primary-100 px-2 py-0.5 text-xs font-medium text-primary-700"
            >
              {eventLabels[event]}
            </span>
          ))}
        </div>
        {webhook.last_triggered && (
          <span className="text-xs text-primary-400">
            Last triggered: {new Date(webhook.last_triggered).toLocaleString()}
          </span>
        )}
      </div>

      {webhook.status === 'error' && (
        <div className="mt-3 flex items-center gap-2 rounded-lg bg-red-50 px-3 py-2 text-sm text-red-700">
          <AlertTriangle className="h-4 w-4" />
          Last request failed with HTTP 500. Check endpoint availability.
        </div>
      )}
    </div>
  )
}

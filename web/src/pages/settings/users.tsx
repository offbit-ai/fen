import { useState } from 'react'
import {
  Users,
  Search,
  Plus,
  MoreHorizontal,
  Mail,
  Shield,
  CheckCircle,
  Clock,
  XCircle,
  Lock,
  ChevronLeft,
  ChevronRight,
} from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Card, CardContent } from '@/components/ui/card'
import { cn } from '@/lib/utils'
import type { User, UserStatus } from '@/types/api'

// Mock data matching backend User type
const mockUsers: User[] = [
  {
    id: 'usr-001',
    tenant_id: 'tenant-1',
    email: 'sarah.anderson@company.com',
    display_name: 'Sarah Anderson',
    status: 'active',
    roles: ['tenant_admin'],
    auth_provider: 'local',
    created_at: '2023-06-15T10:00:00Z',
    updated_at: '2024-02-01T14:30:00Z',
    last_login_at: '2024-02-04T09:15:00Z',
  },
  {
    id: 'usr-002',
    tenant_id: 'tenant-1',
    email: 'john.smith@company.com',
    display_name: 'John Smith',
    status: 'active',
    roles: ['analyst'],
    auth_provider: 'oidc',
    created_at: '2023-08-20T10:00:00Z',
    updated_at: '2024-01-28T11:00:00Z',
    last_login_at: '2024-02-03T16:45:00Z',
  },
  {
    id: 'usr-003',
    tenant_id: 'tenant-1',
    email: 'emily.chen@company.com',
    display_name: 'Emily Chen',
    status: 'active',
    roles: ['analyst', 'rule_manager'],
    auth_provider: 'local',
    created_at: '2023-10-05T10:00:00Z',
    updated_at: '2024-02-02T09:00:00Z',
    last_login_at: '2024-02-04T08:30:00Z',
  },
  {
    id: 'usr-004',
    tenant_id: 'tenant-1',
    email: 'michael.brown@company.com',
    display_name: 'Michael Brown',
    status: 'pending_verification',
    roles: ['viewer'],
    auth_provider: 'local',
    created_at: '2024-02-01T10:00:00Z',
    updated_at: '2024-02-01T10:00:00Z',
  },
  {
    id: 'usr-005',
    tenant_id: 'tenant-1',
    email: 'lisa.wong@company.com',
    display_name: 'Lisa Wong',
    status: 'suspended',
    roles: ['analyst'],
    auth_provider: 'oidc',
    created_at: '2023-07-10T10:00:00Z',
    updated_at: '2024-01-15T10:00:00Z',
    last_login_at: '2024-01-10T12:00:00Z',
  },
  {
    id: 'usr-006',
    tenant_id: 'tenant-1',
    email: 'david.kim@company.com',
    display_name: 'David Kim',
    status: 'locked',
    roles: ['viewer'],
    auth_provider: 'local',
    created_at: '2023-11-20T10:00:00Z',
    updated_at: '2024-02-03T08:00:00Z',
    last_login_at: '2024-02-02T14:00:00Z',
  },
]

type StatusFilter = 'all' | UserStatus

export function UsersPage() {
  const [searchQuery, setSearchQuery] = useState('')
  const [statusFilter, setStatusFilter] = useState<StatusFilter>('all')
  const [selectedUsers, setSelectedUsers] = useState<Set<string>>(new Set())

  const filteredUsers = mockUsers.filter((u) => {
    if (statusFilter !== 'all' && u.status !== statusFilter) return false
    if (searchQuery) {
      const query = searchQuery.toLowerCase()
      return (
        u.display_name.toLowerCase().includes(query) ||
        u.email.toLowerCase().includes(query)
      )
    }
    return true
  })

  const toggleUser = (id: string) => {
    const newSelected = new Set(selectedUsers)
    if (newSelected.has(id)) {
      newSelected.delete(id)
    } else {
      newSelected.add(id)
    }
    setSelectedUsers(newSelected)
  }

  const toggleAll = () => {
    if (selectedUsers.size === filteredUsers.length) {
      setSelectedUsers(new Set())
    } else {
      setSelectedUsers(new Set(filteredUsers.map((u) => u.id)))
    }
  }

  // Stats
  const stats = {
    total: mockUsers.length,
    active: mockUsers.filter((u) => u.status === 'active').length,
    pending: mockUsers.filter((u) => u.status === 'pending_verification').length,
    suspended: mockUsers.filter((u) => u.status === 'suspended' || u.status === 'locked').length,
  }

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-primary-900">Users</h1>
          <p className="text-sm text-primary-500">
            Manage user accounts and permissions
          </p>
        </div>
        <Button>
          <Plus className="mr-2 h-4 w-4" />
          Invite User
        </Button>
      </div>

      {/* Stats Cards */}
      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
        <StatCard
          title="Total Users"
          value={stats.total}
          icon={<Users className="h-5 w-5" />}
        />
        <StatCard
          title="Active"
          value={stats.active}
          icon={<CheckCircle className="h-5 w-5" />}
          color="green"
        />
        <StatCard
          title="Pending"
          value={stats.pending}
          icon={<Clock className="h-5 w-5" />}
          color="yellow"
        />
        <StatCard
          title="Suspended/Locked"
          value={stats.suspended}
          icon={<Lock className="h-5 w-5" />}
          color="red"
        />
      </div>

      {/* Filters */}
      <Card className="p-4">
        <div className="flex flex-col gap-4 sm:flex-row sm:items-center">
          <div className="relative flex-1 max-w-md">
            <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-primary-400" />
            <Input
              type="search"
              placeholder="Search users..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="pl-10"
            />
          </div>
          <select
            value={statusFilter}
            onChange={(e) => setStatusFilter(e.target.value as StatusFilter)}
            className="h-10 rounded-lg border border-primary-200 bg-white px-3 text-sm"
          >
            <option value="all">All Statuses</option>
            <option value="active">Active</option>
            <option value="pending_verification">Pending</option>
            <option value="suspended">Suspended</option>
            <option value="locked">Locked</option>
          </select>
        </div>
      </Card>

      {/* Bulk Actions */}
      {selectedUsers.size > 0 && (
        <Card className="bg-primary-50">
          <CardContent className="py-3 px-4 flex items-center justify-between">
            <span className="text-sm text-primary-700">
              {selectedUsers.size} user{selectedUsers.size > 1 ? 's' : ''} selected
            </span>
            <div className="flex items-center gap-2">
              <Button variant="outline" size="sm">
                <Mail className="mr-2 h-4 w-4" />
                Send Email
              </Button>
              <Button variant="outline" size="sm">
                Edit Roles
              </Button>
            </div>
          </CardContent>
        </Card>
      )}

      {/* Users Table */}
      <Card>
        <CardContent className="p-0">
          <div className="overflow-x-auto">
            <table className="w-full">
              <thead>
                <tr className="border-b border-primary-200 bg-primary-50">
                  <th className="w-12 px-4 py-3">
                    <input
                      type="checkbox"
                      checked={selectedUsers.size === filteredUsers.length && filteredUsers.length > 0}
                      onChange={toggleAll}
                      className="h-4 w-4 rounded border-primary-300"
                    />
                  </th>
                  <th className="px-4 py-3 text-left text-sm font-medium text-primary-700">
                    User
                  </th>
                  <th className="px-4 py-3 text-left text-sm font-medium text-primary-700">
                    Status
                  </th>
                  <th className="px-4 py-3 text-left text-sm font-medium text-primary-700">
                    Roles
                  </th>
                  <th className="px-4 py-3 text-left text-sm font-medium text-primary-700">
                    Auth Provider
                  </th>
                  <th className="px-4 py-3 text-left text-sm font-medium text-primary-700">
                    Last Login
                  </th>
                  <th className="w-12 px-4 py-3"></th>
                </tr>
              </thead>
              <tbody>
                {filteredUsers.map((user) => (
                  <UserRow
                    key={user.id}
                    user={user}
                    selected={selectedUsers.has(user.id)}
                    onToggle={() => toggleUser(user.id)}
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
          Showing 1-{filteredUsers.length} of {mockUsers.length} users
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

function StatCard({
  title,
  value,
  icon,
  color = 'gray',
}: {
  title: string
  value: number
  icon: React.ReactNode
  color?: 'gray' | 'green' | 'yellow' | 'red'
}) {
  const colorClasses = {
    gray: 'bg-primary-100 text-primary-600',
    green: 'bg-green-100 text-green-600',
    yellow: 'bg-yellow-100 text-yellow-600',
    red: 'bg-red-100 text-red-600',
  }

  return (
    <Card>
      <CardContent className="p-4">
        <div className="flex items-center gap-3">
          <div className={cn('rounded-lg p-2', colorClasses[color])}>{icon}</div>
          <div>
            <p className="text-sm text-primary-500">{title}</p>
            <p className="text-2xl font-bold text-primary-900">{value}</p>
          </div>
        </div>
      </CardContent>
    </Card>
  )
}

interface UserRowProps {
  user: User
  selected: boolean
  onToggle: () => void
}

function UserRow({ user, selected, onToggle }: UserRowProps) {
  const statusConfig: Record<UserStatus, { icon: React.ReactNode; className: string; label: string }> = {
    active: {
      icon: <CheckCircle className="h-4 w-4" />,
      className: 'bg-green-100 text-green-700',
      label: 'Active',
    },
    pending_verification: {
      icon: <Clock className="h-4 w-4" />,
      className: 'bg-yellow-100 text-yellow-700',
      label: 'Pending',
    },
    suspended: {
      icon: <XCircle className="h-4 w-4" />,
      className: 'bg-red-100 text-red-700',
      label: 'Suspended',
    },
    deactivated: {
      icon: <XCircle className="h-4 w-4" />,
      className: 'bg-gray-100 text-gray-700',
      label: 'Deactivated',
    },
    locked: {
      icon: <Lock className="h-4 w-4" />,
      className: 'bg-red-100 text-red-700',
      label: 'Locked',
    },
  }

  const roleLabels: Record<string, string> = {
    tenant_admin: 'Admin',
    analyst: 'Analyst',
    viewer: 'Viewer',
    rule_manager: 'Rule Manager',
    system_admin: 'System Admin',
  }

  const authProviderLabels: Record<string, string> = {
    local: 'Email/Password',
    oidc: 'SSO (OIDC)',
    saml: 'SSO (SAML)',
    api_key: 'API Key',
  }

  const status = statusConfig[user.status]
  const initials = user.display_name
    .split(' ')
    .map((n) => n[0])
    .join('')
    .toUpperCase()
    .slice(0, 2)

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
          <div className="flex h-9 w-9 items-center justify-center rounded-full bg-primary-200 text-sm font-medium text-primary-700">
            {initials}
          </div>
          <div>
            <p className="font-medium text-primary-900">{user.display_name}</p>
            <p className="text-sm text-primary-500">{user.email}</p>
          </div>
        </div>
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
        <div className="flex flex-wrap gap-1">
          {user.roles.map((role) => (
            <span
              key={role}
              className="inline-flex items-center gap-1 rounded-full bg-primary-100 px-2 py-0.5 text-xs font-medium text-primary-700"
            >
              <Shield className="h-3 w-3" />
              {roleLabels[role] || role}
            </span>
          ))}
        </div>
      </td>
      <td className="px-4 py-3 text-sm text-primary-600">
        {authProviderLabels[user.auth_provider] || user.auth_provider}
      </td>
      <td className="px-4 py-3 text-sm text-primary-500">
        {user.last_login_at
          ? new Date(user.last_login_at).toLocaleDateString()
          : 'Never'}
      </td>
      <td className="px-4 py-3">
        <Button variant="ghost" size="icon" className="h-8 w-8">
          <MoreHorizontal className="h-4 w-4" />
        </Button>
      </td>
    </tr>
  )
}

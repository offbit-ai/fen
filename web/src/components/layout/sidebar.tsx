import { Link, useLocation } from 'react-router-dom'
import {
  FileText,
  FileCheck,
  Paperclip,
  Search,
  BarChart3,
  AlertTriangle,
  TrendingDown,
  GitBranch,
  Table,
  FlaskConical,
  History,
  Users,
  Building2,
  Bell,
  Link as LinkIcon,
  ScrollText,
} from 'lucide-react'
import { Logo } from '@/components/shared/logo'
import { cn } from '@/lib/utils'

interface NavItem {
  label: string
  href: string
  icon: React.ReactNode
}

interface NavSection {
  title: string
  items: NavItem[]
}

const navigation: NavSection[] = [
  {
    title: 'Documents',
    items: [
      { label: 'Invoices', href: '/documents/invoices', icon: <FileText className="w-4 h-4" /> },
      { label: 'Contracts', href: '/documents/contracts', icon: <FileCheck className="w-4 h-4" /> },
      { label: 'Attachments', href: '/documents/attachments', icon: <Paperclip className="w-4 h-4" /> },
    ],
  },
  {
    title: 'Analysis',
    items: [
      { label: 'Query Workbench', href: '/analysis/workbench', icon: <Search className="w-4 h-4" /> },
      { label: 'Reports', href: '/analysis/reports', icon: <BarChart3 className="w-4 h-4" /> },
      { label: 'Anomalies', href: '/analysis/anomalies', icon: <AlertTriangle className="w-4 h-4" /> },
      { label: 'Baselines', href: '/analysis/baselines', icon: <TrendingDown className="w-4 h-4" /> },
    ],
  },
  {
    title: 'Rules Engine',
    items: [
      { label: 'Rule Graph', href: '/rules/graph', icon: <GitBranch className="w-4 h-4" /> },
      { label: 'Decision Tables', href: '/rules/tables', icon: <Table className="w-4 h-4" /> },
      { label: 'Rule Testing', href: '/rules/testing', icon: <FlaskConical className="w-4 h-4" /> },
      { label: 'Execution History', href: '/rules/history', icon: <History className="w-4 h-4" /> },
    ],
  },
  {
    title: 'Settings',
    items: [
      { label: 'Users', href: '/settings/users', icon: <Users className="w-4 h-4" /> },
      { label: 'Tenant Settings', href: '/settings/tenant', icon: <Building2 className="w-4 h-4" /> },
      { label: 'Notifications', href: '/settings/notifications', icon: <Bell className="w-4 h-4" /> },
      { label: 'Integrations', href: '/settings/integrations', icon: <LinkIcon className="w-4 h-4" /> },
      { label: 'Audit Log', href: '/settings/audit', icon: <ScrollText className="w-4 h-4" /> },
    ],
  },
]

export function Sidebar() {
  const location = useLocation()

  return (
    <aside className="fixed inset-y-0 left-0 z-50 hidden w-sidebar flex-col border-r border-primary-200 bg-white lg:flex">
      {/* Logo */}
      <div className="flex h-header items-center border-b border-primary-200 px-6">
        <Link to="/dashboard">
          <Logo />
        </Link>
      </div>

      {/* Navigation */}
      <nav className="flex-1 overflow-y-auto p-4 scrollbar-thin">
        {/* Dashboard Link */}
        <Link
          to="/dashboard"
          className={cn(
            'flex items-center gap-3 rounded-lg px-3 py-2 text-sm font-medium transition-colors mb-4',
            location.pathname === '/dashboard'
              ? 'bg-primary-100 text-primary-900'
              : 'text-primary-600 hover:bg-primary-50 hover:text-primary-900'
          )}
        >
          <BarChart3 className="w-4 h-4" />
          Dashboard
        </Link>

        {/* Sections */}
        {navigation.map((section) => (
          <div key={section.title} className="mb-6">
            <h3 className="mb-2 px-3 text-xs font-semibold uppercase tracking-wider text-primary-400">
              {section.title}
            </h3>
            <ul className="space-y-1">
              {section.items.map((item) => (
                <li key={item.href}>
                  <Link
                    to={item.href}
                    className={cn(
                      'flex items-center gap-3 rounded-lg px-3 py-2 text-sm transition-colors',
                      location.pathname === item.href
                        ? 'bg-primary-100 text-primary-900 font-medium'
                        : 'text-primary-600 hover:bg-primary-50 hover:text-primary-900'
                    )}
                  >
                    {item.icon}
                    {item.label}
                  </Link>
                </li>
              ))}
            </ul>
          </div>
        ))}
      </nav>

      {/* User Info */}
      <div className="border-t border-primary-200 p-4">
        <div className="flex items-center gap-3">
          <div className="flex h-9 w-9 items-center justify-center rounded-full bg-primary-200 text-sm font-medium text-primary-700">
            SA
          </div>
          <div className="flex-1 min-w-0">
            <p className="text-sm font-medium text-primary-900 truncate">
              Sarah Anderson
            </p>
            <p className="text-xs text-primary-500 truncate">
              Analyst
            </p>
          </div>
        </div>
      </div>
    </aside>
  )
}

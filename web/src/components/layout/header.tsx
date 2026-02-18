import { useState } from 'react'
import { Link } from 'react-router-dom'
import {
  Bell,
  Search,
  ChevronDown,
  LogOut,
  Settings,
  User,
  Menu,
} from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { useUIStore } from '@/store/ui'

export function Header() {
  const [showUserMenu, setShowUserMenu] = useState(false)
  const toggleMobileSidebar = useUIStore((state) => state.toggleMobileSidebar)

  return (
    <header className="sticky top-0 z-40 flex h-header items-center justify-between border-b border-primary-200 bg-white px-6">
      {/* Left side - Mobile menu + Search */}
      <div className="flex items-center gap-4">
        {/* Mobile menu button */}
        <Button
          variant="ghost"
          size="icon"
          className="lg:hidden"
          onClick={toggleMobileSidebar}
        >
          <Menu className="h-5 w-5" />
        </Button>

        {/* Search */}
        <div className="relative hidden sm:block">
          <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-primary-400" />
          <Input
            type="search"
            placeholder="Search documents, anomalies..."
            className="w-64 pl-10 lg:w-80"
          />
          <kbd className="absolute right-3 top-1/2 -translate-y-1/2 hidden rounded bg-primary-100 px-1.5 py-0.5 text-xs text-primary-500 md:block">
            ⌘K
          </kbd>
        </div>
      </div>

      {/* Right side - Notifications + User */}
      <div className="flex items-center gap-2">
        {/* Notifications */}
        <Button variant="ghost" size="icon" className="relative">
          <Bell className="h-5 w-5" />
          <span className="absolute right-1 top-1 flex h-4 w-4 items-center justify-center rounded-full bg-error text-[10px] font-medium text-white">
            3
          </span>
        </Button>

        {/* User Menu */}
        <div className="relative">
          <button
            onClick={() => setShowUserMenu(!showUserMenu)}
            className="flex items-center gap-2 rounded-lg p-2 hover:bg-primary-50 transition-colors"
          >
            <div className="flex h-8 w-8 items-center justify-center rounded-full bg-primary-200 text-sm font-medium text-primary-700">
              SA
            </div>
            <span className="hidden text-sm font-medium text-primary-700 md:block">
              Sarah
            </span>
            <ChevronDown className="h-4 w-4 text-primary-400" />
          </button>

          {/* Dropdown */}
          {showUserMenu && (
            <>
              <div
                className="fixed inset-0 z-40"
                onClick={() => setShowUserMenu(false)}
              />
              <div className="absolute right-0 top-full z-50 mt-2 w-56 rounded-lg border border-primary-200 bg-white py-1 shadow-lg">
                <div className="border-b border-primary-100 px-4 py-3">
                  <p className="text-sm font-medium text-primary-900">
                    Sarah Anderson
                  </p>
                  <p className="text-xs text-primary-500">
                    sarah@company.com
                  </p>
                </div>
                <div className="py-1">
                  <DropdownItem
                    href="/app/settings/tenant"
                    icon={<User className="h-4 w-4" />}
                    label="Profile"
                  />
                  <DropdownItem
                    href="/app/settings/tenant"
                    icon={<Settings className="h-4 w-4" />}
                    label="Settings"
                  />
                </div>
                <div className="border-t border-primary-100 py-1">
                  <button
                    onClick={() => {
                      // TODO: Implement logout
                      window.location.href = '/login'
                    }}
                    className="flex w-full items-center gap-3 px-4 py-2 text-sm text-primary-600 hover:bg-primary-50"
                  >
                    <LogOut className="h-4 w-4" />
                    Sign out
                  </button>
                </div>
              </div>
            </>
          )}
        </div>
      </div>
    </header>
  )
}

interface DropdownItemProps {
  href: string
  icon: React.ReactNode
  label: string
}

function DropdownItem({ href, icon, label }: DropdownItemProps) {
  return (
    <Link
      to={href}
      className="flex items-center gap-3 px-4 py-2 text-sm text-primary-600 hover:bg-primary-50"
    >
      {icon}
      {label}
    </Link>
  )
}

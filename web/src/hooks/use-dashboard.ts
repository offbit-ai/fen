import { useApi } from './use-api'
import { dashboardApi } from '@/api/endpoints/dashboard'
import type { DashboardStats, RuleStatus } from '@/types/api'

export function useDashboardStats() {
  return useApi<DashboardStats>(
    () => dashboardApi.getDashboardStats(),
    []
  )
}

export function useRuleStatus() {
  return useApi<RuleStatus>(
    () => dashboardApi.getRuleStatus(),
    []
  )
}

export function useHealthCheck() {
  return useApi<{ status: string; version?: string }>(
    () => dashboardApi.getHealth(),
    []
  )
}

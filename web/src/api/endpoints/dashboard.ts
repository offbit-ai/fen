import { getApiClient } from '../client'
import type { DashboardStats, StatsResponse, RuleStatus } from '@/types/api'

export const dashboardApi = {
  getStats: async (): Promise<StatsResponse> => {
    return getApiClient().get<StatsResponse>('/stats')
  },

  getHealth: async (): Promise<{ status: string; version?: string }> => {
    return getApiClient().get('/health')
  },

  getRuleStatus: async (): Promise<RuleStatus> => {
    return getApiClient().get<RuleStatus>('/rules/status')
  },

  getDashboardStats: async (): Promise<DashboardStats> => {
    // Aggregate from multiple endpoints
    const [stats, ruleStatus] = await Promise.all([
      getApiClient().get<StatsResponse>('/stats'),
      getApiClient().get<RuleStatus>('/rules/status').catch(() => null),
    ])

    return {
      total_invoices: stats.total_invoices,
      total_contracts: stats.total_contracts,
      documents_processed: stats.total_invoices + stats.total_contracts,
      documents_change_percent: 0,
      total_value: stats.total_value ?? 0,
      value_change_percent: 0,
      anomaly_rate: stats.anomaly_rate ?? 0,
      anomaly_rate_change: 0,
      active_rules: ruleStatus?.active_rules ?? ruleStatus?.total_rules ?? 0,
      rules_change: 0,
    }
  },
}

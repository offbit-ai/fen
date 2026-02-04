import { getApiClient } from '../client'
import type {
  Anomaly,
  AnomalySeverity,
  AnomalyStatus,
  AnomalyType,
  PaginatedResponse,
} from '@/types/api'

export interface ListAnomaliesParams {
  page?: number
  page_size?: number
  severity?: AnomalySeverity
  status?: AnomalyStatus
  type?: AnomalyType
  document_id?: string
  from_date?: string
  to_date?: string
  sort_by?: string
  sort_order?: 'asc' | 'desc'
}

export const anomaliesApi = {
  list: async (params?: ListAnomaliesParams): Promise<PaginatedResponse<Anomaly>> => {
    return getApiClient().get<PaginatedResponse<Anomaly>>('/anomalies', params as Record<string, string | number | boolean | undefined>)
  },

  get: async (id: string): Promise<Anomaly> => {
    return getApiClient().get<Anomaly>(`/anomalies/${id}`)
  },

  resolve: async (id: string, notes?: string): Promise<Anomaly> => {
    return getApiClient().post<Anomaly>(`/anomalies/${id}/resolve`, { notes })
  },

  dismiss: async (id: string, reason: string): Promise<Anomaly> => {
    return getApiClient().post<Anomaly>(`/anomalies/${id}/dismiss`, { reason })
  },

  investigate: async (id: string): Promise<Anomaly> => {
    return getApiClient().post<Anomaly>(`/anomalies/${id}/investigate`)
  },

  // Bulk operations
  bulkResolve: async (ids: string[], notes?: string): Promise<{ resolved: number }> => {
    return getApiClient().post('/anomalies/bulk/resolve', { ids, notes })
  },

  bulkDismiss: async (ids: string[], reason: string): Promise<{ dismissed: number }> => {
    return getApiClient().post('/anomalies/bulk/dismiss', { ids, reason })
  },

  // Statistics
  getStats: async (): Promise<{
    total: number
    by_severity: Record<AnomalySeverity, number>
    by_status: Record<AnomalyStatus, number>
    by_type: Record<AnomalyType, number>
  }> => {
    return getApiClient().get('/anomalies/stats')
  },
}

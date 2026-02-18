import { useApi } from './use-api'
import { anomaliesApi, type ListAnomaliesParams } from '@/api/endpoints/anomalies'
import type { Anomaly, AnomalySeverity, AnomalyStatus, AnomalyType, PaginatedResponse } from '@/types/api'

export function useAnomalies(params?: ListAnomaliesParams) {
  return useApi<PaginatedResponse<Anomaly>>(
    () => anomaliesApi.list(params),
    [params?.page, params?.page_size, params?.severity, params?.status, params?.type, params?.document_id]
  )
}

export function useAnomaly(id: string) {
  return useApi<Anomaly>(
    () => anomaliesApi.get(id),
    [id]
  )
}

export function useAnomalyStats() {
  return useApi<{
    total: number
    by_severity: Record<AnomalySeverity, number>
    by_status: Record<AnomalyStatus, number>
    by_type: Record<AnomalyType, number>
  }>(
    () => anomaliesApi.getStats(),
    []
  )
}

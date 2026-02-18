import { useApi } from './use-api'
import { documentsApi, type ListDocumentsParams } from '@/api/endpoints/documents'
import type { Invoice, Contract, PaginatedResponse } from '@/types/api'

export function useInvoices(params?: ListDocumentsParams) {
  return useApi<PaginatedResponse<Invoice>>(
    () => documentsApi.listInvoices(params),
    [params?.page, params?.page_size, params?.status, params?.vendor, params?.sort_by, params?.sort_order]
  )
}

export function useInvoice(id: string) {
  return useApi<Invoice>(
    () => documentsApi.getInvoice(id),
    [id]
  )
}

export function useContracts(params?: ListDocumentsParams) {
  return useApi<PaginatedResponse<Contract>>(
    () => documentsApi.listContracts(params),
    [params?.page, params?.page_size, params?.status, params?.vendor, params?.sort_by, params?.sort_order]
  )
}

export function useContract(id: string) {
  return useApi<Contract>(
    () => documentsApi.getContract(id),
    [id]
  )
}

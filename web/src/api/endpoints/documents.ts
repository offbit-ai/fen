import { getApiClient } from '../client'
import type {
  Invoice,
  Contract,
  PaginatedResponse,
} from '@/types/api'

export interface ListDocumentsParams {
  page?: number
  page_size?: number
  status?: string
  vendor?: string
  from_date?: string
  to_date?: string
  sort_by?: string
  sort_order?: 'asc' | 'desc'
}

export const documentsApi = {
  // Invoices
  listInvoices: async (params?: ListDocumentsParams): Promise<PaginatedResponse<Invoice>> => {
    return getApiClient().get<PaginatedResponse<Invoice>>('/documents', {
      type: 'invoice',
      ...params,
    })
  },

  getInvoice: async (id: string): Promise<Invoice> => {
    return getApiClient().get<Invoice>(`/documents/${id}`)
  },

  createInvoice: async (data: Partial<Invoice>): Promise<Invoice> => {
    return getApiClient().post<Invoice>('/documents', data)
  },

  updateInvoice: async (id: string, data: Partial<Invoice>): Promise<Invoice> => {
    return getApiClient().put<Invoice>(`/documents/${id}`, data)
  },

  deleteInvoice: async (id: string): Promise<void> => {
    return getApiClient().delete(`/documents/${id}`)
  },

  // Contracts
  listContracts: async (params?: ListDocumentsParams): Promise<PaginatedResponse<Contract>> => {
    return getApiClient().get<PaginatedResponse<Contract>>('/documents', {
      type: 'contract',
      ...params,
    })
  },

  getContract: async (id: string): Promise<Contract> => {
    return getApiClient().get<Contract>(`/documents/${id}`)
  },

  // Document actions
  validateDocument: async (id: string): Promise<{ valid: boolean; errors: string[] }> => {
    return getApiClient().post(`/documents/${id}/validate`)
  },

  approveDocument: async (id: string, notes?: string): Promise<Invoice> => {
    return getApiClient().post<Invoice>(`/documents/${id}/approve`, { notes })
  },

  rejectDocument: async (id: string, reason: string): Promise<Invoice> => {
    return getApiClient().post<Invoice>(`/documents/${id}/reject`, { reason })
  },

  // Upload
  uploadDocument: async (file: File, metadata?: Record<string, unknown>): Promise<Invoice> => {
    const formData = new FormData()
    formData.append('file', file)
    if (metadata) {
      formData.append('metadata', JSON.stringify(metadata))
    }

    const response = await fetch('/api/v1/ingest', {
      method: 'POST',
      body: formData,
      headers: {
        // Don't set Content-Type - browser will set it with boundary
      },
    })

    if (!response.ok) {
      throw new Error('Upload failed')
    }

    return response.json()
  },
}

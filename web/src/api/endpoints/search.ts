import { getApiClient } from '../client'
import type {
  SearchParams,
  SearchResponse,
  QueryRequest,
  QueryResult,
} from '@/types/api'

type QueryParams = Record<string, string | number | boolean | undefined>

export const searchApi = {
  // Full-text search
  textSearch: async (params: SearchParams): Promise<SearchResponse> => {
    const queryParams: QueryParams = {
      q: params.q,
      limit: params.limit,
      offset: params.offset,
      ...params.filters,
    }
    return getApiClient().get<SearchResponse>('/search/text', queryParams)
  },

  // Semantic/vector search
  semanticSearch: async (params: SearchParams): Promise<SearchResponse> => {
    const queryParams: QueryParams = {
      q: params.q,
      limit: params.limit,
      offset: params.offset,
      ...params.filters,
    }
    return getApiClient().get<SearchResponse>('/search/semantic', queryParams)
  },

  // Hybrid search (combines text + semantic)
  hybridSearch: async (params: SearchParams): Promise<SearchResponse> => {
    const queryParams: QueryParams = {
      q: params.q,
      limit: params.limit,
      offset: params.offset,
      ...params.filters,
    }
    return getApiClient().get<SearchResponse>('/search/hybrid', queryParams)
  },

  // SQL-like query execution
  executeQuery: async (request: QueryRequest): Promise<QueryResult> => {
    return getApiClient().post<QueryResult>('/search/query', request)
  },
}

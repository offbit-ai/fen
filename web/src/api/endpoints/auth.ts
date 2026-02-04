import { getApiClient } from '../client'
import type { LoginRequest, LoginResponse, User } from '@/types/api'

export const authApi = {
  login: async (credentials: LoginRequest): Promise<LoginResponse> => {
    return getApiClient().post<LoginResponse>('/auth/login', credentials)
  },

  logout: async (): Promise<void> => {
    return getApiClient().post('/auth/logout')
  },

  me: async (): Promise<User> => {
    return getApiClient().get<User>('/auth/me')
  },

  refresh: async (refreshToken: string): Promise<LoginResponse> => {
    return getApiClient().post<LoginResponse>('/auth/refresh', { refresh_token: refreshToken })
  },

  // OIDC endpoints
  getOidcAuthUrl: (): string => {
    return '/api/v1/auth/oidc/authorize'
  },

  handleOidcCallback: async (code: string, state: string): Promise<LoginResponse> => {
    return getApiClient().post<LoginResponse>('/auth/oidc/callback', { code, state })
  },
}

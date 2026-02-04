import { create } from 'zustand'
import { persist } from 'zustand/middleware'
import type { User, AuthTokens } from '@/types/api'
import { authApi } from '@/api/endpoints/auth'
import { configureApiClient } from '@/api/client'

interface AuthState {
  user: User | null
  tokens: AuthTokens | null
  isAuthenticated: boolean
  isLoading: boolean
  error: string | null
}

interface AuthActions {
  login: (email: string, password: string) => Promise<void>
  logout: () => Promise<void>
  setUser: (user: User) => void
  setTokens: (tokens: AuthTokens) => void
  clearError: () => void
  checkAuth: () => Promise<void>
}

type AuthStore = AuthState & AuthActions

const initialState: AuthState = {
  user: null,
  tokens: null,
  isAuthenticated: false,
  isLoading: false,
  error: null,
}

export const useAuthStore = create<AuthStore>()(
  persist(
    (set, get) => {
      // Configure API client to use token from store
      const getToken = () => get().tokens?.access_token ?? null
      configureApiClient(getToken)

      return {
        ...initialState,

        login: async (email: string, password: string) => {
          set({ isLoading: true, error: null })
          try {
            const response = await authApi.login({ email, password })
            set({
              user: response.user,
              tokens: response.tokens,
              isAuthenticated: true,
              isLoading: false,
            })
          } catch (err) {
            const message = err instanceof Error ? err.message : 'Login failed'
            set({ error: message, isLoading: false })
            throw err
          }
        },

        logout: async () => {
          try {
            await authApi.logout()
          } catch {
            // Ignore logout errors
          } finally {
            set(initialState)
          }
        },

        setUser: (user: User) => {
          set({ user })
        },

        setTokens: (tokens: AuthTokens) => {
          set({ tokens, isAuthenticated: true })
        },

        clearError: () => {
          set({ error: null })
        },

        checkAuth: async () => {
          const { tokens } = get()
          if (!tokens?.access_token) {
            set({ isAuthenticated: false })
            return
          }

          set({ isLoading: true })
          try {
            const user = await authApi.me()
            set({ user, isAuthenticated: true, isLoading: false })
          } catch {
            set(initialState)
          }
        },
      }
    },
    {
      name: 'fenalytics-auth',
      partialize: (state) => ({
        tokens: state.tokens,
        user: state.user,
      }),
    }
  )
)

// Selectors
export const selectUser = (state: AuthStore) => state.user
export const selectIsAuthenticated = (state: AuthStore) => state.isAuthenticated
export const selectIsLoading = (state: AuthStore) => state.isLoading
export const selectAuthError = (state: AuthStore) => state.error

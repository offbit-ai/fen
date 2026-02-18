import { useState, useEffect, useCallback, useRef } from 'react'

interface UseApiState<T> {
  data: T | null
  isLoading: boolean
  error: string | null
}

interface UseApiResult<T> extends UseApiState<T> {
  refetch: () => Promise<void>
}

/**
 * Generic hook for fetching data from an API endpoint.
 * Handles loading, error states, and refetching.
 */
export function useApi<T>(
  fetcher: () => Promise<T>,
  deps: unknown[] = []
): UseApiResult<T> {
  const [state, setState] = useState<UseApiState<T>>({
    data: null,
    isLoading: true,
    error: null,
  })

  const fetcherRef = useRef(fetcher)
  fetcherRef.current = fetcher

  const fetch = useCallback(async () => {
    setState((prev) => ({ ...prev, isLoading: true, error: null }))
    try {
      const data = await fetcherRef.current()
      setState({ data, isLoading: false, error: null })
    } catch (err) {
      const message = err instanceof Error ? err.message : 'An error occurred'
      setState((prev) => ({ ...prev, isLoading: false, error: message }))
    }
  }, [])

  useEffect(() => {
    fetch()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, deps)

  return { ...state, refetch: fetch }
}

/**
 * Hook for API mutations (POST, PUT, DELETE).
 * Returns a trigger function and state.
 */
export function useMutation<TInput, TResult>(
  mutationFn: (input: TInput) => Promise<TResult>
) {
  const [isLoading, setIsLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const mutate = useCallback(
    async (input: TInput): Promise<TResult> => {
      setIsLoading(true)
      setError(null)
      try {
        const result = await mutationFn(input)
        setIsLoading(false)
        return result
      } catch (err) {
        const message = err instanceof Error ? err.message : 'An error occurred'
        setError(message)
        setIsLoading(false)
        throw err
      }
    },
    [mutationFn]
  )

  return { mutate, isLoading, error }
}

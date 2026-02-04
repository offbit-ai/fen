// API Response Types

export interface ApiResponse<T> {
  data: T
  message?: string
}

export interface PaginatedResponse<T> {
  data: T[]
  total: number
  page: number
  page_size: number
  total_pages: number
}

export interface ApiError {
  code: string
  message: string
  details?: Record<string, unknown>
}

// Auth Types
export interface User {
  id: string
  email: string
  name: string
  tenant_id: string
  roles: string[]
  created_at: string
}

export interface AuthTokens {
  access_token: string
  refresh_token?: string
  expires_in: number
  token_type: string
}

export interface LoginRequest {
  email: string
  password: string
}

export interface LoginResponse {
  user: User
  tokens: AuthTokens
}

// Document Types
export interface Invoice {
  id: string
  tenant_id: string
  vendor_name: string
  invoice_number: string
  invoice_date: string
  due_date?: string
  total_amount: number
  currency: string
  status: InvoiceStatus
  line_items: LineItem[]
  anomalies: Anomaly[]
  metadata: Record<string, unknown>
  created_at: string
  updated_at: string
}

export type InvoiceStatus =
  | 'pending'
  | 'validated'
  | 'flagged'
  | 'approved'
  | 'rejected'

export interface LineItem {
  id: string
  description: string
  quantity: number
  unit_price: number
  total: number
  category?: string
}

export interface Contract {
  id: string
  tenant_id: string
  vendor_name: string
  contract_number: string
  start_date: string
  end_date?: string
  total_value: number
  currency: string
  status: ContractStatus
  pricing_terms: PricingTerm[]
  created_at: string
  updated_at: string
}

export type ContractStatus =
  | 'draft'
  | 'active'
  | 'expired'
  | 'terminated'

export interface PricingTerm {
  item_category: string
  unit_price: number
  min_quantity?: number
  max_quantity?: number
  discount_percentage?: number
}

// Anomaly Types
export interface Anomaly {
  id: string
  document_id: string
  document_type: 'invoice' | 'contract'
  tenant_id: string
  anomaly_type: AnomalyType
  severity: AnomalySeverity
  confidence: number
  description: string
  details: Record<string, unknown>
  status: AnomalyStatus
  resolved_by?: string
  resolved_at?: string
  created_at: string
}

export type AnomalyType =
  | 'price_deviation'
  | 'duplicate_invoice'
  | 'missing_contract'
  | 'quantity_mismatch'
  | 'date_anomaly'
  | 'vendor_mismatch'
  | 'statistical_outlier'
  | 'rule_violation'

export type AnomalySeverity = 'critical' | 'high' | 'medium' | 'low'

export type AnomalyStatus = 'open' | 'investigating' | 'resolved' | 'dismissed'

// Search Types
export interface SearchParams {
  q: string
  limit?: number
  offset?: number
  filters?: Record<string, string | number | boolean>
}

export interface SearchResult {
  id: string
  document_type: string
  score: number
  highlight?: string
  data: Invoice | Contract
}

export interface SearchResponse {
  results: SearchResult[]
  total: number
  query: string
  took_ms: number
}

// Query Types
export interface QueryRequest {
  sql: string
  params?: Record<string, unknown>
}

export interface QueryResult {
  columns: string[]
  rows: unknown[][]
  row_count: number
  execution_time_ms: number
}

// Rules Types
export interface RuleStatus {
  total_rules: number
  active_rules: number
  last_execution?: string
  error_count: number
}

// Stats Types
export interface DashboardStats {
  documents_processed: number
  documents_change_percent: number
  total_value: number
  value_change_percent: number
  anomaly_rate: number
  anomaly_rate_change: number
  active_rules: number
  rules_change: number
}

export interface AnomalyTrend {
  date: string
  count: number
  by_severity: Record<AnomalySeverity, number>
}

// Notification Types
export interface Notification {
  id: string
  tenant_id: string
  type: string
  title: string
  message: string
  severity: 'info' | 'warning' | 'error'
  read: boolean
  created_at: string
}

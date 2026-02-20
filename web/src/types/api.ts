// API Response Types

export interface ApiResponse<T> {
  data: T
  message?: string
}

export interface PaginatedResponse<T> {
  items: T[]
  total: number
  limit: number
  offset: number
}

export interface ApiError {
  code: string
  message: string
  details?: Record<string, unknown>
}

// Common Types (matching backend domain types)
export interface Address {
  street?: string
  city?: string
  state?: string
  postal_code?: string
  country?: string
}

export interface Contact {
  name?: string
  email?: string
  phone?: string
}

export interface Party {
  id: string
  name: string
  tax_id?: string
  address?: Address
  contact?: Contact
}

// Currency enum matching backend
export type Currency = 'USD' | 'EUR' | 'GBP' | 'JPY' | 'CAD' | 'AUD' | 'CHF' | 'CNY' | 'Other'

// Validation status matching backend enum
export type ValidationStatus = 'pending' | 'passed' | 'passed_with_warnings' | 'failed'

// Auth Types
export type UserStatus = 'active' | 'pending_verification' | 'suspended' | 'deactivated' | 'locked'

export type Role = 'tenant_admin' | 'analyst' | 'viewer' | 'rule_manager' | 'system_admin' | string

export interface User {
  id: string
  tenant_id: string
  email: string
  display_name: string
  status: UserStatus
  roles: Role[]
  auth_provider: 'local' | 'oidc' | 'saml' | 'api_key'
  created_at: string
  updated_at: string
  last_login_at?: string
  metadata?: Record<string, unknown>
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

// Invoice Types (matching backend fen-core domain)
export interface LineItem {
  line_number?: number
  description: string
  quantity: number | string // number for frontend, string for backend precision
  unit?: string
  unit_price: number | string
  tax_rate?: number | string
  discount?: number | string
  total: number | string
  item_code?: string
  // Frontend convenience fields
  id?: string
  category?: string
}

// Frontend invoice status for UI display
export type InvoiceStatus = 'pending' | 'validated' | 'flagged' | 'approved' | 'rejected'

export interface Invoice {
  id: string
  document_id?: string
  tenant_id: string
  invoice_number: string
  invoice_date: string // NaiveDate as YYYY-MM-DD
  due_date?: string
  po_number?: string
  contract_id?: string
  contract_number?: string
  vendor?: Party
  bill_to?: Party
  currency: Currency | string
  line_items: LineItem[]
  subtotal?: number | string
  tax_amount?: number | string
  discount_amount?: number | string
  total_amount: number | string
  validation_status?: ValidationStatus
  confidence_score?: number
  extracted_text?: string
  // Frontend display fields
  vendor_name?: string
  status?: InvoiceStatus // Frontend UI status
  anomalies?: Anomaly[] // For detail views
  metadata?: Record<string, unknown>
  created_at?: string
  updated_at?: string
}

// Invoice API Response (simplified for list views)
export interface InvoiceListItem {
  id: string
  document_id: string
  invoice_number: string
  invoice_date: string
  due_date?: string
  total_amount: string
  subtotal: string
  tax_amount: string
  currency: string
  vendor_name: string
  validation_status: ValidationStatus
  confidence_score: number
  line_items_count: number
}

// Contract Types (matching backend fen-core domain)
export type ContractType =
  | 'service_agreement'
  | 'purchase_order'
  | 'master_service_agreement'
  | 'statement_of_work'
  | 'amendment'
  | 'other'

export type ClauseType =
  | 'payment_terms'
  | 'termination'
  | 'confidentiality'
  | 'liability'
  | 'intellectual_property'
  | 'dispute_resolution'
  | 'force_majeure'
  | 'governing_law'
  | string // Custom clause types

export interface ContractClause {
  clause_id: string
  clause_type: ClauseType
  title?: string
  text: string
}

// Frontend contract status for UI
export type ContractStatus = 'draft' | 'active' | 'expiring_soon' | 'expired'

export interface Contract {
  id: string
  document_id?: string
  tenant_id?: string // Not displayed in UI
  contract_number?: string
  title?: string
  contract_type?: ContractType
  parties?: Party[]
  effective_date?: string // NaiveDate
  expiration_date?: string
  execution_date?: string
  total_value?: number | string
  currency?: Currency | string
  clauses?: ContractClause[]
  validation_status?: ValidationStatus
  confidence_score?: number
  extracted_text?: string
  // Frontend display fields
  vendor_name?: string
  start_date?: string // Alias for effective_date
  end_date?: string // Alias for expiration_date
  status?: ContractStatus // Frontend UI status
  pricing_terms?: PricingTerm[]
  metadata?: Record<string, unknown>
  created_at?: string
  updated_at?: string
}

export interface PricingTerm {
  item_category: string
  unit_price: string
  min_quantity?: number
  max_quantity?: number
  discount_percentage?: number
}

// Anomaly Types (matching backend fen-core domain + frontend aliases)
export type AnomalyType =
  // Backend types
  | 'math_mismatch'
  | 'missing_field'
  | 'invalid_format'
  | 'out_of_range'
  | 'potential_duplicate'
  | 'date_inconsistency'
  | 'contract_violation'
  | 'validation_failure'
  | 'statistical_outlier'
  // Frontend display aliases
  | 'price_deviation'
  | 'duplicate_invoice'
  | 'missing_contract'
  | 'quantity_mismatch'
  | 'date_anomaly'
  | 'vendor_mismatch'
  | 'rule_violation'

export type AnomalySeverity = 'low' | 'medium' | 'high' | 'critical'

export interface StatisticalScore {
  z_score: number
  percentile: number
  trend: 'increasing' | 'decreasing' | 'stable' | 'volatile'
  is_outlier: boolean
  baseline_mean: number
  baseline_stddev: number
  sample_count: number
}

export interface Anomaly {
  document_id: string
  anomaly_type: AnomalyType
  severity: AnomalySeverity
  description: string
  field_path?: string
  expected_value?: string
  actual_value?: string
  confidence: number // f32
  statistical_score?: StatisticalScore
  detected_at?: string // DateTime<Utc>
  // Frontend convenience fields
  id?: string
  tenant_id?: string
  document_type?: 'invoice' | 'contract'
  status?: AnomalyStatus
  resolved_by?: string
  resolved_at?: string
  created_at?: string
  details?: Record<string, unknown> // For backward compat with frontend
}

// Frontend anomaly status for UI workflow
export type AnomalyStatus = 'open' | 'investigating' | 'resolved' | 'dismissed'

export interface ValidationResult {
  document_id: string
  is_valid: boolean
  anomalies: Anomaly[]
  validation_time_ms: number
}

// Baseline Types (matching backend)
export type BaselinePeriod = 'rolling_30_days' | 'rolling_90_days' | 'rolling_365_days' | string

export interface BaselineStats {
  count: number
  mean: number
  stddev: number
  min: number
  max: number
  p25: number
  p50: number
  p75: number
  p90: number
  p95: number
  p99: number
}

export interface VendorBaseline {
  id: string
  vendor_name: string
  metric_name: string
  period: BaselinePeriod
  stats: BaselineStats
  recent_values: number[]
  computed_at: string
  expires_at: string
}

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
  total_invoices: number
  total_contracts: number
  documents_processed: number
  documents_change_percent: number
  total_value: number
  value_change_percent: number
  anomaly_rate: number
  anomaly_rate_change: number
  active_rules: number
  rules_change: number
}

export interface StatsResponse {
  total_invoices: number
  total_contracts: number
  total_value: number
  total_anomalies: number
  open_anomalies: number
  anomaly_rate: number
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

// Tenant Types
export type TenantStatus = 'active' | 'trial' | 'suspended' | 'deprovisioning' | 'archived'

export interface RetentionPolicy {
  hot_retention_days: number
  warm_retention_days: number
  archive_retention_days: number
  hard_delete: boolean
}

export interface RateLimitConfig {
  requests_per_minute: number
  uploads_per_hour: number
  max_concurrent_jobs: number
  max_batch_size: number
}

export interface QuotaConfig {
  max_storage_bytes: number
  max_documents: number
  max_rules: number
  max_users: number
}

export interface FeatureFlags {
  ml_anomaly_detection: boolean
  statistical_baselines: boolean
  real_time_notifications: boolean
  api_webhooks: boolean
  custom_rules: boolean
  export_enabled: boolean
  zip_queries: boolean
  contract_matching: boolean
}

export interface TenantConfig {
  tenant_id: string
  name: string
  status: TenantStatus
  created_at: string
  updated_at: string
  retention_policies: Record<string, RetentionPolicy>
  rate_limits: RateLimitConfig
  quotas: QuotaConfig
  features: FeatureFlags
  metadata: Record<string, unknown>
}

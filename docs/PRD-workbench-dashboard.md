# Product Requirements Document: Fen Workbench & Dashboard

**Version:** 1.0
**Date:** 2026-02-03
**Status:** Draft
**Author:** Engineering Team

---

## Executive Summary

Fen Workbench is a Business Intelligence and Document Analysis platform that enables finance and procurement teams to analyze invoices, contracts, and financial documents using SQL-like queries, real-time anomaly detection, and AI-powered insights. The platform combines the code-driven analytics approach of Evidence.dev with the real-time data enrichment capabilities of Fluar.com, tailored specifically for financial document processing and fraud detection.

---

## Problem Statement

Finance teams managing large volumes of invoices and contracts face several challenges:

1. **Siloed Data**: Invoice and contract data scattered across systems with no unified query interface
2. **Manual Anomaly Detection**: Reliance on manual review to catch pricing anomalies, duplicate payments, and contract violations
3. **Limited Visibility**: Lack of real-time dashboards showing document processing status and risk indicators
4. **No Historical Analysis**: Difficulty comparing vendor pricing trends and detecting gradual price creep
5. **Collaboration Gaps**: No shared workspace for analysts to build and share reports

---

## Solution Overview

Fen Workbench provides:

- **Query Workbench**: SQL-like interface to query invoices, contracts, and anomalies
- **Live Data Grid**: Real-time columnar view with inline anomaly indicators and AI enrichment
- **Visual Analytics**: Charts, tables, and KPI cards rendered from markdown + SQL (Evidence-style)
- **Visual Rules Graph**: Node-based editor for creating and testing validation rules and decision tables
- **AI Assistant**: Context-aware agent that can query data, explain anomalies, and suggest actions
- **Document Repositories**: Centralized management for contracts and invoices with version tracking
- **Multi-tenant Dashboard**: Role-based access with tenant isolation

---

## User Personas

### 1. Finance Analyst (Primary)
- **Goals**: Identify payment anomalies, validate invoice accuracy, generate reports
- **Pain Points**: Manual data export, limited query capabilities, no real-time alerts
- **Usage**: Daily queries, weekly reports, ad-hoc investigations

### 2. Procurement Manager
- **Goals**: Monitor vendor pricing, ensure contract compliance, track spend
- **Pain Points**: Contract-invoice mismatches, price creep over time
- **Usage**: Monthly reviews, vendor negotiations, compliance audits

### 3. Tenant Administrator
- **Goals**: Manage team access, configure workflows, monitor system health
- **Pain Points**: User provisioning complexity, audit trail gaps
- **Usage**: Weekly user management, monthly access reviews

### 4. System Administrator (Fen Operator)
- **Goals**: Manage tenants, monitor platform health, configure integrations
- **Pain Points**: Multi-tenant complexity, resource allocation
- **Usage**: Continuous monitoring, tenant onboarding

---

## Feature Specifications

## 1. Navigation & Layout

### 1.1 Side Menu (Left Sidebar)

```
┌─────────────────────────────────────────────────────────────┐
│  ┌─────────────────┐                                        │
│  │  🔷 Fen         │  [Tenant Selector ▼]    [User Menu ▼]  │
│  └─────────────────┘                                        │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────────────────────────────────────────────────┐│
│  │ 🏠 Home                                                 ││
│  │ 📊 Dashboard                                            ││
│  │ ─────────────────────────────────────────               ││
│  │ 📁 DOCUMENTS                                            ││
│  │   📄 Invoices                                           ││
│  │   📋 Contracts                                          ││
│  │   📎 Attachments                                        ││
│  │ ─────────────────────────────────────────               ││
│  │ 🔍 ANALYSIS                                             ││
│  │   📝 Query Workbench                                    ││
│  │   📈 Reports                                            ││
│  │   🎯 Anomalies                                          ││
│  │   📉 Baselines                                          ││
│  │ ─────────────────────────────────────────               ││
│  │ ⚡ RULES ENGINE                                         ││
│  │   🔀 Rule Graph                                         ││
│  │   📋 Decision Tables                                    ││
│  │   🧪 Rule Testing                                       ││
│  │   📊 Execution History                                  ││
│  │ ─────────────────────────────────────────               ││
│  │ ⚙️ SETTINGS                                             ││
│  │   👥 Users                                              ││
│  │   🏢 Tenant Settings                                    ││
│  │   🔔 Notifications                                      ││
│  │   🔗 Integrations                                       ││
│  │   📜 Audit Log                                          ││
│  └─────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────┘
```

**Behavior:**
- Collapsible to icon-only mode
- Active item highlighted with accent color
- Sections expandable/collapsible
- Badge indicators for anomaly counts, pending approvals
- Keyboard shortcuts (⌘1 for Dashboard, ⌘2 for Invoices, etc.)

### 1.2 Header Bar

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  [☰]  Fen Workbench    │  🔍 Global Search...    │  🔔 3  │  [Avatar ▼]    │
│       [Breadcrumb: Home > Analysis > Query Workbench]                        │
└──────────────────────────────────────────────────────────────────────────────┘
```

**Components:**
- Hamburger menu (mobile/collapsed mode)
- Global search with keyboard shortcut (⌘K)
- Notification bell with unread count
- User avatar with dropdown (Profile, Settings, Logout)
- Breadcrumb navigation

---

## 2. Dashboard (Home)

### 2.1 Overview Layout

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                           DASHBOARD                                           │
├──────────────────────────────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐         │
│  │ Total       │  │ Processed   │  │ Anomalies   │  │ At Risk     │         │
│  │ Invoices    │  │ Today       │  │ Detected    │  │ Amount      │         │
│  │ 12,847      │  │ 234         │  │ 47 🔴       │  │ $2.4M       │         │
│  │ ↑ 12% MTD   │  │ ↑ 8% avg    │  │ ↓ 15% WoW   │  │ ↑ 23% MoM   │         │
│  └─────────────┘  └─────────────┘  └─────────────┘  └─────────────┘         │
├──────────────────────────────────────────────────────────────────────────────┤
│  ┌─────────────────────────────────┐  ┌─────────────────────────────────┐   │
│  │ Processing Pipeline             │  │ Anomalies by Category           │   │
│  │ [═══════════════░░░░] 78%       │  │ [Pie Chart]                     │   │
│  │                                 │  │ • Price Deviation: 45%          │   │
│  │ Stage         Count    Status   │  │ • Duplicate: 28%                │   │
│  │ ─────────────────────────────   │  │ • Contract Mismatch: 18%        │   │
│  │ Ingested      234      ✓        │  │ • Volume Anomaly: 9%            │   │
│  │ Extracted     228      ✓        │  │                                 │   │
│  │ Validated     215      ●        │  │                                 │   │
│  │ Pending       19       ○        │  │                                 │   │
│  └─────────────────────────────────┘  └─────────────────────────────────┘   │
├──────────────────────────────────────────────────────────────────────────────┤
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │ Invoice Volume Trend (Last 30 Days)                                   │   │
│  │ [Area Chart with anomaly markers]                                     │   │
│  │     $1.2M ─────────────────────────────────────────────               │   │
│  │           ╱╲    ╱╲                                                    │   │
│  │     $800K ╱  ╲──╱  ╲────╱╲                   🔴 Anomaly               │   │
│  │          ╱          ╲──╱  ╲──────────────────                         │   │
│  │     $400K                                                             │   │
│  │           Jan 5   Jan 12   Jan 19   Jan 26   Feb 2                    │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
├──────────────────────────────────────────────────────────────────────────────┤
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │ Recent Anomalies                                          [View All →]│   │
│  │ ─────────────────────────────────────────────────────────────────────│   │
│  │ 🔴 CRITICAL  INV-2024-8847  Acme Corp    +340% price deviation       │   │
│  │ 🟡 WARNING   INV-2024-8832  Widget Inc   Potential duplicate         │   │
│  │ 🟡 WARNING   INV-2024-8819  TechParts    Contract price mismatch     │   │
│  │ 🔵 INFO      INV-2024-8801  Supplies Co  New vendor pattern          │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────────────────────┘
```

### 2.2 KPI Cards

**Design Spec:**
- White card with subtle shadow
- Large primary metric (32px, bold)
- Trend indicator with percentage and direction arrow
- Sparkline (optional, on hover expand)
- Click to drill down

**Data Bindings:**
```sql
-- Total Invoices
SELECT COUNT(*) as total,
       SUM(total_amount) as amount
FROM invoices
WHERE tenant_id = :tenant_id

-- Anomalies Detected
SELECT COUNT(*) as count,
       severity,
       anomaly_type
FROM anomalies
WHERE tenant_id = :tenant_id
  AND detected_at > NOW() - INTERVAL '30 days'
GROUP BY severity, anomaly_type
```

---

## 3. Query Workbench (Evidence-Inspired)

### 3.1 Layout

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  Query Workbench                                    [▶ Run] [💾 Save] [📤]   │
├────────────────────────────────────┬─────────────────────────────────────────┤
│                                    │                                         │
│  ## Monthly Vendor Analysis        │  Preview                    [Raw JSON]  │
│                                    │  ─────────────────────────────────────  │
│  ```sql vendors                    │                                         │
│  SELECT                            │  Monthly Vendor Analysis                │
│    vendor_name,                    │                                         │
│    COUNT(*) as invoice_count,      │  ┌─────────────────────────────────┐   │
│    SUM(total_amount) as total,     │  │ Vendor         Count    Total   │   │
│    AVG(total_amount) as avg_amount │  │ ────────────────────────────────│   │
│  FROM invoices                     │  │ Acme Corp      47       $234K   │   │
│  WHERE invoice_date >= '2024-01'   │  │ Widget Inc     35       $189K   │   │
│  GROUP BY vendor_name              │  │ TechParts      28       $156K   │   │
│  ORDER BY total DESC               │  │ Supplies Co    24       $98K    │   │
│  LIMIT 10                          │  │ ...                             │   │
│  ```                               │  └─────────────────────────────────┘   │
│                                    │                                         │
│  {% big_value                      │  Total Invoice Volume                   │
│    title="Total Invoice Volume"    │  ┌─────────────────────────────────┐   │
│    data=vendors                    │  │  $1.2M                          │   │
│    value=sum(total)                │  │  ↑ 12% vs last month  ~~~~~~~~~ │   │
│    fmt="usd"                       │  └─────────────────────────────────┘   │
│    sparkline=true                  │                                         │
│  /%}                               │                                         │
│                                    │                                         │
│  {% bar_chart                      │  [Bar Chart Visualization]              │
│    data=vendors                    │                                         │
│    x=vendor_name                   │  Acme Corp    ████████████████ $234K    │
│    y=total                         │  Widget Inc   ████████████     $189K    │
│    title="Vendor Spend"            │  TechParts    ██████████       $156K    │
│  /%}                               │  Supplies Co  ██████           $98K     │
│                                    │                                         │
├────────────────────────────────────┴─────────────────────────────────────────┤
│  Schema Browser                    │  Query History                          │
│  ───────────────                   │  ──────────────                         │
│  📁 invoices                       │  • Monthly Vendor Analysis (2m ago)     │
│    ├─ id (uuid)                    │  • Anomaly Trends Q4 (1h ago)           │
│    ├─ vendor_name (text)           │  • Contract Compliance Check (yesterday)│
│    ├─ invoice_number (text)        │                                         │
│    ├─ total_amount (decimal)       │                                         │
│    └─ ...                          │                                         │
│  📁 contracts                      │                                         │
│  📁 anomalies                      │                                         │
│  📁 baselines                      │                                         │
└──────────────────────────────────────────────────────────────────────────────┘
```

### 3.2 Query Language Extensions

**FenQL** extends standard SQL with domain-specific functions:

```sql
-- Anomaly detection in query
SELECT *
FROM invoices
WHERE ANOMALY_SCORE(total_amount, vendor_name) > 0.8

-- Contract compliance check
SELECT
  i.*,
  CONTRACT_MATCH(i.vendor_name, i.unit_price, i.item_description) as compliance
FROM invoices i
WHERE compliance.status = 'VIOLATION'

-- Baseline comparison
SELECT
  vendor_name,
  total_amount,
  BASELINE_DEVIATION(total_amount, vendor_name, 'price') as deviation
FROM invoices
WHERE deviation > 2.0  -- More than 2 standard deviations

-- Time-series analysis
SELECT
  DATE_TRUNC('week', invoice_date) as week,
  vendor_name,
  SUM(total_amount) as weekly_total,
  MOVING_AVG(SUM(total_amount), 4) OVER (PARTITION BY vendor_name ORDER BY week) as ma_4week
FROM invoices
GROUP BY week, vendor_name
```

### 3.3 Visualization Components

| Component | Description | Properties |
|-----------|-------------|------------|
| `big_value` | KPI card with optional sparkline | `title`, `data`, `value`, `fmt`, `sparkline`, `comparison` |
| `table` | Data table with sorting/filtering | `data`, `columns`, `pagination`, `search` |
| `bar_chart` | Horizontal/vertical bar chart | `data`, `x`, `y`, `color`, `orientation` |
| `line_chart` | Time-series line chart | `data`, `x`, `y`, `series`, `markers` |
| `area_chart` | Stacked/filled area chart | `data`, `x`, `y`, `stack`, `fill` |
| `pie_chart` | Pie/donut chart | `data`, `value`, `label`, `donut` |
| `scatter` | Scatter plot with optional regression | `data`, `x`, `y`, `size`, `color`, `trendline` |
| `heatmap` | Matrix heatmap | `data`, `x`, `y`, `value`, `colorscale` |
| `anomaly_timeline` | Timeline with anomaly markers | `data`, `date`, `value`, `anomalies` |

### 3.4 Report Saving & Sharing

**Report Structure:**
```json
{
  "id": "rpt_abc123",
  "tenant_id": "tenant_xyz",
  "name": "Monthly Vendor Analysis",
  "description": "Vendor spend and anomaly summary",
  "content": "## Monthly Vendor Analysis\n\n```sql vendors...",
  "parameters": [
    { "name": "start_date", "type": "date", "default": "2024-01-01" },
    { "name": "vendor", "type": "string", "options": "SELECT DISTINCT vendor_name FROM invoices" }
  ],
  "schedule": {
    "enabled": true,
    "cron": "0 9 * * MON",
    "recipients": ["analyst@company.com"]
  },
  "permissions": {
    "visibility": "tenant",
    "editors": ["user_123"],
    "viewers": ["role:analyst"]
  },
  "created_at": "2024-01-15T10:00:00Z",
  "updated_at": "2024-02-01T14:30:00Z"
}
```

---

## 4. Live Data Grid (Fluar-Inspired)

### 4.1 Invoice Grid Layout

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  Invoices                                              [+ Upload] [⚙ Config] │
├──────────────────────────────────────────────────────────────────────────────┤
│  🔍 Search invoices...   [Status ▼] [Vendor ▼] [Date Range ▼] [Anomaly ▼]   │
├──────────────────────────────────────────────────────────────────────────────┤
│  ENRICH  [📧 Extract] [🔗 Match Contract] [📊 Score Risk] [🔍 Verify Vendor] │
├──────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ☐ │ Status │ Invoice #    │ Vendor       │ Amount    │ Risk  │ Contract   │
│  ──┼────────┼──────────────┼──────────────┼───────────┼───────┼────────────│
│  ☐ │ ✅ Valid│ INV-2024-001│ Acme Corp    │ $12,450.00│ 🟢 Low│ ✓ Matched  │
│  ☐ │ ⚠️ Review│ INV-2024-002│ Widget Inc   │ $8,230.50 │ 🟡 Med│ ⏳ Checking │
│  ☑ │ 🔴 Alert│ INV-2024-003│ TechParts    │ $45,000.00│ 🔴 High│ ❌ Mismatch│
│  ☐ │ ⏳ Proc │ INV-2024-004│ Supplies Co  │ $3,120.00 │ ⏳...  │ ⏳ Searching│
│  ☐ │ ⏳ Proc │ INV-2024-005│ NewVendor    │ $7,500.00 │ ⏳...  │ ⏳ Searching│
│  ☐ │ ✅ Valid│ INV-2024-006│ Acme Corp    │ $9,800.00 │ 🟢 Low│ ✓ Matched  │
│                                                                              │
├──────────────────────────────────────────────────────────────────────────────┤
│  Showing 1-50 of 1,234 invoices    [◀ Prev] [1] [2] [3] ... [25] [Next ▶]   │
└──────────────────────────────────────────────────────────────────────────────┘
```

### 4.2 Real-Time Enrichment Actions

**Enrichment Pipeline:**

```
┌─────────────┐    ┌─────────────┐    ┌─────────────┐    ┌─────────────┐
│ 1. Upload   │ →  │ 2. Extract  │ →  │ 3. Validate │ →  │ 4. Enrich   │
│   Document  │    │   Fields    │    │   Data      │    │   & Score   │
└─────────────┘    └─────────────┘    └─────────────┘    └─────────────┘
                                                                │
                   ┌────────────────────────────────────────────┘
                   ▼
    ┌──────────────────────────────────────────────────────────────┐
    │                    ENRICHMENT ACTIONS                         │
    ├──────────────────────────────────────────────────────────────┤
    │  📧 Extract Fields    │ OCR + ML extraction from document    │
    │  🔗 Match Contract    │ Find matching contract by vendor     │
    │  📊 Score Risk        │ Calculate anomaly probability        │
    │  🔍 Verify Vendor     │ Check vendor registry & history      │
    │  💰 Price Check       │ Compare to historical baselines      │
    │  📋 Duplicate Check   │ Find potential duplicate invoices    │
    │  🏷️ Auto-Categorize   │ Assign GL codes and categories       │
    └──────────────────────────────────────────────────────────────┘
```

### 4.3 Column Configuration

**Default Columns:**
| Column | Type | Width | Sortable | Filterable |
|--------|------|-------|----------|------------|
| Status | Badge | 80px | Yes | Yes |
| Invoice # | Text | 120px | Yes | Yes |
| Vendor | Text + Link | 150px | Yes | Yes |
| Invoice Date | Date | 100px | Yes | Yes |
| Due Date | Date | 100px | Yes | Yes |
| Amount | Currency | 120px | Yes | Yes |
| Risk Score | Badge + Progress | 100px | Yes | Yes |
| Contract Match | Status | 100px | Yes | Yes |
| Anomalies | Tag List | 150px | No | Yes |
| Actions | Buttons | 100px | No | No |

**Custom Column Types:**
- **Computed**: `{baseline_deviation}%` - Shows calculated deviation
- **Linked**: Click vendor to see vendor profile
- **Expandable**: Click row to see full invoice details
- **Inline Edit**: Quick approval/rejection without leaving grid

### 4.4 Bulk Actions

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  ☑ 3 invoices selected                                                       │
│  ─────────────────────────────────────────────────────────────────────────── │
│  [✅ Approve] [❌ Reject] [🔄 Re-process] [📧 Export] [🏷️ Tag] [🗑️ Delete]   │
└──────────────────────────────────────────────────────────────────────────────┘
```

---

## 5. AI Assistant Panel

### 5.1 Layout (Right Sidebar)

```
┌─────────────────────────────────────┐
│  🤖 Fen AI Assistant          [─ ×] │
├─────────────────────────────────────┤
│                                     │
│  ┌─────────────────────────────────┐│
│  │ 👤 User                         ││
│  │ Why is INV-2024-003 flagged as  ││
│  │ high risk?                      ││
│  └─────────────────────────────────┘│
│                                     │
│  ┌─────────────────────────────────┐│
│  │ 🤖 Assistant                    ││
│  │                                 ││
│  │ INV-2024-003 from TechParts is  ││
│  │ flagged for 3 reasons:          ││
│  │                                 ││
│  │ 1. **Price Deviation**: The     ││
│  │    unit price of $450 is 340%   ││
│  │    above the baseline of $102   ││
│  │                                 ││
│  │ 2. **Contract Mismatch**: No    ││
│  │    matching contract found for  ││
│  │    this vendor/item combo       ││
│  │                                 ││
│  │ 3. **New Pattern**: First       ││
│  │    invoice >$40K from this      ││
│  │    vendor                       ││
│  │                                 ││
│  │ [View Invoice] [See Baseline]   ││
│  └─────────────────────────────────┘│
│                                     │
│  ┌─────────────────────────────────┐│
│  │ 💡 Suggested Actions            ││
│  │ • Request quote verification    ││
│  │ • Compare with similar vendors  ││
│  │ • Check if contract exists      ││
│  └─────────────────────────────────┘│
│                                     │
├─────────────────────────────────────┤
│  💬 Ask about this data...    [📎] │
│  ┌─────────────────────────────────┐│
│  │                            [➤] ││
│  └─────────────────────────────────┘│
├─────────────────────────────────────┤
│  Quick Actions:                     │
│  [📊 Summarize] [🔍 Find Similar]   │
│  [📈 Show Trend] [⚠️ List Risks]    │
└─────────────────────────────────────┘
```

### 5.2 AI Capabilities

**Context-Aware Analysis:**
- Understands current view (dashboard, invoice detail, query results)
- Can reference specific invoices, vendors, or anomalies
- Maintains conversation context within session

**Tool Integration:**
| Tool | Description | Example Query |
|------|-------------|---------------|
| `query_invoices` | Search and filter invoices | "Show me all invoices from Acme over $10K" |
| `explain_anomaly` | Explain why an anomaly was flagged | "Why is this flagged as duplicate?" |
| `compare_baseline` | Compare value to historical baseline | "How does this price compare to average?" |
| `find_contract` | Search contracts by vendor/item | "Is there a contract covering this?" |
| `calculate_risk` | Compute risk score for a set | "What's the total risk exposure this month?" |
| `generate_report` | Create analysis report | "Generate a vendor spend report for Q4" |
| `suggest_action` | Recommend next steps | "What should I do about these anomalies?" |

**Response Types:**
- **Narrative**: Natural language explanation
- **Data Table**: Inline table with results
- **Chart**: Embedded visualization
- **Action Buttons**: Quick actions based on context
- **Citations**: Links to source data

### 5.3 Proactive Insights

The AI assistant can proactively surface insights:

```
┌─────────────────────────────────────┐
│  💡 Insight                    [×]  │
├─────────────────────────────────────┤
│  I noticed that invoices from       │
│  **TechParts** have increased 45%   │
│  in unit price over the last 3      │
│  months. This affects 12 invoices   │
│  totaling $127,000.                 │
│                                     │
│  [View Details] [Dismiss] [Mute]    │
└─────────────────────────────────────┘
```

---

## 6. Document Repositories

### 6.1 Invoice Repository

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  📄 Invoices                                                                 │
├──────────────────────────────────────────────────────────────────────────────┤
│  [+ Upload Invoice] [📁 Bulk Import] [⬇️ Export]                            │
│                                                                              │
│  Tabs: [All] [Pending Review (23)] [Approved] [Rejected] [Anomalies (47)]   │
├──────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  [Live Data Grid - See Section 4]                                           │
│                                                                              │
└──────────────────────────────────────────────────────────────────────────────┘
```

**Invoice Detail View:**
```
┌──────────────────────────────────────────────────────────────────────────────┐
│  ← Back to Invoices                                          [✅ Approve] [❌]│
├──────────────────────────────────────────────────────────────────────────────┤
│  ┌─────────────────────────────┐  ┌─────────────────────────────────────────┐│
│  │                             │  │ Invoice Details                         ││
│  │    [PDF Preview]            │  │ ─────────────────────────────────────── ││
│  │                             │  │ Invoice #:     INV-2024-003             ││
│  │    Page 1 of 2              │  │ Vendor:        TechParts Inc            ││
│  │                             │  │ Invoice Date:  2024-01-15               ││
│  │    [🔍 Zoom] [⬇️ Download]  │  │ Due Date:      2024-02-14               ││
│  │                             │  │ PO Number:     PO-2024-789              ││
│  └─────────────────────────────┘  │ ─────────────────────────────────────── ││
│                                   │ Line Items:                             ││
│                                   │ ┌──────────────────────────────────────┐││
│                                   │ │ Item          Qty    Price    Total  │││
│                                   │ │ Widget A      100    $450    $45,000 │││
│                                   │ │               ⚠️ 340% above baseline │││
│                                   │ └──────────────────────────────────────┘││
│                                   │ ─────────────────────────────────────── ││
│                                   │ Subtotal:      $45,000.00               ││
│                                   │ Tax:           $3,600.00                ││
│                                   │ Total:         $48,600.00               ││
│                                   └─────────────────────────────────────────┘│
├──────────────────────────────────────────────────────────────────────────────┤
│  Anomalies (3)                                                               │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │ 🔴 CRITICAL  Price Deviation       +340% vs baseline ($102 avg)       │  │
│  │ 🔴 CRITICAL  Contract Mismatch     No contract found for this item    │  │
│  │ 🟡 WARNING   New Pattern           First invoice >$40K from vendor    │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
├──────────────────────────────────────────────────────────────────────────────┤
│  Activity Log                                                                │
│  • 2024-01-15 10:23 - Uploaded by john@company.com                          │
│  • 2024-01-15 10:24 - ML extraction completed                               │
│  • 2024-01-15 10:24 - 3 anomalies detected                                  │
│  • 2024-01-15 11:00 - Assigned to sarah@company.com for review              │
└──────────────────────────────────────────────────────────────────────────────┘
```

### 6.2 Contract Repository

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  📋 Contracts                                                                │
├──────────────────────────────────────────────────────────────────────────────┤
│  [+ Add Contract] [📁 Bulk Import]                                          │
│                                                                              │
│  Tabs: [All] [Active (156)] [Expiring Soon (12)] [Expired] [Draft]          │
├──────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ☐ │ Status │ Contract #   │ Vendor       │ Start      │ End        │ Value │
│  ──┼────────┼──────────────┼──────────────┼────────────┼────────────┼───────│
│  ☐ │ ✅ Active│ CTR-2023-045│ Acme Corp    │ 2023-01-01│ 2025-12-31│ $2.5M │
│  ☐ │ ⚠️ Expiring│ CTR-2022-012│ Widget Inc │ 2022-06-01│ 2024-05-31│ $1.2M │
│  ☐ │ ✅ Active│ CTR-2024-001│ TechParts    │ 2024-01-01│ 2026-12-31│ $500K │
│                                                                              │
├──────────────────────────────────────────────────────────────────────────────┤
│  Contract Pricing (CTR-2024-001 - TechParts)                                 │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │ Item Description    │ Unit Price │ Min Qty │ Max Qty │ Effective     │  │
│  │ ────────────────────┼────────────┼─────────┼─────────┼───────────────│  │
│  │ Widget A            │ $95.00     │ 50      │ 500     │ 2024-01-01    │  │
│  │ Widget B            │ $120.00    │ 25      │ 250     │ 2024-01-01    │  │
│  │ Service Fee         │ $150/hr    │ -       │ 100 hrs │ 2024-01-01    │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────────────────────┘
```

---

## 7. User & Tenant Management

### 7.1 User Management (Tenant Admin View)

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  👥 Users                                                [+ Invite User]     │
├──────────────────────────────────────────────────────────────────────────────┤
│  🔍 Search users...                              [Role ▼] [Status ▼]         │
├──────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌────────┐ John Smith                                                       │
│  │  👤    │ john@company.com                                                 │
│  └────────┘ Role: Tenant Admin    Status: ✅ Active    Last login: 2h ago   │
│             [Edit] [Reset Password] [Disable]                                │
│  ─────────────────────────────────────────────────────────────────────────── │
│  ┌────────┐ Sarah Johnson                                                    │
│  │  👤    │ sarah@company.com                                                │
│  └────────┘ Role: Analyst         Status: ✅ Active    Last login: 5m ago   │
│             [Edit] [Reset Password] [Disable]                                │
│  ─────────────────────────────────────────────────────────────────────────── │
│  ┌────────┐ Mike Wilson                                                      │
│  │  👤    │ mike@company.com                                                 │
│  └────────┘ Role: Viewer          Status: ⏸️ Disabled   Last login: 30d ago │
│             [Edit] [Reset Password] [Enable]                                 │
│                                                                              │
└──────────────────────────────────────────────────────────────────────────────┘
```

### 7.2 User Invite Flow

```
┌─────────────────────────────────────┐
│  Invite User                   [×]  │
├─────────────────────────────────────┤
│                                     │
│  Email Address *                    │
│  ┌─────────────────────────────────┐│
│  │ newuser@company.com             ││
│  └─────────────────────────────────┘│
│                                     │
│  Role *                             │
│  ┌─────────────────────────────────┐│
│  │ Analyst                      ▼  ││
│  └─────────────────────────────────┘│
│  ○ Tenant Admin - Full access       │
│  ● Analyst - Process documents      │
│  ○ Viewer - Read-only access        │
│  ○ Rule Manager - Configure rules   │
│                                     │
│  Send welcome email                 │
│  ☑ Include getting started guide    │
│                                     │
│  [Cancel]              [Send Invite]│
└─────────────────────────────────────┘
```

### 7.3 Tenant Settings (Tenant Admin View)

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  🏢 Tenant Settings                                                          │
├──────────────────────────────────────────────────────────────────────────────┤
│  Tabs: [General] [Security] [Notifications] [Integrations] [Billing]        │
├──────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  Organization Name                                                           │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │ Acme Corporation                                                      │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
│  Tenant ID                                                                   │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │ tenant_abc123def456                                        [📋 Copy] │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
│  ─────────────────────────────────────────────────────────────────────────── │
│                                                                              │
│  Data Retention                                                              │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │ Invoice retention:    90 days  ▼                                      │   │
│  │ Audit log retention:  365 days ▼                                      │   │
│  │ Archived data:        7 years  ▼                                      │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
│  ─────────────────────────────────────────────────────────────────────────── │
│                                                                              │
│  API Access                                                                  │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │ API Key: fen_api_****************************4a2f        [Regenerate]│   │
│  │ Rate Limit: 1,000 requests/minute                                    │   │
│  │ Usage this month: 45,231 / 100,000                                   │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
│                                                        [Save Changes]        │
└──────────────────────────────────────────────────────────────────────────────┘
```

### 7.4 System Admin View (Multi-Tenant Management)

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  🏢 Tenant Management (System Admin)                     [+ Create Tenant]   │
├──────────────────────────────────────────────────────────────────────────────┤
│  🔍 Search tenants...                           [Plan ▼] [Status ▼]          │
├──────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  │ Tenant          │ Plan       │ Users │ Documents │ Status  │ Created    │
│  │ ────────────────┼────────────┼───────┼───────────┼─────────┼────────────│
│  │ Acme Corp       │ Enterprise │ 45    │ 12,847    │ ✅ Active│ 2023-06-01│
│  │ Widget Inc      │ Pro        │ 12    │ 3,421     │ ✅ Active│ 2023-09-15│
│  │ StartupXYZ      │ Starter    │ 3     │ 234       │ ⏸️ Trial │ 2024-01-01│
│  │ OldCo           │ Pro        │ 8     │ 8,902     │ 🔴 Suspended│ 2022-03-20│
│                                                                              │
├──────────────────────────────────────────────────────────────────────────────┤
│  Platform Statistics                                                         │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐        │
│  │ Total       │  │ Active      │  │ Total       │  │ API Calls   │        │
│  │ Tenants     │  │ Users       │  │ Documents   │  │ Today       │        │
│  │ 127         │  │ 1,234       │  │ 2.4M        │  │ 847K        │        │
│  └─────────────┘  └─────────────┘  └─────────────┘  └─────────────┘        │
└──────────────────────────────────────────────────────────────────────────────┘
```

---

## 8. Anomaly Management

### 8.1 Anomaly List View

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  🎯 Anomalies                                                                │
├──────────────────────────────────────────────────────────────────────────────┤
│  Tabs: [All (147)] [Critical (23)] [Warning (89)] [Info (35)] [Resolved]    │
├──────────────────────────────────────────────────────────────────────────────┤
│  🔍 Search...    [Type ▼] [Vendor ▼] [Date Range ▼]    [Export] [Bulk Action]│
├──────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ☐ │ Severity │ Type              │ Invoice      │ Vendor     │ Details    │
│  ──┼──────────┼───────────────────┼──────────────┼────────────┼────────────│
│  ☐ │ 🔴 CRIT  │ Price Deviation   │ INV-2024-003│ TechParts  │ +340%      │
│  ☐ │ 🔴 CRIT  │ Duplicate Invoice │ INV-2024-018│ Acme Corp  │ 99% match  │
│  ☐ │ 🟡 WARN  │ Contract Mismatch │ INV-2024-022│ Widget Inc │ No contract│
│  ☐ │ 🟡 WARN  │ Volume Spike      │ INV-2024-031│ Supplies   │ 3x average │
│  ☐ │ 🔵 INFO  │ New Vendor        │ INV-2024-045│ NewCo      │ First order│
│                                                                              │
└──────────────────────────────────────────────────────────────────────────────┘
```

### 8.2 Anomaly Detail & Resolution

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  ← Back                                         🔴 CRITICAL: Price Deviation │
├──────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  Invoice: INV-2024-003                    Vendor: TechParts Inc             │
│  Detected: 2024-01-15 10:24              Confidence: 98.5%                  │
│                                                                              │
│  ─────────────────────────────────────────────────────────────────────────── │
│                                                                              │
│  Anomaly Details                                                             │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │ Item: Widget A                                                        │   │
│  │ Invoice Price: $450.00                                                │   │
│  │ Baseline Price: $102.00 (avg of last 50 invoices)                     │   │
│  │ Deviation: +341.2% (4.4 standard deviations)                          │   │
│  │                                                                       │   │
│  │ [Price History Chart]                                                 │   │
│  │     $450 ─────────────────────────────────────── ● Current            │   │
│  │     $200                                                              │   │
│  │     $100 ●─●──●───●──●──●───●──●───●──●───●───── Historical           │   │
│  │          Jun  Jul Aug Sep Oct Nov Dec Jan Feb                         │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
│  ─────────────────────────────────────────────────────────────────────────── │
│                                                                              │
│  Resolution                                                                  │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │ Status: ○ Open  ○ Under Review  ○ Resolved  ○ False Positive         │   │
│  │                                                                       │   │
│  │ Resolution Note:                                                      │   │
│  │ ┌────────────────────────────────────────────────────────────────┐   │   │
│  │ │ Verified with vendor - price increase due to supply chain     │   │   │
│  │ │ issues. New contract being negotiated.                         │   │   │
│  │ └────────────────────────────────────────────────────────────────┘   │   │
│  │                                                                       │   │
│  │ [Approve Invoice] [Reject Invoice] [Request More Info] [Escalate]    │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
└──────────────────────────────────────────────────────────────────────────────┘
```

---

## 9. Rules Engine & Visual Graph

### 9.1 Overview

The Rules Engine provides a visual interface for creating, managing, and testing validation rules that detect anomalies in invoices and contracts. Rules are represented as an interactive directed graph, allowing users to understand the flow of validation logic and see how rules connect to form complex detection pipelines.

**Core Capabilities:**
- Visual rule graph editor (node-based flow)
- Decision table editor for tabular rule definitions
- Rule testing with sample data
- Execution history and performance metrics
- Version control for rule changes

### 9.2 Visual Rule Graph Layout

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  🔀 Rule Graph: Invoice Validation Pipeline              [+ Add Node] [Save] │
├──────────────────────────────────────────────────────────────────────────────┤
│  Toolbar: [📥 Input] [⚙️ Condition] [🔀 Branch] [📊 Score] [📤 Output] [🗑️]  │
├──────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────────┐ │
│  │                                                                         │ │
│  │   ┌──────────────┐                                                      │ │
│  │   │ 📥 Invoice   │                                                      │ │
│  │   │    Input     │                                                      │ │
│  │   └──────┬───────┘                                                      │ │
│  │          │                                                              │ │
│  │          ▼                                                              │ │
│  │   ┌──────────────┐     ┌──────────────┐                                 │ │
│  │   │ ⚙️ Extract   │────▶│ ⚙️ Validate  │                                 │ │
│  │   │   Fields     │     │   Format     │                                 │ │
│  │   └──────────────┘     └──────┬───────┘                                 │ │
│  │                               │                                         │ │
│  │                               ▼                                         │ │
│  │                        ┌──────────────┐                                 │ │
│  │                        │ 🔀 Check     │                                 │ │
│  │                        │   Amount     │                                 │ │
│  │                        └──────┬───────┘                                 │ │
│  │                               │                                         │ │
│  │              ┌────────────────┼────────────────┐                        │ │
│  │              ▼                ▼                ▼                        │ │
│  │       ┌────────────┐  ┌────────────┐  ┌────────────┐                    │ │
│  │       │ < $1,000   │  │ $1K - $50K │  │ > $50,000  │                    │ │
│  │       │ Low Risk   │  │ Med Risk   │  │ High Risk  │                    │ │
│  │       └─────┬──────┘  └─────┬──────┘  └─────┬──────┘                    │ │
│  │             │               │               │                           │ │
│  │             ▼               ▼               ▼                           │ │
│  │       ┌────────────┐  ┌────────────┐  ┌────────────┐                    │ │
│  │       │ 📊 Score   │  │ 📋 Decision│  │ 🚨 Alert + │                    │ │
│  │       │ = 0.2      │  │   Table    │  │   Escalate │                    │ │
│  │       └─────┬──────┘  └─────┬──────┘  └─────┬──────┘                    │ │
│  │             │               │               │                           │ │
│  │             └───────────────┼───────────────┘                           │ │
│  │                             ▼                                           │ │
│  │                      ┌──────────────┐                                   │ │
│  │                      │ 📤 Anomaly   │                                   │ │
│  │                      │    Output    │                                   │ │
│  │                      └──────────────┘                                   │ │
│  │                                                                         │ │
│  └─────────────────────────────────────────────────────────────────────────┘ │
│                                                                              │
│  Minimap: [▪▪▪]                          Zoom: [−] 100% [+]   Fit: [⊡]      │
└──────────────────────────────────────────────────────────────────────────────┘
```

### 9.3 Node Types

| Node Type | Icon | Description | Inputs | Outputs |
|-----------|------|-------------|--------|---------|
| **Input** | 📥 | Data source (Invoice, Contract) | None | Document data |
| **Extract** | ⚙️ | Field extraction/transformation | Document | Extracted fields |
| **Condition** | 🔀 | Boolean condition check | Any value | True/False branches |
| **Branch** | ⇅ | Multi-way split based on ranges | Numeric value | Multiple branches |
| **Decision Table** | 📋 | Tabular rule lookup | Multiple inputs | Score/Action |
| **Score** | 📊 | Assign anomaly score | Conditions met | Risk score |
| **Statistical** | 📈 | Baseline comparison | Value + Context | Deviation |
| **ML Model** | 🤖 | ML prediction node | Features | Prediction |
| **Alert** | 🚨 | Trigger notification | Any input | Alert event |
| **Output** | 📤 | Final anomaly result | Scores/Flags | Anomaly record |

### 9.4 Node Configuration Panel

```
┌─────────────────────────────────────┐
│  ⚙️ Node: Check Amount         [×]  │
├─────────────────────────────────────┤
│                                     │
│  Name                               │
│  ┌─────────────────────────────────┐│
│  │ Check Invoice Amount            ││
│  └─────────────────────────────────┘│
│                                     │
│  Type: Branch (Range)               │
│                                     │
│  Input Field                        │
│  ┌─────────────────────────────────┐│
│  │ invoice.total_amount         ▼  ││
│  └─────────────────────────────────┘│
│                                     │
│  Branches                           │
│  ┌─────────────────────────────────┐│
│  │ Branch 1: < 1000                ││
│  │   Label: "Low Risk"             ││
│  │ ─────────────────────────────── ││
│  │ Branch 2: 1000 - 50000          ││
│  │   Label: "Medium Risk"          ││
│  │ ─────────────────────────────── ││
│  │ Branch 3: > 50000               ││
│  │   Label: "High Risk"            ││
│  │                    [+ Add Branch]││
│  └─────────────────────────────────┘│
│                                     │
│  [Delete Node]        [Apply]       │
└─────────────────────────────────────┘
```

### 9.5 Decision Table Editor

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  📋 Decision Table: Vendor Price Validation                    [+ Add Rule]  │
├──────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌────────────────────────────────────────────────────────────────────────┐ │
│  │ #  │ Vendor Type │ Amount Range │ Deviation │ Has Contract │ → Action  │ │
│  │────┼─────────────┼──────────────┼───────────┼──────────────┼───────────│ │
│  │ 1  │ Preferred   │ Any          │ < 10%     │ Yes          │ ✅ Approve │ │
│  │ 2  │ Preferred   │ Any          │ 10-25%    │ Yes          │ ⚠️ Review  │ │
│  │ 3  │ Preferred   │ Any          │ > 25%     │ Any          │ 🚨 Alert   │ │
│  │ 4  │ Standard    │ < $10K       │ < 15%     │ Yes          │ ✅ Approve │ │
│  │ 5  │ Standard    │ < $10K       │ < 15%     │ No           │ ⚠️ Review  │ │
│  │ 6  │ Standard    │ $10K-$50K    │ Any       │ No           │ 🚨 Alert   │ │
│  │ 7  │ Standard    │ > $50K       │ Any       │ Any          │ 🚨 Escalate│ │
│  │ 8  │ New         │ Any          │ Any       │ Any          │ ⚠️ Review  │ │
│  │ 9  │ *           │ *            │ > 50%     │ *            │ 🚨 Alert   │ │
│  └────────────────────────────────────────────────────────────────────────┘ │
│                                                                              │
│  Legend: * = Any value    Drag rows to reorder (priority: top to bottom)    │
│                                                                              │
│  [Import CSV] [Export CSV] [Test Table]                        [Save Table] │
└──────────────────────────────────────────────────────────────────────────────┘
```

### 9.6 Statistical Rule Configuration

```
┌─────────────────────────────────────┐
│  📈 Statistical Rule: Price Check   │
├─────────────────────────────────────┤
│                                     │
│  Baseline Type                      │
│  ┌─────────────────────────────────┐│
│  │ ● Vendor + Item combination     ││
│  │ ○ Vendor overall                ││
│  │ ○ Item category                 ││
│  │ ○ Global                        ││
│  └─────────────────────────────────┘│
│                                     │
│  Detection Method                   │
│  ┌─────────────────────────────────┐│
│  │ Z-Score (Standard Deviations) ▼ ││
│  └─────────────────────────────────┘│
│                                     │
│  Thresholds                         │
│  ┌─────────────────────────────────┐│
│  │ Warning:  > 2.0 σ               ││
│  │ Critical: > 3.0 σ               ││
│  │ ─────────────────────────────── ││
│  │ Min samples for baseline: 10    ││
│  │ Baseline window: 90 days        ││
│  └─────────────────────────────────┘│
│                                     │
│  Preview Distribution               │
│  ┌─────────────────────────────────┐│
│  │      ╱╲                         ││
│  │     ╱  ╲    μ=$102              ││
│  │    ╱    ╲   σ=$15               ││
│  │ ──╱──────╲──────────            ││
│  │   -3σ  μ  +3σ                   ││
│  └─────────────────────────────────┘│
│                                     │
│  [Test with Sample]     [Apply]     │
└─────────────────────────────────────┘
```

### 9.7 Rule Testing Interface

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  🧪 Rule Testing: Invoice Validation Pipeline                                │
├──────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  Test Input                                                                  │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │ {                                                                     │   │
│  │   "vendor_name": "TechParts Inc",                                     │   │
│  │   "vendor_type": "Standard",                                          │   │
│  │   "total_amount": 45000.00,                                           │   │
│  │   "line_items": [                                                     │   │
│  │     { "description": "Widget A", "quantity": 100, "unit_price": 450 } │   │
│  │   ],                                                                  │   │
│  │   "has_contract": false                                               │   │
│  │ }                                                                     │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
│  [▶ Run Test]  [📁 Load Sample]  [🎲 Generate Random]                        │
│                                                                              │
├──────────────────────────────────────────────────────────────────────────────┤
│  Execution Trace                                                             │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │ ✓ 📥 Invoice Input                              0.1ms                 │   │
│  │   └─▶ Loaded invoice data                                             │   │
│  │ ✓ ⚙️ Extract Fields                             0.3ms                 │   │
│  │   └─▶ Extracted: vendor_name, total_amount, line_items               │   │
│  │ ✓ ⚙️ Validate Format                            0.2ms                 │   │
│  │   └─▶ All required fields present                                     │   │
│  │ ✓ 🔀 Check Amount                               0.1ms                 │   │
│  │   └─▶ $45,000 → Branch: "$1K - $50K" (Medium Risk)                   │   │
│  │ ✓ 📋 Decision Table                             0.4ms                 │   │
│  │   └─▶ Rule #6 matched: Standard + $10K-$50K + No Contract            │   │
│  │   └─▶ Action: 🚨 Alert                                                │   │
│  │ ✓ 📊 Calculate Score                            0.2ms                 │   │
│  │   └─▶ Risk Score: 0.85 (High)                                         │   │
│  │ ✓ 📤 Anomaly Output                             0.1ms                 │   │
│  │   └─▶ Generated anomaly: PRICE_RISK_HIGH                              │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
│  Total Execution Time: 1.4ms                                                 │
│                                                                              │
├──────────────────────────────────────────────────────────────────────────────┤
│  Output                                                                      │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │ {                                                                     │   │
│  │   "anomaly_detected": true,                                           │   │
│  │   "anomaly_type": "PRICE_RISK_HIGH",                                  │   │
│  │   "severity": "critical",                                             │   │
│  │   "risk_score": 0.85,                                                 │   │
│  │   "matched_rules": ["decision_table_rule_6"],                         │   │
│  │   "recommendations": ["Require contract before payment"]              │   │
│  │ }                                                                     │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────────────────────┘
```

### 9.8 Execution History & Analytics

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  📊 Rule Execution History                                                   │
├──────────────────────────────────────────────────────────────────────────────┤
│  Time Range: [Last 7 Days ▼]    Pipeline: [Invoice Validation ▼]            │
├──────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐        │
│  │ Total       │  │ Anomalies   │  │ Avg Exec    │  │ Error       │        │
│  │ Executions  │  │ Detected    │  │ Time        │  │ Rate        │        │
│  │ 12,847      │  │ 847 (6.5%)  │  │ 2.3ms       │  │ 0.02%       │        │
│  └─────────────┘  └─────────────┘  └─────────────┘  └─────────────┘        │
│                                                                              │
│  Rule Trigger Frequency                                                      │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │ decision_table_rule_6  ████████████████████████████████████ 423      │   │
│  │ statistical_price      ██████████████████████████           312      │   │
│  │ decision_table_rule_3  █████████████████                    198      │   │
│  │ duplicate_check        ████████████                         145      │   │
│  │ contract_mismatch      ████████                             102      │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
│  Execution Timeline                                                          │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │ Executions ───  Anomalies ─ ─ ─                                       │   │
│  │     2K │      ╱╲    ╱╲                                                │   │
│  │     1K │ ────╱──╲──╱──╲────────                                       │   │
│  │        │─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─                                       │   │
│  │        └──────────────────────────────────                            │   │
│  │          Mon   Tue   Wed   Thu   Fri   Sat   Sun                      │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
│  Recent Executions                                                           │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │ Time       │ Invoice      │ Result      │ Rules Triggered │ Duration │   │
│  │────────────┼──────────────┼─────────────┼─────────────────┼──────────│   │
│  │ 10:23:45   │ INV-2024-889 │ 🚨 Alert    │ rule_6, stat    │ 2.1ms    │   │
│  │ 10:23:44   │ INV-2024-888 │ ✅ Pass     │ -               │ 1.8ms    │   │
│  │ 10:23:42   │ INV-2024-887 │ ⚠️ Warning  │ rule_2          │ 2.4ms    │   │
│  │ 10:23:41   │ INV-2024-886 │ ✅ Pass     │ -               │ 1.9ms    │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────────────────────┘
```

### 9.9 Rule Version Control

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  📜 Version History: Invoice Validation Pipeline                             │
├──────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │ Version │ Author        │ Date         │ Changes          │ Status   │   │
│  │─────────┼───────────────┼──────────────┼──────────────────┼──────────│   │
│  │ v2.3    │ sarah@co.com  │ 2024-02-01   │ Added ML node    │ 🟢 Active│   │
│  │ v2.2    │ john@co.com   │ 2024-01-28   │ Updated thresholds│ Archived│   │
│  │ v2.1    │ sarah@co.com  │ 2024-01-15   │ New decision table│ Archived│   │
│  │ v2.0    │ john@co.com   │ 2024-01-01   │ Major refactor   │ Archived │   │
│  │ v1.x    │ ...           │ ...          │ ...              │ Archived │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
│  [Compare Versions]  [Rollback to v2.2]  [Export]                            │
│                                                                              │
│  Version Diff (v2.2 → v2.3)                                                  │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │ + Added node: "ML Price Predictor" (ml_model)                         │   │
│  │ + Connected: statistical_price → ml_predictor → score_calculator      │   │
│  │ ~ Modified: score_calculator weights (0.6 statistical, 0.4 ml)        │   │
│  │ ~ Modified: decision_table_rule_3 threshold (25% → 20%)               │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────────────────────┘
```

### 9.10 Rule Graph Technical Implementation

**Graph Library:** React Flow (for node-based editor)

```typescript
interface RuleNode {
  id: string;
  type: 'input' | 'extract' | 'condition' | 'branch' | 'decision_table' |
        'score' | 'statistical' | 'ml_model' | 'alert' | 'output';
  position: { x: number; y: number };
  data: {
    label: string;
    config: NodeConfig;
    inputs: PortDefinition[];
    outputs: PortDefinition[];
  };
}

interface RuleEdge {
  id: string;
  source: string;
  sourceHandle: string;
  target: string;
  targetHandle: string;
  label?: string;
  animated?: boolean;
}

interface RulePipeline {
  id: string;
  name: string;
  version: string;
  tenant_id: string;
  nodes: RuleNode[];
  edges: RuleEdge[];
  metadata: {
    created_at: string;
    updated_at: string;
    created_by: string;
    description: string;
  };
}

// API for rule execution
interface RuleEngineAPI {
  pipelines: {
    list(): Promise<RulePipeline[]>;
    get(id: string): Promise<RulePipeline>;
    create(pipeline: RulePipelineInput): Promise<RulePipeline>;
    update(id: string, pipeline: RulePipelineInput): Promise<RulePipeline>;
    delete(id: string): Promise<void>;
    execute(id: string, input: object): Promise<ExecutionResult>;
    test(id: string, input: object): Promise<TestResult>;
  };

  decisionTables: {
    list(): Promise<DecisionTable[]>;
    get(id: string): Promise<DecisionTable>;
    evaluate(id: string, input: object): Promise<EvaluationResult>;
  };

  executions: {
    list(params: ExecutionListParams): Promise<PaginatedResult<Execution>>;
    get(id: string): Promise<ExecutionDetail>;
    getTrace(id: string): Promise<ExecutionTrace>;
  };
}
```

---

## 10. Technical Architecture

### 10.1 Frontend Stack

| Component | Technology | Rationale |
|-----------|------------|-----------|
| Framework | React 18 + TypeScript | Industry standard, strong ecosystem |
| State Management | Zustand | Lightweight, TypeScript-first |
| Routing | React Router v6 | Standard routing solution |
| UI Components | Shadcn/ui + Radix | Accessible, customizable primitives |
| Styling | Tailwind CSS | Utility-first, consistent design |
| Charts | Recharts + D3.js | Flexible visualization |
| Data Grid | TanStack Table | High-performance virtual scrolling |
| Query Editor | Monaco Editor | VS Code editor experience |
| Rule Graph | React Flow | Node-based flow editor |
| Forms | React Hook Form + Zod | Type-safe validation |
| Real-time | WebSocket + SSE | Live updates |

### 10.2 API Integration

```typescript
// API Client Structure
interface FenAPI {
  // Authentication
  auth: {
    login(provider: 'oidc' | 'local', credentials?: Credentials): Promise<AuthResult>;
    logout(): Promise<void>;
    refresh(): Promise<TokenPair>;
  };

  // Documents
  invoices: {
    list(params: ListParams): Promise<PaginatedResult<Invoice>>;
    get(id: string): Promise<Invoice>;
    upload(file: File): Promise<Invoice>;
    approve(id: string): Promise<Invoice>;
    reject(id: string, reason: string): Promise<Invoice>;
  };

  contracts: {
    list(params: ListParams): Promise<PaginatedResult<Contract>>;
    get(id: string): Promise<Contract>;
    create(data: ContractInput): Promise<Contract>;
  };

  // Analysis
  query: {
    execute(sql: string): Promise<QueryResult>;
    explain(sql: string): Promise<QueryPlan>;
    save(report: ReportInput): Promise<Report>;
  };

  search: {
    text(query: string, params: SearchParams): Promise<SearchResult>;
    semantic(query: string, params: SearchParams): Promise<SearchResult>;
  };

  // Anomalies
  anomalies: {
    list(params: AnomalyParams): Promise<PaginatedResult<Anomaly>>;
    get(id: string): Promise<AnomalyDetail>;
    resolve(id: string, resolution: Resolution): Promise<Anomaly>;
  };

  // AI Assistant
  assistant: {
    chat(message: string, context: AssistantContext): Promise<AssistantResponse>;
    streamChat(message: string, context: AssistantContext): AsyncIterable<AssistantChunk>;
  };

  // Admin
  users: {
    list(): Promise<User[]>;
    invite(email: string, role: Role): Promise<Invitation>;
    update(id: string, data: UserUpdate): Promise<User>;
  };

  tenants: {
    getCurrent(): Promise<Tenant>;
    update(data: TenantUpdate): Promise<Tenant>;
  };
}
```

### 10.3 Real-time Updates

```typescript
// WebSocket Events
type WSEvent =
  | { type: 'invoice.created'; data: Invoice }
  | { type: 'invoice.updated'; data: Invoice }
  | { type: 'anomaly.detected'; data: Anomaly }
  | { type: 'enrichment.progress'; data: EnrichmentStatus }
  | { type: 'query.complete'; data: QueryResult };

// SSE for Dashboard Metrics
interface MetricsStream {
  connect(): EventSource;
  onMetric(callback: (metric: Metric) => void): void;
  onError(callback: (error: Error) => void): void;
}
```

### 10.4 Component Hierarchy

```
App
├── AuthProvider
│   ├── TenantProvider
│   │   ├── Layout
│   │   │   ├── Sidebar
│   │   │   │   ├── Navigation
│   │   │   │   ├── TenantSelector
│   │   │   │   └── UserMenu
│   │   │   ├── Header
│   │   │   │   ├── Breadcrumbs
│   │   │   │   ├── GlobalSearch
│   │   │   │   └── Notifications
│   │   │   ├── MainContent
│   │   │   │   ├── Dashboard
│   │   │   │   ├── InvoiceRepository
│   │   │   │   ├── ContractRepository
│   │   │   │   ├── QueryWorkbench
│   │   │   │   ├── AnomalyManager
│   │   │   │   └── Settings
│   │   │   └── AIAssistantPanel
│   │   └── NotificationToast
│   └── ErrorBoundary
└── ThemeProvider
```

---

## 11. Design System

### 11.1 Color Palette

```css
:root {
  /* Primary - Monochrome Black to Gray */
  --primary-50: #fafafa;
  --primary-100: #f4f4f5;
  --primary-200: #e4e4e7;
  --primary-300: #d4d4d8;
  --primary-400: #a1a1aa;
  --primary-500: #71717a;
  --primary-600: #52525b;
  --primary-700: #3f3f46;
  --primary-800: #27272a;
  --primary-900: #18181b;
  --primary-950: #09090b;

  /* Semantic - Muted tones to complement monochrome */
  --success: #22c55e;
  --warning: #f59e0b;
  --error: #ef4444;
  --info: #71717a;

  /* Background */
  --background: #ffffff;
  --background-subtle: #fafafa;
  --foreground: #09090b;
  --foreground-muted: #71717a;

  /* Borders */
  --border: #e4e4e7;
  --border-strong: #d4d4d8;

  /* Anomaly Severity - High contrast against monochrome */
  --critical: #dc2626;
  --high: #ea580c;
  --medium: #ca8a04;
  --low: #22c55e;

  /* Accent - Subtle blue for links/interactive elements */
  --accent: #3b82f6;
  --accent-foreground: #ffffff;
}
```

**Dark Mode:**
```css
:root.dark {
  --primary-50: #09090b;
  --primary-100: #18181b;
  --primary-200: #27272a;
  --primary-300: #3f3f46;
  --primary-400: #52525b;
  --primary-500: #71717a;
  --primary-600: #a1a1aa;
  --primary-700: #d4d4d8;
  --primary-800: #e4e4e7;
  --primary-900: #f4f4f5;
  --primary-950: #fafafa;

  --background: #09090b;
  --background-subtle: #18181b;
  --foreground: #fafafa;
  --foreground-muted: #a1a1aa;

  --border: #27272a;
  --border-strong: #3f3f46;
}
```

### 11.2 Typography

```css
:root {
  --font-sans: 'Inter', -apple-system, BlinkMacSystemFont, sans-serif;
  --font-mono: 'JetBrains Mono', 'Fira Code', monospace;

  /* Scale */
  --text-xs: 0.75rem;    /* 12px */
  --text-sm: 0.875rem;   /* 14px */
  --text-base: 1rem;     /* 16px */
  --text-lg: 1.125rem;   /* 18px */
  --text-xl: 1.25rem;    /* 20px */
  --text-2xl: 1.5rem;    /* 24px */
  --text-3xl: 1.875rem;  /* 30px */
}
```

### 11.3 Component Tokens

```css
:root {
  /* Spacing */
  --space-1: 0.25rem;
  --space-2: 0.5rem;
  --space-3: 0.75rem;
  --space-4: 1rem;
  --space-6: 1.5rem;
  --space-8: 2rem;

  /* Border Radius */
  --radius-sm: 0.25rem;
  --radius-md: 0.375rem;
  --radius-lg: 0.5rem;
  --radius-full: 9999px;

  /* Shadows */
  --shadow-sm: 0 1px 2px rgba(0, 0, 0, 0.05);
  --shadow-md: 0 4px 6px -1px rgba(0, 0, 0, 0.1);
  --shadow-lg: 0 10px 15px -3px rgba(0, 0, 0, 0.1);

  /* Sidebar */
  --sidebar-width: 256px;
  --sidebar-collapsed: 64px;

  /* Header */
  --header-height: 64px;
}
```

---

## 12. Success Metrics

### 12.1 User Engagement

| Metric | Target | Measurement |
|--------|--------|-------------|
| Daily Active Users | 70% of seats | Auth events |
| Queries Executed | 50/user/week | Query API calls |
| Reports Created | 5/user/month | Report saves |
| AI Assistant Usage | 30% of sessions | Chat API calls |

### 12.2 Business Outcomes

| Metric | Target | Measurement |
|--------|--------|-------------|
| Anomaly Detection Rate | 95% | Manual audit comparison |
| False Positive Rate | <15% | Resolution feedback |
| Time to Resolution | <24 hours | Anomaly lifecycle |
| Invoice Processing Time | <5 minutes | End-to-end tracking |

### 12.3 Technical Performance

| Metric | Target | Measurement |
|--------|--------|-------------|
| Page Load Time | <2 seconds | Core Web Vitals |
| Query Response Time | <3 seconds | API latency |
| Real-time Update Latency | <500ms | WebSocket events |
| Uptime | 99.9% | Health checks |

---

## 13. Implementation Phases

### Phase 1: Foundation (Weeks 1-4)
- [ ] Project scaffolding (Vite + React + TypeScript)
- [ ] Design system implementation (Tailwind + Shadcn)
- [ ] Authentication integration (Keycloak OIDC)
- [ ] Basic layout (sidebar, header, routing)
- [ ] API client setup

### Phase 2: Core Features (Weeks 5-8)
- [ ] Dashboard with KPI cards
- [ ] Invoice repository with data grid
- [ ] Invoice detail view
- [ ] Basic search functionality
- [ ] User management UI

### Phase 3: Analysis Tools (Weeks 9-12)
- [ ] Query Workbench with Monaco editor
- [ ] Visualization components
- [ ] Report saving and sharing
- [ ] Anomaly list and detail views

### Phase 4: Rules Engine (Weeks 13-16)

- [ ] Visual rule graph editor (React Flow)
- [ ] Node palette and configuration panels
- [ ] Decision table editor
- [ ] Rule testing interface with execution trace
- [ ] Rule version control

### Phase 5: AI & Real-time (Weeks 17-20)
- [ ] AI Assistant panel
- [ ] Real-time enrichment
- [ ] WebSocket integration
- [ ] Notification system

### Phase 6: Polish & Launch (Weeks 21-24)
- [ ] Contract repository
- [ ] Advanced filtering
- [ ] Keyboard shortcuts
- [ ] Rule execution analytics
- [ ] Performance optimization
- [ ] Documentation

---

## 14. Design Decisions

| Question | Decision | Notes |
|----------|----------|-------|
| **Offline Support** | ✅ Yes | Support offline query drafting and report editing. ML APIs require connectivity for intelligence features (anomaly scoring, predictions). Use service worker for caching. |
| **Mobile Support** | 📱 Tablet minimum | No dedicated mobile app. Responsive design with tablet (768px+) as minimum supported viewport. Desktop-first experience. |
| **Real-time Collaboration** | 🔄 Future | Yes, support collaborative report editing (Google Docs-style). Not immediate priority - target Phase 7+. |
| **Export Formats** | 📄 CSV, Excel, PDF | Standard export formats. CSV for data interchange, Excel for analysis, PDF for formal reports. |
| **Custom Visualizations** | 📊 Future | Nice-to-have. Allow custom chart types/widgets in later phases. Initial release uses predefined components. |
| **Embedded Analytics** | ✅ Yes | Support iframe-based embed URLs for dashboards. Customers can embed reports in their internal portals. Requires auth token passthrough. |

### Embed URL Format

```
https://app.fen.io/embed/report/{report_id}?token={embed_token}
https://app.fen.io/embed/dashboard/{dashboard_id}?token={embed_token}&theme=light
```

**Embed Features:**
- Read-only dashboard/report view
- Optional parameter filtering via URL query params
- Theming support (light/dark, custom brand colors)
- Responsive sizing to container
- Secure embed tokens with expiration and scope limits

---

## 15. Appendix

### A. Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| ⌘K | Global search |
| ⌘1-9 | Navigate to section |
| ⌘Enter | Execute query |
| ⌘S | Save report |
| ⌘/ | Toggle AI assistant |
| Esc | Close modal/panel |

### B. API Endpoints Reference

See [API Documentation](./api-reference.md) for complete endpoint specifications.

### C. Database Schema

See [Schema Documentation](./schema.md) for data model details.

---

*Document Version: 1.0 | Last Updated: 2026-02-03*

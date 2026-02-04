# Fenalytics Landing Page Design

**Inspired by:** FinSage landing page
**Design Philosophy:** Modern, clean, AI-forward with subtle gradients and monochrome accents
**Brand Name:** Fenalytics
**Logo Font:** Geom

---

## Design System Reference

### Logo

**Icon:** Fennec Fox silhouette (`docs/design/logo.svg`)
**Wordmark:** "Fenalytics" in Geom font with monospace styling
**Usage:** Icon + wordmark together, or icon only for compact spaces

```tsx
// Logo Component
<div className="flex items-center gap-2">
  <FennecLogo className="w-8 h-8 text-primary-900" />
  <span className="font-logo text-xl tracking-wide">Fenalytics</span>
</div>
```

### Color Palette (Monochrome Primary)

```css
:root {
  /* Primary - Monochrome */
  --primary-950: #09090b;
  --primary-900: #18181b;
  --primary-800: #27272a;
  --primary-700: #3f3f46;
  --primary-600: #52525b;
  --primary-500: #71717a;
  --primary-400: #a1a1aa;
  --primary-300: #d4d4d8;
  --primary-200: #e4e4e7;
  --primary-100: #f4f4f5;
  --primary-50: #fafafa;

  /* Hero Gradient */
  --gradient-start: #f8e1f4;   /* Soft pink */
  --gradient-mid: #e8d5f5;     /* Soft purple */
  --gradient-end: #d5e8f5;     /* Soft blue */

  /* Accent Colors (for feature cards) */
  --accent-pink: #f472b6;
  --accent-blue: #3b82f6;
  --accent-cyan: #22d3d3;
  --accent-gold: #d4a574;

  /* Semantic */
  --success: #22c55e;
  --warning: #f59e0b;
  --error: #ef4444;
}
```

### Typography

```css
:root {
  --font-logo: 'Geom', monospace;            /* Logo/Brand - Geom with mono style */
  --font-display: 'Inter', -apple-system, sans-serif;
  --font-mono: 'JetBrains Mono', monospace;

  /* Logo: Geom font + monospace style, tracking slightly expanded */
  /* Hero headline: 48-64px, bold, Inter */
  /* Section headlines: 36-42px, semibold */
  /* Body: 16-18px, regular */
  /* Captions: 14px, medium */
}
```

---

## Page Structure

```
┌─────────────────────────────────────────────────────────────────────────────┐
│ 1. HERO SECTION (with gradient background)                                  │
├─────────────────────────────────────────────────────────────────────────────┤
│ 2. FEATURES OVERVIEW (core capabilities)                                    │
├─────────────────────────────────────────────────────────────────────────────┤
│ 3. QUERY WORKBENCH DEMO (SQL interface showcase)                            │
├─────────────────────────────────────────────────────────────────────────────┤
│ 4. VISUAL RULES ENGINE (node-based editor)                                  │
├─────────────────────────────────────────────────────────────────────────────┤
│ 5. PERSONAS SECTION (3-column)                                              │
├─────────────────────────────────────────────────────────────────────────────┤
│ 6. AI ASSISTANT FEATURE (split dark/blue)                                   │
├─────────────────────────────────────────────────────────────────────────────┤
│ 7. INTEGRATIONS & SECURITY                                                  │
├─────────────────────────────────────────────────────────────────────────────┤
│ 8. CTA SECTION (dark gradient)                                              │
├─────────────────────────────────────────────────────────────────────────────┤
│ 9. FOOTER                                                                   │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Section 1: Hero Section

Full-width hero with pastel gradient background.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                                                           Anomalies         │
│  🦊 Fenalytics                                            Rules Engine      │
│                                                           Documents         │
│                         ┌────────────┐                    Dashboard         │
│                         │  (((◯)))   │                    History           │
│                         └────────────┘                    Explore           │
│                                                                             │
│              AI-powered anomaly detection for financial documents.          │
│                                                                             │
│                     Catch Invoice Anomalies                                 │
│                     Before They Cost You.                                   │
│                                                                             │
│              ┌─────────────────────────────────────────────┐               │
│              │  Analyze an invoice or ask a question...    │               │
│              └─────────────────────────────────────────────┘               │
│                         [ Ask Fenalytics → ]                                │
│                                                                             │
│  ░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░ │
│  (gradient pink → purple → blue)                                            │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Key Elements:**
- **Logo:** Fennec Fox silhouette icon (`docs/design/logo.svg`) + "Fenalytics" wordmark (Geom mono font)
- **Navigation:** Right-aligned vertical menu (underlined links)
- **Icon:** Abstract wireframe globe/circles motif
- **Headline:** Large, bold, multi-line
- **Subheadline:** Smaller, above the main headline
- **Search Input:** Centered, rounded, with placeholder text
- **CTA Button:** Dark pill button "Ask Fenalytics →"
- **Background:** Soft gradient (pink-purple-blue) with subtle noise texture

**Implementation:**
```tsx
<section className="relative min-h-screen overflow-hidden">
  {/* Gradient Background */}
  <div
    className="absolute inset-0"
    style={{
      background: 'linear-gradient(135deg, #f8e1f4 0%, #e8d5f5 50%, #d5e8f5 100%)'
    }}
  />

  {/* Content */}
  <div className="relative z-10 container mx-auto px-6 pt-20">
    {/* Header */}
    <header className="flex justify-between items-start">
      <div className="flex items-center gap-2">
        <FennecLogo className="w-8 h-8" /> {/* Fennec fox silhouette */}
        <span className="text-xl font-semibold font-logo">Fenalytics</span>
      </div>

      <nav className="text-right space-y-1">
        {['Anomalies', 'Rules Engine', 'Documents', 'Dashboard', 'History', 'Explore'].map((item) => (
          <a key={item} className="block text-sm hover:underline">{item}</a>
        ))}
      </nav>
    </header>

    {/* Hero Content */}
    <div className="text-center mt-24 max-w-3xl mx-auto">
      <div className="mb-8">
        <WireframeGlobe className="w-24 h-24 mx-auto" />
      </div>

      <p className="text-sm text-primary-600 mb-4">
        AI-powered insights for invoices and contracts.
      </p>

      <h1 className="text-5xl md:text-6xl font-bold tracking-tight mb-8">
        Intelligent Document<br />
        Analysis Platform.
      </h1>

      <div className="max-w-xl mx-auto">
        <input
          type="text"
          placeholder="Analyze an invoice or ask a question..."
          className="w-full px-6 py-4 rounded-full border border-primary-300
                     focus:outline-none focus:ring-2 focus:ring-primary-500"
        />
        <button className="mt-4 px-8 py-3 bg-primary-900 text-white rounded-full
                          font-medium hover:bg-primary-800 transition">
          Ask Fenalytics →
        </button>
      </div>
    </div>
  </div>
</section>
```

---

## Section 2: Features Overview

Core product capabilities in a grid layout.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                               • Platform                                    │
│                                                                             │
│                    Everything You Need to                                   │
│                    Validate Financial Documents.                            │
│                                                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌──────────────────────┐  ┌──────────────────────┐  ┌──────────────────┐  │
│  │ 🔍 Query Workbench   │  │ 📊 Visual Rules      │  │ 🤖 AI Assistant  │  │
│  │                      │  │                      │  │                  │  │
│  │ SQL-like interface   │  │ Node-based rule      │  │ Context-aware    │  │
│  │ to query invoices,   │  │ editor for building  │  │ agent that       │  │
│  │ contracts, and       │  │ validation logic     │  │ explains         │  │
│  │ anomalies.           │  │ visually.            │  │ anomalies.       │  │
│  └──────────────────────┘  └──────────────────────┘  └──────────────────┘  │
│                                                                             │
│  ┌──────────────────────┐  ┌──────────────────────┐  ┌──────────────────┐  │
│  │ 📈 Statistical       │  │ 📄 Document          │  │ 🔔 Real-time     │  │
│  │    Baselines         │  │    Repositories      │  │    Alerts        │  │
│  │                      │  │                      │  │                  │  │
│  │ Auto-calculated      │  │ Centralized          │  │ Instant          │  │
│  │ vendor/item          │  │ management for       │  │ notifications    │  │
│  │ baselines for        │  │ invoices and         │  │ for detected     │  │
│  │ deviation detection. │  │ contracts.           │  │ anomalies.       │  │
│  └──────────────────────┘  └──────────────────────┘  └──────────────────┘  │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Key Elements:**
- 2x3 grid of feature cards
- Icon + title + short description per card
- Clean, minimal styling

---

## Section 3: Query Workbench Demo

Interactive SQL interface showcase with live results.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                               • Query Workbench                             │
│                                                                             │
│  Query Your Documents              Write SQL-like queries to analyze        │
│  Like a Database.                  invoices, contracts, and anomalies.      │
│                                    Get instant results with visualizations. │
│                                                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│  ┌────────────────────────────────────────────────────────────────────┐    │
│  │ ● ● ●                                                   [▶ Run]    │    │
│  ├────────────────────────────────────────────────────────────────────┤    │
│  │                                                                    │    │
│  │  SELECT vendor_name, COUNT(*) as invoice_count,                    │    │
│  │         AVG(total_amount) as avg_amount,                           │    │
│  │         COUNT(anomaly_id) as anomaly_count                         │    │
│  │  FROM invoices                                                     │    │
│  │  LEFT JOIN anomalies ON invoices.id = anomalies.document_id        │    │
│  │  WHERE invoice_date >= '2024-01-01'                                │    │
│  │  GROUP BY vendor_name                                              │    │
│  │  ORDER BY anomaly_count DESC                                       │    │
│  │  LIMIT 10;                                                         │    │
│  │                                                                    │    │
│  └────────────────────────────────────────────────────────────────────┘    │
│                                                                             │
│  Results (156 rows, 23ms)                                                   │
│  ┌──────────────────┬───────────────┬────────────┬───────────────┐         │
│  │ vendor_name      │ invoice_count │ avg_amount │ anomaly_count │         │
│  ├──────────────────┼───────────────┼────────────┼───────────────┤         │
│  │ TechParts Inc    │ 47            │ $12,450    │ 8             │         │
│  │ GlobalSupply Co  │ 32            │ $8,200     │ 5             │         │
│  │ Acme Corp        │ 28            │ $15,600    │ 3             │         │
│  └──────────────────┴───────────────┴────────────┴───────────────┘         │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Key Elements:**

- **Section Label:** "• Query Workbench" with bullet
- **Two-column header:** Left headline, right description
- **Code Editor:** Monaco-style SQL editor with syntax highlighting
- **Results Table:** Live query results with row count and timing
- **Run Button:** Execute query action

---

## Section 4: Visual Rules Engine

Node-based rule editor showcase with flow diagram.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                               • Rules Engine                                │
│                                                                             │
│  Build Validation Logic              Create powerful validation rules       │
│  Visually.                           without writing code. Connect nodes    │
│                                      to build complex detection pipelines.  │
│                                                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│    ┌──────────────┐                                                         │
│    │ 📥 Invoice   │                                                         │
│    │    Input     │                                                         │
│    └──────┬───────┘                                                         │
│           │                                                                 │
│           ▼                                                                 │
│    ┌──────────────┐                                                         │
│    │ 🔀 Check     │                                                         │
│    │   Amount     │                                                         │
│    └──────┬───────┘                                                         │
│           │                                                                 │
│     ┌─────┼─────┐                                                           │
│     ▼     ▼     ▼                                                           │
│  ┌─────┐ ┌─────┐ ┌─────┐                                                    │
│  │<$1K │ │$1-50K│ │>$50K│                                                   │
│  │ ✅  │ │  ⚠️ │ │ 🚨 │                                                    │
│  └──┬──┘ └──┬──┘ └──┬──┘                                                    │
│     └───────┼───────┘                                                       │
│             ▼                                                               │
│      ┌──────────────┐                                                       │
│      │ 📤 Output    │                                                       │
│      └──────────────┘                                                       │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Key Elements:**

- **Section Label:** "• Rules Engine" with bullet
- **Headline:** Bold, left-aligned with description on right
- **Flow Diagram:** Interactive node-based visualization
- **Node Types:** Input, Condition, Branch, Output with status icons

**Implementation:**
```tsx
<section className="py-24 bg-primary-50">
  <div className="container mx-auto px-6">
    <div className="grid md:grid-cols-2 gap-12 items-center">
      <div>
        <span className="text-sm font-medium">• Rules Engine</span>
        <h2 className="text-4xl font-bold mt-4 mb-4">
          Build Validation Logic Visually.
        </h2>
        <p className="text-primary-600">
          Create powerful validation rules without writing code.
          Connect nodes to build complex detection pipelines.
        </p>
        <button className="mt-6 text-primary-900 font-medium hover:underline">
          Learn more →
        </button>
      </div>
      <div className="bg-white rounded-xl shadow-lg p-8">
        <RuleFlowDiagram /> {/* React Flow component */}
      </div>
    </div>
  </div>
</section>
```

---

## Section 5: Personas Section

Three-column layout highlighting different user types.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                                • Roles                                      │
│                                                                             │
│                    Solutions for Every Role.                                │
│                                                                             │
│      Whether you analyze, audit, or manage, Fenalytics integrates           │
│                       into your workflow.                                   │
│                                                                             │
├─────────────────────┬─────────────────────┬─────────────────────────────────┤
│                     │                     │                                 │
│  ▣ Finance Analysts │  ▤ Auditors         │  ▥ Procurement Managers        │
│                     │                     │                                 │
│  Real-time anomaly  │  Comprehensive      │  Contract compliance           │
│  detection and      │  audit trails and   │  monitoring and vendor         │
│  pattern analysis.  │  compliance tools.  │  spend optimization.           │
│                     │                     │                                 │
└─────────────────────┴─────────────────────┴─────────────────────────────────┘
```

**Key Elements:**
- **Icon Badges:** Dark rounded squares with icon
- **Vertical Dividers:** Thin lines between columns
- **Role Title:** Bold, medium size
- **Description:** Regular weight, 2-3 lines

---

## Section 6: AI Assistant Feature

Split-screen with dark left side (visual) and colored right side (content).

```
┌─────────────────────────────────────────────────────────────────────────────┐
│ ┌─────────────────────────────┬─────────────────────────────────────────┐  │
│ │                             │                                         │  │
│ │                             │  • FenalyticsGPT                               │  │
│ │        ◉                    │                                         │  │
│ │       ╱ ╲                   │  AI Tools for Smarter                   │  │
│ │      ╱   ╲                  │  Document Analysis.                     │  │
│ │     ◌─────◌                 │                                         │  │
│ │                             │  Your 24/7 AI analyst for instant       │  │
│ │  (Glowing compass visual)   │  document insights.                     │  │
│ │                             │                                         │  │
│ │                             │  → Live anomaly detection               │  │
│ │                             │  → Pattern recognition                  │  │
│ │                             │  → Just ask, get answers                │  │
│ │                             │                                         │  │
│ │                             │  Start Chatting →                       │  │
│ │                             │                                         │  │
│ └─────────────────────────────┴─────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Key Elements:**
- **Left Panel:** Dark/black with abstract visual (compass/radar)
- **Right Panel:** Blue/primary color background
- **Feature List:** Arrow-prefixed bullet points
- **CTA:** "Start Chatting →" link

---

## Section 7: Integrations & Security

Trust badges and integration options.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           • Enterprise Ready                                │
│                                                                             │
│              Secure, Scalable, and Ready                                    │
│              for Your Organization.                                         │
│                                                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  Security                           Integrations                            │
│  ┌──────────────────────────────┐  ┌──────────────────────────────┐        │
│  │ 🔒 SOC 2 Type II Compliant   │  │ 📤 REST API                  │        │
│  │ 🔐 SSO/OIDC Authentication   │  │ 📊 Excel Export              │        │
│  │ 🏢 Multi-tenant Isolation    │  │ 🔗 Webhook Notifications     │        │
│  │ 📋 Audit Logging             │  │ 📁 S3/GCS Document Storage   │        │
│  │ 🗝️ Role-based Access         │  │ 🔄 ERP Connectors            │        │
│  └──────────────────────────────┘  └──────────────────────────────┘        │
│                                                                             │
│  Deployment Options                                                         │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐                           │
│  │ ☁️ Cloud    │ │ 🏠 On-Prem  │ │ 🔒 Private  │                           │
│  │   Hosted    │ │   Deploy    │ │   Cloud     │                           │
│  └─────────────┘ └─────────────┘ └─────────────┘                           │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Key Elements:**

- Two-column layout for Security and Integrations
- Icon + feature name format
- Deployment option cards at bottom
- Clean monochrome styling

---

## Section 8: CTA Section

Dark gradient banner with centered messaging.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│ ░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░ │
│ ░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░ │
│ ░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░ │
│ ░░░                                                                   ░░░ │
│ ░░░               Stop losing money to                                ░░░ │
│ ░░░               invoice anomalies.                                  ░░░ │
│ ░░░                                                                   ░░░ │
│ ░░░    Fenalytics detects pricing anomalies, duplicate invoices,      ░░░ │
│ ░░░    and contract violations automatically. Start catching          ░░░ │
│ ░░░    costly errors before they hit your bottom line.                ░░░ │
│ ░░░                                                                   ░░░ │
│ ░░░                    [ Get Started ]                                ░░░ │
│ ░░░                                                                   ░░░ │
│ ░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░ │
│ (gradient: blue-left → black-center → pink-right)                         │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Key Elements:**
- **Background:** Dark with subtle blue/pink gradient corners
- **Headline:** Large, white, multi-line
- **Body Text:** Smaller, light gray
- **CTA Button:** White pill button with dark text

**Implementation:**
```tsx
<section className="py-24">
  <div
    className="mx-6 rounded-2xl py-20 px-8 text-center"
    style={{
      background: 'linear-gradient(135deg, #1e3a5f 0%, #09090b 50%, #2d1b3d 100%)'
    }}
  >
    <h2 className="text-4xl md:text-5xl font-bold text-white mb-6">
      Analyze smarter, let<br />
      AI handle the details.
    </h2>
    <p className="text-primary-400 max-w-2xl mx-auto mb-8">
      As you navigate complex invoice and contract data, our AI agents provide
      real-time analysis. They help you make confident, data-driven decisions.
    </p>
    <button className="px-8 py-3 bg-white text-primary-900 rounded-full
                       font-medium hover:bg-primary-100 transition">
      Get Started
    </button>
  </div>
</section>
```

---

## Section 9: Footer

Multi-column footer with large brand wordmark.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                                                                             │
│  🦊 Fenalytics       Navigation       Tools           Resources            │
│                                                                             │
│                      Home              Dashboard       Documentation        │
│  Privacy Policy      Discover          Anomaly View    Academy             │
│  Terms of Use        Documents         Query Builder   API Reference       │
│                      Contracts         Rules Engine    Pricing             │
│                      Reports           Baselines       Blog                │
│                                                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│                                                                             │
│                           F  e  n                                           │
│                    (oversized wordmark)                                     │
│                                                                             │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Key Elements:**
- **Logo:** Top left with product name
- **Link Columns:** Navigation, Tools, Resources
- **Legal Links:** Privacy Policy, Terms of Use
- **Brand Mark:** Oversized "Fenalytics" text at bottom (Geom mono font)

---

## Component Library

### Buttons

```tsx
// Primary Button (dark)
<button className="px-6 py-3 bg-primary-900 text-white rounded-full font-medium
                   hover:bg-primary-800 transition-colors">
  Get Started
</button>

// Secondary Button (outline)
<button className="px-6 py-3 border border-primary-300 rounded-full font-medium
                   hover:bg-primary-100 transition-colors">
  Learn More
</button>

// Ghost Button (text link)
<button className="font-medium hover:underline">
  Execute →
</button>
```

### Tabs

```tsx
<div className="flex gap-1 p-1 bg-primary-100 rounded-full">
  {tabs.map((tab) => (
    <button
      key={tab.id}
      className={cn(
        "px-4 py-2 rounded-full text-sm font-medium transition-colors",
        activeTab === tab.id
          ? "bg-primary-900 text-white"
          : "text-primary-600 hover:text-primary-900"
      )}
    >
      {tab.label}
    </button>
  ))}
</div>
```

### Cards

```tsx
// Feature Card (colored)
<div className="rounded-xl p-6 h-80 flex flex-col justify-between"
     style={{ backgroundColor: cardColor }}>
  <div>
    <h3 className="text-xl font-bold mb-2">{title}</h3>
    <p className="text-sm opacity-80">{description}</p>
  </div>
  <div className="flex justify-between items-end">
    <a className="font-medium hover:underline">Start Learning →</a>
    <GeometricArt className="w-24 h-24 opacity-20" />
  </div>
</div>

// Process Step Card
<div className="border-l-2 border-primary-200 pl-4 py-3">
  <div className="flex justify-between items-center mb-1">
    <span className="font-medium">{stepName}</span>
    <span className={cn(
      "text-xs px-2 py-1 rounded-full",
      status === 'Complete' ? 'bg-success/10 text-success' :
      status === 'Processing' ? 'bg-warning/10 text-warning' :
      'bg-primary-100 text-primary-500'
    )}>{status}</span>
  </div>
  <p className="text-sm text-primary-500">{description}</p>
</div>
```

### Input

```tsx
<div className="relative">
  <input
    type="text"
    placeholder="Analyze an invoice or ask a question..."
    className="w-full px-6 py-4 pr-12 rounded-full border border-primary-200
               bg-white focus:outline-none focus:ring-2 focus:ring-primary-500
               focus:border-transparent"
  />
  <button className="absolute right-2 top-1/2 -translate-y-1/2
                     px-4 py-2 bg-primary-900 text-white rounded-full text-sm">
    Ask Fenalytics →
  </button>
</div>
```

---

## Animations

### Scroll Reveal
```tsx
// Using Framer Motion
<motion.div
  initial={{ opacity: 0, y: 20 }}
  whileInView={{ opacity: 1, y: 0 }}
  transition={{ duration: 0.6 }}
  viewport={{ once: true }}
>
  {content}
</motion.div>
```

### Ticker Marquee
```css
@keyframes marquee {
  0% { transform: translateX(0); }
  100% { transform: translateX(-50%); }
}

.animate-marquee {
  animation: marquee 30s linear infinite;
}
```

### Gradient Animation
```css
@keyframes gradient-shift {
  0% { background-position: 0% 50%; }
  50% { background-position: 100% 50%; }
  100% { background-position: 0% 50%; }
}

.animated-gradient {
  background-size: 200% 200%;
  animation: gradient-shift 15s ease infinite;
}
```

---

## Responsive Breakpoints

| Breakpoint | Width | Layout Changes |
|------------|-------|----------------|
| Mobile | < 768px | Single column, stacked sections |
| Tablet | 768px - 1024px | Two columns where applicable |
| Desktop | > 1024px | Full layout as designed |

---

## Implementation Priority

1. **Phase 1:** Hero section + Navigation
2. **Phase 2:** Features Overview grid
3. **Phase 3:** CTA section + Footer
4. **Phase 4:** Query Workbench demo
5. **Phase 5:** Visual Rules Engine showcase
6. **Phase 6:** Personas + AI Assistant sections
7. **Phase 7:** Integrations & Security
8. **Phase 8:** Animations + Polish

---

## Login Page

Split-screen login with product showcase on the right.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                                                                             │
│  ┌─────────────────────────────┬───────────────────────────────────────────┐│
│  │                             │                                           ││
│  │                             │  ┌─────────────────────────────────────┐  ││
│  │  Welcome back               │  │  Good Morning, Sarah              ▼ │  ││
│  │                             │  │  Summary of document analysis       │  ││
│  │  Continue with one of the   │  ├─────────────────────────────────────┤  ││
│  │  following options          │  │                                     │  ││
│  │                             │  │  ┌─────┐ ┌─────┐ ┌─────┐ ┌─────┐   │  ││
│  │  Email                      │  │  │ 847 │ │$2.1M│ │ 6.5%│ │ 127 │   │  ││
│  │  ┌───────────────────────┐  │  │  │Docs │ │Total│ │Anom.│ │Rules│   │  ││
│  │  │ Email Address         │  │  │  └─────┘ └─────┘ └─────┘ └─────┘   │  ││
│  │  └───────────────────────┘  │  │                                     │  ││
│  │                             │  │  [Anomalies] [Rules] [Baselines]    │  ││
│  │  Password                   │  │                                     │  ││
│  │  ┌───────────────────────┐  │  │  Recent Anomalies         View All  │  ││
│  │  │ ●●●●●●●●●●        👁️  │  │  │  ┌─────────────────────────────┐   │  ││
│  │  └───────────────────────┘  │  │  │ INV-2024-889  🚨 High Risk  │   │  ││
│  │                             │  │  │ TechParts Inc  $45,000      │   │  ││
│  │          Forgot Password?   │  │  └─────────────────────────────┘   │  ││
│  │                             │  │                                     │  ││
│  │  ┌───────────────────────┐  │  │  ┌─────────────────────────────────┐│  ││
│  │  │      Sign in          │  │  │  │         ╱╲                     ││  ││
│  │  └───────────────────────┘  │  │  │        ╱  ╲    Anomaly Trend   ││  ││
│  │                             │  │  │   ────╱────╲────────────────   ││  ││
│  │  ┌───────────────────────┐  │  │  │   Jan  Feb  Mar  Apr  May      ││  ││
│  │  │ G  Continue with Google│  │  │  └─────────────────────────────────┘│  ││
│  │  └───────────────────────┘  │  │                                     │  ││
│  │                             │  └─────────────────────────────────────┘  ││
│  │  Already have an account?   │                                           ││
│  │  Log In                     │  (Product screenshot / animated demo)     ││
│  │                             │                                           ││
│  └─────────────────────────────┴───────────────────────────────────────────┘│
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Login Page Layout

**Left Panel (40% width):**
- Centered login form
- "Welcome back" heading (24px, bold)
- "Continue with one of the following options" subtext (14px, muted)
- Email input with label
- Password input with visibility toggle
- "Forgot Password?" link (right-aligned)
- Primary "Sign in" button (dark, pill shape, full width)
- "Continue with Google" button (outline style, full width)
- "Already have an account? Log In" link at bottom

**Right Panel (60% width):**
- Product screenshot showcase
- Shows Fenalytics dashboard/analytics view OR rules engine
- Future: Animated carousel or live demo
- Subtle background (light gray or gradient)

### Implementation

```tsx
<div className="min-h-screen flex">
  {/* Left Panel - Login Form */}
  <div className="w-2/5 flex items-center justify-center p-12 bg-white">
    <div className="w-full max-w-md">
      <h1 className="text-2xl font-bold mb-2">Welcome back</h1>
      <p className="text-primary-500 mb-8">
        Continue with one of the following options
      </p>

      <form className="space-y-6">
        <div>
          <label className="block text-sm font-medium mb-2">Email</label>
          <input
            type="email"
            placeholder="Email Address"
            className="w-full px-4 py-3 border border-primary-200 rounded-lg
                       focus:outline-none focus:ring-2 focus:ring-primary-500"
          />
        </div>

        <div>
          <label className="block text-sm font-medium mb-2">Password</label>
          <div className="relative">
            <input
              type="password"
              placeholder="••••••••••"
              className="w-full px-4 py-3 border border-primary-200 rounded-lg
                         focus:outline-none focus:ring-2 focus:ring-primary-500"
            />
            <button type="button" className="absolute right-3 top-1/2 -translate-y-1/2">
              <EyeIcon className="w-5 h-5 text-primary-400" />
            </button>
          </div>
          <a href="#" className="block text-right text-sm mt-2 hover:underline">
            Forgot Password?
          </a>
        </div>

        <button
          type="submit"
          className="w-full py-3 bg-primary-900 text-white rounded-full
                     font-medium hover:bg-primary-800 transition"
        >
          Sign in
        </button>

        <button
          type="button"
          className="w-full py-3 border border-primary-200 rounded-full
                     font-medium flex items-center justify-center gap-2
                     hover:bg-primary-50 transition"
        >
          <GoogleIcon className="w-5 h-5" />
          Continue with Google
        </button>
      </form>

      <p className="text-center mt-8 text-sm text-primary-500">
        Don't have an account?{' '}
        <a href="#" className="text-primary-900 font-medium hover:underline">
          Sign Up
        </a>
      </p>
    </div>
  </div>

  {/* Right Panel - Product Showcase */}
  <div className="w-3/5 bg-primary-50 p-8 flex items-center justify-center">
    <div className="relative w-full max-w-3xl">
      {/* Product screenshot with shadow */}
      <div className="rounded-2xl shadow-2xl overflow-hidden bg-white">
        <ProductScreenshot />
        {/* Or animated demo component */}
      </div>
    </div>
  </div>
</div>
```

### Product Showcase Options

The right panel can display (rotatable/animated in future):

1. **Analytics Dashboard** - KPI cards, anomaly trend chart, recent alerts
2. **Rules Engine** - Visual rule graph with connected nodes
3. **Query Workbench** - SQL editor with results table
4. **Document View** - Invoice with highlighted anomalies

### Sign Up Variation

Same layout with additional fields:
- Full Name
- Company/Organization
- Email
- Password
- "Create Account" CTA
- Terms & Privacy checkbox

---

## Assets Required

- [x] Logo SVG - Fennec Fox silhouette (`docs/design/logo.svg`) ✓
- [ ] "Fenalytics" wordmark in Geom mono font
- [ ] Wireframe globe/circles icon for hero section
- [ ] Geometric patterns for resource cards (4 variants)
- [ ] Compass/radar visual for AI section
- [ ] Icon set for persona badges

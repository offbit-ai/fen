import { Link } from 'react-router-dom'
import { Logo, FennecIcon } from '@/components/shared/logo'
import {
  Database,
  GitBranch,
  Bot,
  BarChart3,
  FolderOpen,
  Bell,
  ArrowRight,
  Shield,
  KeyRound,
  Building2,
  ClipboardList,
  Users2,
  Webhook,
  FileSpreadsheet,
  HardDrive,
  RefreshCw,
  Cloud,
  Server,
  Lock,
  Play,
  MessageSquare,
  Sparkles,
  Eye,
  Download,
  Settings,
  Upload,
} from 'lucide-react'

// ─── Hero Section ──────────────────────────────────────────────────────────
function HeroSection() {
  return (
    <section className="relative min-h-screen overflow-hidden">
      {/* Gradient Background */}
      <div
        className="absolute inset-0 animated-gradient"
        style={{
          background: 'linear-gradient(135deg, #f8e1f4 0%, #e8d5f5 35%, #d5e8f5 65%, #dff0f5 100%)',
          backgroundSize: '200% 200%',
        }}
      />

      {/* Subtle noise texture overlay */}
      <div className="absolute inset-0 opacity-[0.03]" style={{
        backgroundImage: `url("data:image/svg+xml,%3Csvg viewBox='0 0 256 256' xmlns='http://www.w3.org/2000/svg'%3E%3Cfilter id='n'%3E%3CfeTurbulence type='fractalNoise' baseFrequency='0.9' numOctaves='4' stitchTiles='stitch'/%3E%3C/filter%3E%3Crect width='100%25' height='100%25' filter='url(%23n)'/%3E%3C/svg%3E")`,
      }} />

      <div className="relative z-10 container mx-auto px-6 pt-8 pb-20">
        {/* Header */}
        <header className="flex justify-between items-start">
          <Logo size="lg" />

          <nav className="hidden md:flex text-right space-y-1.5 flex-col">
            {[
              { label: 'Anomalies', href: '#features' },
              { label: 'Rules Engine', href: '#rules' },
              { label: 'Documents', href: '#workbench' },
              { label: 'Dashboard', href: '/app/dashboard' },
              { label: 'History', href: '#enterprise' },
              { label: 'Explore', href: '#personas' },
            ].map((item) => (
              <a
                key={item.label}
                href={item.href}
                className="block text-sm text-primary-700 hover:text-primary-900 hover:underline underline-offset-4 transition-colors"
              >
                {item.label}
              </a>
            ))}
          </nav>
        </header>

        {/* Hero Content */}
        <div className="text-center mt-20 md:mt-28 max-w-3xl mx-auto">
          {/* Wireframe Globe / Abstract Motif */}
          <div className="mb-8 flex justify-center">
            <div className="relative w-24 h-24">
              <div className="absolute inset-0 rounded-full border-2 border-primary-300 opacity-60" />
              <div className="absolute inset-3 rounded-full border border-primary-400 opacity-40" />
              <div className="absolute inset-6 rounded-full border border-primary-500 opacity-30" />
              <div className="absolute inset-0 flex items-center justify-center">
                <FennecIcon className="w-10 h-10 text-primary-800" />
              </div>
            </div>
          </div>

          <p className="text-sm text-primary-600 mb-4 tracking-wide uppercase">
            AI-powered insights for invoices and contracts.
          </p>

          <h1 className="text-5xl md:text-6xl lg:text-7xl font-bold tracking-tight text-primary-950 mb-8 leading-[1.1]">
            Intelligent Document<br />
            Analysis Platform.
          </h1>

          <div className="max-w-xl mx-auto flex flex-col sm:flex-row items-center justify-center gap-4">
            <Link
              to="/login"
              className="inline-flex items-center gap-2 px-8 py-3 bg-primary-900 text-white rounded-full
                         font-medium hover:bg-primary-800 transition-all hover:shadow-lg hover:shadow-primary-900/20"
            >
              Get Started
              <ArrowRight className="w-4 h-4" />
            </Link>
            <a
              href="#features"
              className="inline-flex items-center gap-2 px-8 py-3 border border-primary-300 rounded-full
                         font-medium text-primary-800 hover:bg-white/60 transition-all"
            >
              Learn More
            </a>
          </div>
        </div>
      </div>

      {/* Bottom gradient fade */}
      <div className="absolute bottom-0 left-0 right-0 h-32 bg-gradient-to-t from-white to-transparent" />
    </section>
  )
}

// ─── Features Overview ─────────────────────────────────────────────────────
const features = [
  {
    icon: Database,
    title: 'Query Workbench',
    description: 'SQL-like interface to query invoices, contracts, and anomalies with instant results and visualizations.',
  },
  {
    icon: GitBranch,
    title: 'Visual Rules Engine',
    description: 'Node-based rule editor for building validation logic visually. No coding required.',
  },
  {
    icon: Bot,
    title: 'AI Assistant',
    description: 'Context-aware agent that explains anomalies, answers questions, and suggests next steps.',
  },
  {
    icon: BarChart3,
    title: 'Statistical Baselines',
    description: 'Auto-calculated vendor and item baselines for detecting pricing deviations and trends.',
  },
  {
    icon: FolderOpen,
    title: 'Document Repositories',
    description: 'Centralized management for invoices and contracts with version tracking and search.',
  },
  {
    icon: Bell,
    title: 'Real-time Alerts',
    description: 'Instant notifications for detected anomalies, compliance violations, and threshold breaches.',
  },
]

function FeaturesSection() {
  return (
    <section id="features" className="py-24 bg-white">
      <div className="container mx-auto px-6">
        <div className="text-center mb-16">
          <span className="text-sm font-medium text-primary-500">
            &bull; Platform
          </span>
          <h2 className="text-4xl md:text-5xl font-bold text-primary-950 mt-4 mb-4">
            Everything You Need to<br />
            Validate Financial Documents.
          </h2>
        </div>

        <div className="grid md:grid-cols-2 lg:grid-cols-3 gap-6 max-w-6xl mx-auto">
          {features.map((feature) => (
            <div
              key={feature.title}
              className="group p-6 rounded-xl border border-primary-100 hover:border-primary-200
                         hover:shadow-lg transition-all duration-300 bg-white"
            >
              <div className="w-10 h-10 rounded-lg bg-primary-100 flex items-center justify-center mb-4
                              group-hover:bg-primary-900 group-hover:text-white transition-colors duration-300">
                <feature.icon className="w-5 h-5" />
              </div>
              <h3 className="text-lg font-semibold text-primary-900 mb-2">
                {feature.title}
              </h3>
              <p className="text-sm text-primary-500 leading-relaxed">
                {feature.description}
              </p>
            </div>
          ))}
        </div>
      </div>
    </section>
  )
}

// ─── Query Workbench Demo ──────────────────────────────────────────────────
const sqlCode = `SELECT vendor_name, COUNT(*) as invoice_count,
       AVG(total_amount) as avg_amount,
       COUNT(anomaly_id) as anomaly_count
FROM invoices
LEFT JOIN anomalies ON invoices.id = anomalies.document_id
WHERE invoice_date >= '2024-01-01'
GROUP BY vendor_name
ORDER BY anomaly_count DESC
LIMIT 10;`

const queryResults = [
  { vendor: 'TechParts Inc', count: 47, avg: '$12,450', anomalies: 8 },
  { vendor: 'GlobalSupply Co', count: 32, avg: '$8,200', anomalies: 5 },
  { vendor: 'Acme Corp', count: 28, avg: '$15,600', anomalies: 3 },
  { vendor: 'Widget Industries', count: 24, avg: '$6,800', anomalies: 2 },
  { vendor: 'Supplies Direct', count: 19, avg: '$3,400', anomalies: 1 },
]

function QueryWorkbenchSection() {
  return (
    <section id="workbench" className="py-24 bg-primary-50/50">
      <div className="container mx-auto px-6">
        <div className="grid md:grid-cols-2 gap-12 items-start mb-16">
          <div>
            <span className="text-sm font-medium text-primary-500">
              &bull; Query Workbench
            </span>
            <h2 className="text-4xl font-bold text-primary-950 mt-4 mb-4">
              Query Your Documents<br />
              Like a Database.
            </h2>
          </div>
          <div className="flex items-end">
            <p className="text-primary-600 leading-relaxed">
              Write SQL-like queries to analyze invoices, contracts, and anomalies.
              Get instant results with visualizations. FenQL extends standard SQL with
              domain-specific functions for anomaly detection and baseline comparison.
            </p>
          </div>
        </div>

        {/* Code Editor Mock */}
        <div className="max-w-5xl mx-auto">
          <div className="rounded-xl overflow-hidden shadow-2xl border border-primary-200">
            {/* Title bar */}
            <div className="bg-primary-900 px-4 py-3 flex items-center justify-between">
              <div className="flex items-center gap-2">
                <div className="w-3 h-3 rounded-full bg-red-400" />
                <div className="w-3 h-3 rounded-full bg-yellow-400" />
                <div className="w-3 h-3 rounded-full bg-green-400" />
                <span className="ml-3 text-xs text-primary-400 font-mono">vendor_analysis.sql</span>
              </div>
              <button className="flex items-center gap-1.5 px-3 py-1 bg-green-500/20 text-green-400 rounded text-xs font-medium hover:bg-green-500/30 transition">
                <Play className="w-3 h-3" />
                Run
              </button>
            </div>

            {/* Code area */}
            <div className="bg-primary-950 p-6 font-mono text-sm">
              <pre className="text-primary-300 leading-relaxed">
                <code>
                  {sqlCode.split('\n').map((line, i) => (
                    <div key={i} className="flex">
                      <span className="text-primary-600 select-none w-8 text-right mr-4">{i + 1}</span>
                      <span dangerouslySetInnerHTML={{
                        __html: line
                          .replace(/(SELECT|FROM|LEFT JOIN|ON|WHERE|GROUP BY|ORDER BY|LIMIT|COUNT|AVG|DESC|AS)/g,
                            '<span class="text-blue-400">$1</span>')
                          .replace(/('[\d-]+')/g, '<span class="text-green-400">$1</span>')
                          .replace(/(vendor_name|invoice_count|avg_amount|anomaly_count|invoice_date|total_amount|anomaly_id|document_id)/g,
                            '<span class="text-purple-300">$1</span>')
                          .replace(/(invoices|anomalies)/g, '<span class="text-yellow-300">$1</span>')
                          .replace(/(10;?)/g, '<span class="text-orange-300">$1</span>')
                      }} />
                    </div>
                  ))}
                </code>
              </pre>
            </div>

            {/* Results area */}
            <div className="bg-white p-6">
              <div className="flex items-center gap-3 mb-4">
                <span className="text-xs font-medium text-primary-900">Results</span>
                <span className="text-xs text-primary-400">156 rows &middot; 23ms</span>
              </div>
              <div className="overflow-x-auto">
                <table className="w-full text-sm">
                  <thead>
                    <tr className="border-b border-primary-200">
                      <th className="text-left py-2 px-3 font-medium text-primary-600">vendor_name</th>
                      <th className="text-right py-2 px-3 font-medium text-primary-600">invoice_count</th>
                      <th className="text-right py-2 px-3 font-medium text-primary-600">avg_amount</th>
                      <th className="text-right py-2 px-3 font-medium text-primary-600">anomaly_count</th>
                    </tr>
                  </thead>
                  <tbody>
                    {queryResults.map((row) => (
                      <tr key={row.vendor} className="border-b border-primary-100 hover:bg-primary-50">
                        <td className="py-2.5 px-3 text-primary-900 font-medium">{row.vendor}</td>
                        <td className="py-2.5 px-3 text-right text-primary-700 font-mono">{row.count}</td>
                        <td className="py-2.5 px-3 text-right text-primary-700 font-mono">{row.avg}</td>
                        <td className="py-2.5 px-3 text-right">
                          <span className={`font-mono ${row.anomalies > 5 ? 'text-critical font-medium' : row.anomalies > 2 ? 'text-warning' : 'text-primary-700'}`}>
                            {row.anomalies}
                          </span>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </div>
          </div>
        </div>
      </div>
    </section>
  )
}

// ─── Visual Rules Engine ───────────────────────────────────────────────────
function RulesEngineSection() {
  return (
    <section id="rules" className="py-24 bg-white">
      <div className="container mx-auto px-6">
        <div className="grid md:grid-cols-2 gap-12 items-center max-w-6xl mx-auto">
          <div>
            <span className="text-sm font-medium text-primary-500">
              &bull; Rules Engine
            </span>
            <h2 className="text-4xl font-bold text-primary-950 mt-4 mb-4">
              Build Validation Logic<br />
              Visually.
            </h2>
            <p className="text-primary-600 leading-relaxed mb-6">
              Create powerful validation rules without writing code.
              Connect nodes to build complex detection pipelines.
              Test against real data and see execution traces in real time.
            </p>
            <a href="#" className="inline-flex items-center text-primary-900 font-medium hover:underline underline-offset-4">
              Learn more <ArrowRight className="w-4 h-4 ml-1" />
            </a>
          </div>

          {/* Flow Diagram */}
          <div className="bg-primary-50 rounded-xl p-8 shadow-lg border border-primary-100">
            <div className="flex flex-col items-center gap-3">
              {/* Input Node */}
              <FlowNode icon={<Download className="w-4 h-4" />} label="Invoice Input" color="bg-blue-50 border-blue-200" />
              <FlowArrow />

              {/* Extract Node */}
              <FlowNode icon={<Settings className="w-4 h-4" />} label="Extract Fields" color="bg-purple-50 border-purple-200" />
              <FlowArrow />

              {/* Branch Node */}
              <FlowNode icon={<GitBranch className="w-4 h-4" />} label="Check Amount" color="bg-amber-50 border-amber-200" />

              {/* Branches */}
              <div className="flex items-start gap-4 w-full justify-center mt-1">
                <div className="flex flex-col items-center gap-2">
                  <div className="w-px h-4 bg-primary-300" />
                  <div className="px-3 py-1.5 rounded-lg bg-green-50 border border-green-200 text-xs font-medium text-green-800">
                    &lt; $1K
                  </div>
                  <div className="text-xs text-green-600">Low Risk</div>
                </div>
                <div className="flex flex-col items-center gap-2">
                  <div className="w-px h-4 bg-primary-300" />
                  <div className="px-3 py-1.5 rounded-lg bg-amber-50 border border-amber-200 text-xs font-medium text-amber-800">
                    $1K--$50K
                  </div>
                  <div className="text-xs text-amber-600">Med Risk</div>
                </div>
                <div className="flex flex-col items-center gap-2">
                  <div className="w-px h-4 bg-primary-300" />
                  <div className="px-3 py-1.5 rounded-lg bg-red-50 border border-red-200 text-xs font-medium text-red-800">
                    &gt; $50K
                  </div>
                  <div className="text-xs text-red-600">High Risk</div>
                </div>
              </div>

              {/* Converge */}
              <div className="flex items-center gap-0 mt-1">
                <div className="w-20 h-px bg-primary-300" />
                <div className="w-2 h-2 rounded-full bg-primary-400" />
                <div className="w-20 h-px bg-primary-300" />
              </div>
              <FlowArrow />

              {/* Output Node */}
              <FlowNode icon={<Upload className="w-4 h-4" />} label="Anomaly Output" color="bg-emerald-50 border-emerald-200" />
            </div>
          </div>
        </div>
      </div>
    </section>
  )
}

function FlowNode({ icon, label, color }: { icon: React.ReactNode; label: string; color: string }) {
  return (
    <div className={`flex items-center gap-2 px-4 py-2.5 rounded-lg border ${color} shadow-sm`}>
      <span className="text-primary-700">{icon}</span>
      <span className="text-sm font-medium text-primary-800">{label}</span>
    </div>
  )
}

function FlowArrow() {
  return (
    <div className="flex flex-col items-center">
      <div className="w-px h-4 bg-primary-300" />
      <div className="w-0 h-0 border-l-[5px] border-r-[5px] border-t-[6px] border-l-transparent border-r-transparent border-t-primary-300" />
    </div>
  )
}

// ─── Personas Section ──────────────────────────────────────────────────────
const personas = [
  {
    icon: BarChart3,
    title: 'Finance Analysts',
    description: 'Real-time anomaly detection and pattern analysis across all invoices and vendor transactions.',
  },
  {
    icon: Eye,
    title: 'Auditors',
    description: 'Comprehensive audit trails, compliance verification tools, and automated reporting.',
  },
  {
    icon: Users2,
    title: 'Procurement Managers',
    description: 'Contract compliance monitoring, vendor spend optimization, and pricing trend analysis.',
  },
]

function PersonasSection() {
  return (
    <section id="personas" className="py-24 bg-primary-50/50">
      <div className="container mx-auto px-6">
        <div className="text-center mb-16">
          <span className="text-sm font-medium text-primary-500">
            &bull; Roles
          </span>
          <h2 className="text-4xl font-bold text-primary-950 mt-4 mb-4">
            Solutions for Every Role.
          </h2>
          <p className="text-primary-600 max-w-2xl mx-auto">
            Whether you analyze, audit, or manage, Fenalytics integrates into your workflow.
          </p>
        </div>

        <div className="grid md:grid-cols-3 gap-0 max-w-5xl mx-auto">
          {personas.map((persona, i) => (
            <div
              key={persona.title}
              className={`p-8 ${i < personas.length - 1 ? 'md:border-r border-primary-200' : ''}`}
            >
              <div className="w-12 h-12 rounded-xl bg-primary-900 text-white flex items-center justify-center mb-5">
                <persona.icon className="w-6 h-6" />
              </div>
              <h3 className="text-lg font-semibold text-primary-900 mb-3">
                {persona.title}
              </h3>
              <p className="text-sm text-primary-500 leading-relaxed">
                {persona.description}
              </p>
            </div>
          ))}
        </div>
      </div>
    </section>
  )
}

// ─── AI Assistant Feature ──────────────────────────────────────────────────
function AIAssistantSection() {
  return (
    <section id="ai" className="py-24 bg-white">
      <div className="container mx-auto px-6">
        <div className="max-w-6xl mx-auto rounded-2xl overflow-hidden shadow-xl">
          <div className="grid md:grid-cols-2 min-h-[480px]">
            {/* Left Panel - Dark visual */}
            <div
              className="relative p-12 flex items-center justify-center"
              style={{
                background: 'linear-gradient(135deg, #09090b 0%, #18181b 50%, #1e293b 100%)',
              }}
            >
              {/* Abstract compass/radar visual */}
              <div className="relative w-48 h-48">
                <div className="absolute inset-0 rounded-full border border-primary-700 animate-pulse-slow" />
                <div className="absolute inset-6 rounded-full border border-primary-600 opacity-70" />
                <div className="absolute inset-12 rounded-full border border-primary-500 opacity-50" />
                <div className="absolute inset-0 flex items-center justify-center">
                  <div className="w-16 h-16 rounded-full bg-blue-500/20 flex items-center justify-center">
                    <Sparkles className="w-8 h-8 text-blue-400" />
                  </div>
                </div>
                {/* Orbiting dots */}
                <div className="absolute top-4 right-8 w-2 h-2 rounded-full bg-blue-400 animate-ping" />
                <div className="absolute bottom-8 left-4 w-2 h-2 rounded-full bg-purple-400 animate-ping" style={{ animationDelay: '1s' }} />
                <div className="absolute top-1/2 right-0 w-1.5 h-1.5 rounded-full bg-cyan-400 animate-ping" style={{ animationDelay: '2s' }} />
              </div>
            </div>

            {/* Right Panel - Content */}
            <div
              className="p-12 flex flex-col justify-center"
              style={{
                background: 'linear-gradient(135deg, #1e40af 0%, #2563eb 50%, #3b82f6 100%)',
              }}
            >
              <span className="text-sm font-medium text-blue-200 mb-4">
                &bull; FenalyticsGPT
              </span>
              <h2 className="text-3xl md:text-4xl font-bold text-white mb-4">
                AI Tools for Smarter<br />
                Document Analysis.
              </h2>
              <p className="text-blue-100 mb-8 leading-relaxed">
                Your 24/7 AI analyst for instant document insights.
              </p>

              <ul className="space-y-3 mb-8">
                {[
                  'Live anomaly detection and explanation',
                  'Pattern recognition across vendors',
                  'Just ask, get instant answers',
                ].map((item) => (
                  <li key={item} className="flex items-center gap-2 text-white text-sm">
                    <ArrowRight className="w-4 h-4 text-blue-200 flex-shrink-0" />
                    {item}
                  </li>
                ))}
              </ul>

              <div>
                <Link
                  to="/login"
                  className="inline-flex items-center gap-2 text-white font-medium hover:underline underline-offset-4"
                >
                  <MessageSquare className="w-4 h-4" />
                  Start Chatting <ArrowRight className="w-4 h-4" />
                </Link>
              </div>
            </div>
          </div>
        </div>
      </div>
    </section>
  )
}

// ─── Integrations & Security ───────────────────────────────────────────────
const securityFeatures = [
  { icon: Shield, label: 'SOC 2 Type II Compliant' },
  { icon: KeyRound, label: 'SSO / OIDC Authentication' },
  { icon: Building2, label: 'Multi-tenant Isolation' },
  { icon: ClipboardList, label: 'Full Audit Logging' },
  { icon: Users2, label: 'Role-based Access Control' },
]

const integrations = [
  { icon: Webhook, label: 'REST API & Webhooks' },
  { icon: FileSpreadsheet, label: 'Excel & CSV Export' },
  { icon: HardDrive, label: 'S3 / GCS Document Storage' },
  { icon: RefreshCw, label: 'ERP Connectors' },
  { icon: Database, label: 'Custom Data Sources' },
]

const deploymentOptions = [
  { icon: Cloud, label: 'Cloud Hosted', desc: 'Managed SaaS' },
  { icon: Server, label: 'On-Premise', desc: 'Your infrastructure' },
  { icon: Lock, label: 'Private Cloud', desc: 'Dedicated instance' },
]

function EnterpriseSection() {
  return (
    <section id="enterprise" className="py-24 bg-primary-50/50">
      <div className="container mx-auto px-6">
        <div className="text-center mb-16">
          <span className="text-sm font-medium text-primary-500">
            &bull; Enterprise Ready
          </span>
          <h2 className="text-4xl font-bold text-primary-950 mt-4 mb-4">
            Secure, Scalable, and Ready<br />
            for Your Organization.
          </h2>
        </div>

        <div className="max-w-5xl mx-auto">
          {/* Security + Integrations */}
          <div className="grid md:grid-cols-2 gap-8 mb-12">
            <div>
              <h3 className="text-sm font-semibold text-primary-900 uppercase tracking-wider mb-4">Security</h3>
              <div className="space-y-3">
                {securityFeatures.map((item) => (
                  <div key={item.label} className="flex items-center gap-3 p-3 rounded-lg bg-white border border-primary-100">
                    <item.icon className="w-4 h-4 text-primary-600" />
                    <span className="text-sm text-primary-800">{item.label}</span>
                  </div>
                ))}
              </div>
            </div>
            <div>
              <h3 className="text-sm font-semibold text-primary-900 uppercase tracking-wider mb-4">Integrations</h3>
              <div className="space-y-3">
                {integrations.map((item) => (
                  <div key={item.label} className="flex items-center gap-3 p-3 rounded-lg bg-white border border-primary-100">
                    <item.icon className="w-4 h-4 text-primary-600" />
                    <span className="text-sm text-primary-800">{item.label}</span>
                  </div>
                ))}
              </div>
            </div>
          </div>

          {/* Deployment Options */}
          <div>
            <h3 className="text-sm font-semibold text-primary-900 uppercase tracking-wider mb-4 text-center">
              Deployment Options
            </h3>
            <div className="grid grid-cols-3 gap-4 max-w-lg mx-auto">
              {deploymentOptions.map((item) => (
                <div key={item.label} className="text-center p-4 rounded-xl bg-white border border-primary-100 hover:border-primary-300 transition-colors">
                  <item.icon className="w-6 h-6 mx-auto mb-2 text-primary-700" />
                  <div className="text-sm font-medium text-primary-900">{item.label}</div>
                  <div className="text-xs text-primary-500 mt-1">{item.desc}</div>
                </div>
              ))}
            </div>
          </div>
        </div>
      </div>
    </section>
  )
}

// ─── CTA Section ───────────────────────────────────────────────────────────
function CTASection() {
  return (
    <section className="py-24 px-6">
      <div
        className="max-w-6xl mx-auto rounded-2xl py-20 px-8 text-center"
        style={{
          background: 'linear-gradient(135deg, #1e3a5f 0%, #09090b 50%, #2d1b3d 100%)',
        }}
      >
        <h2 className="text-4xl md:text-5xl font-bold text-white mb-6 leading-tight">
          Stop losing money to<br />
          invoice anomalies.
        </h2>
        <p className="text-primary-400 max-w-2xl mx-auto mb-8 leading-relaxed">
          Fenalytics detects pricing anomalies, duplicate invoices,
          and contract violations automatically. Start catching
          costly errors before they hit your bottom line.
        </p>
        <Link
          to="/login"
          className="inline-flex items-center gap-2 px-8 py-3 bg-white text-primary-900 rounded-full
                     font-medium hover:bg-primary-100 transition-all hover:shadow-lg"
        >
          Get Started
          <ArrowRight className="w-4 h-4" />
        </Link>
      </div>
    </section>
  )
}

// ─── Footer ────────────────────────────────────────────────────────────────
const footerLinks = {
  Navigation: ['Home', 'Discover', 'Documents', 'Contracts', 'Reports'],
  Tools: ['Dashboard', 'Anomaly View', 'Query Builder', 'Rules Engine', 'Baselines'],
  Resources: ['Documentation', 'Academy', 'API Reference', 'Pricing', 'Blog'],
}

function Footer() {
  return (
    <footer className="bg-white border-t border-primary-100">
      <div className="container mx-auto px-6 py-16">
        <div className="grid md:grid-cols-5 gap-8 mb-16">
          {/* Brand */}
          <div className="md:col-span-2">
            <Logo size="md" className="mb-4" />
            <p className="text-sm text-primary-500 mb-4 max-w-xs">
              AI-powered anomaly detection for financial documents. Built for finance teams.
            </p>
            <div className="flex gap-4 text-xs text-primary-400">
              <a href="#" className="hover:text-primary-700 transition-colors">Privacy Policy</a>
              <a href="#" className="hover:text-primary-700 transition-colors">Terms of Use</a>
            </div>
          </div>

          {/* Link columns */}
          {Object.entries(footerLinks).map(([title, links]) => (
            <div key={title}>
              <h4 className="text-sm font-semibold text-primary-900 mb-4">{title}</h4>
              <ul className="space-y-2.5">
                {links.map((link) => (
                  <li key={link}>
                    <a href="#" className="text-sm text-primary-500 hover:text-primary-800 transition-colors">
                      {link}
                    </a>
                  </li>
                ))}
              </ul>
            </div>
          ))}
        </div>

        {/* Brand Mark */}
        <div className="text-center pt-8 border-t border-primary-100">
          <span className="font-logo text-[80px] md:text-[120px] leading-none text-primary-100 select-none tracking-widest">
            Fen
          </span>
        </div>
      </div>
    </footer>
  )
}

// ─── Landing Page ──────────────────────────────────────────────────────────
export function LandingPage() {
  return (
    <div className="min-h-screen">
      <HeroSection />
      <FeaturesSection />
      <QueryWorkbenchSection />
      <RulesEngineSection />
      <PersonasSection />
      <AIAssistantSection />
      <EnterpriseSection />
      <CTASection />
      <Footer />
    </div>
  )
}

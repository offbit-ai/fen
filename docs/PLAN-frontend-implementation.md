# Frontend Implementation Plan: Login to Workbench

## Overview

Build the Fenalytics web application from login screen to full workbench dashboard using React 18, TypeScript, and Tailwind CSS.

---

## Phase 1: Project Setup & Foundation (Current Sprint)

### 1.1 Project Scaffolding
- [x] Create Vite + React + TypeScript project at `web/`
- [x] Configure Tailwind CSS with design system tokens
- [x] Set up path aliases and project structure
- [x] Install core dependencies

### 1.2 Design System
- [x] Implement monochrome color palette from PRD
- [x] Configure typography (Inter, JetBrains Mono, Geom)
- [x] Set up Shadcn/ui components
- [x] Create base layout components

### 1.3 Core Components
- [ ] Logo component (Fennec Fox + Fenalytics wordmark)
- [ ] Button variants (primary, secondary, ghost, outline)
- [ ] Input components (text, password with visibility toggle)
- [ ] Card component

---

## Phase 2: Authentication Flow

### 2.1 Login Page
- [ ] Split-screen layout (40/60)
- [ ] Left panel: Login form
  - Welcome heading
  - Email input
  - Password input with visibility toggle
  - "Forgot Password?" link
  - "Sign in" button
  - "Continue with Google" SSO button
  - Sign up link
- [ ] Right panel: Product showcase
  - Dashboard screenshot/preview
  - Animated carousel (future)

### 2.2 Auth Integration
- [ ] Keycloak OIDC client setup
- [ ] JWT token management
- [ ] Protected route wrapper
- [ ] Auth context provider

---

## Phase 3: App Shell & Navigation

### 3.1 Layout Structure
```
┌─────────────────────────────────────────────────────────┐
│ Header (64px)                                    [User] │
├──────────┬──────────────────────────────────────────────┤
│ Sidebar  │ Main Content                                 │
│ (240px)  │                                              │
│          │                                              │
│ DOCUMENTS│                                              │
│  Invoices│                                              │
│  Contracts                                              │
│  ...     │                                              │
│          │                                              │
│ ANALYSIS │                                              │
│  Query   │                                              │
│  Reports │                                              │
│  ...     │                                              │
└──────────┴──────────────────────────────────────────────┘
```

### 3.2 Components
- [ ] AppShell layout wrapper
- [ ] Sidebar with collapsible sections
- [ ] Header with user menu
- [ ] Breadcrumb navigation

---

## Phase 4: Dashboard (Home)

### 4.1 KPI Cards
- [ ] Documents processed
- [ ] Total value analyzed
- [ ] Anomaly rate
- [ ] Active rules

### 4.2 Widgets
- [ ] Recent anomalies list
- [ ] Anomaly trend chart
- [ ] Quick actions panel

---

## Phase 5: Document Repository

### 5.1 Invoice List
- [ ] TanStack Table with virtual scrolling
- [ ] Column sorting and filtering
- [ ] Bulk selection
- [ ] Inline status indicators

### 5.2 Invoice Detail
- [ ] Document viewer
- [ ] Extracted fields panel
- [ ] Anomaly highlights
- [ ] Action buttons

---

## Phase 6: Query Workbench

### 6.1 Editor
- [ ] Monaco editor integration
- [ ] SQL syntax highlighting
- [ ] Auto-completion

### 6.2 Results
- [ ] Results table
- [ ] Export options
- [ ] Query history

---

## Tech Stack

| Component | Technology |
|-----------|------------|
| Framework | React 18 + TypeScript |
| Build Tool | Vite |
| Routing | React Router v6 |
| State | Zustand |
| UI Components | Shadcn/ui + Radix |
| Styling | Tailwind CSS |
| Forms | React Hook Form + Zod |
| HTTP Client | Ky or Fetch |
| Icons | Lucide React |

---

## Directory Structure

```
web/
├── src/
│   ├── components/
│   │   ├── ui/              # Shadcn components
│   │   ├── layout/          # AppShell, Sidebar, Header
│   │   ├── forms/           # Form components
│   │   └── shared/          # Shared components
│   ├── pages/
│   │   ├── auth/
│   │   │   ├── login.tsx
│   │   │   └── callback.tsx
│   │   ├── dashboard/
│   │   │   └── index.tsx
│   │   ├── documents/
│   │   │   ├── invoices/
│   │   │   └── contracts/
│   │   ├── analysis/
│   │   │   ├── workbench/
│   │   │   └── anomalies/
│   │   └── settings/
│   ├── hooks/
│   │   ├── use-auth.ts
│   │   └── use-api.ts
│   ├── store/
│   │   ├── auth.ts
│   │   └── ui.ts
│   ├── api/
│   │   ├── client.ts
│   │   └── endpoints/
│   ├── types/
│   │   ├── api.ts
│   │   └── domain.ts
│   ├── lib/
│   │   └── utils.ts
│   ├── styles/
│   │   └── globals.css
│   ├── App.tsx
│   └── main.tsx
├── public/
│   └── logo.svg
├── index.html
├── package.json
├── tsconfig.json
├── vite.config.ts
├── tailwind.config.ts
└── postcss.config.js
```

---

## API Integration

Base URL: `http://localhost:3000/api/v1`

### Endpoints Used

| Feature | Method | Endpoint |
|---------|--------|----------|
| Login | POST | `/auth/login` |
| OIDC Callback | GET | `/auth/callback` |
| Current User | GET | `/auth/me` |
| List Invoices | GET | `/documents` |
| Get Invoice | GET | `/documents/:id` |
| Search | GET | `/search/text` |
| Execute Query | POST | `/search/query` |
| List Anomalies | GET | `/anomalies` |
| Get Rules | GET | `/rules/status` |

---

## Implementation Order

1. **Today**: Project setup, Tailwind config, base components
2. **Next**: Login page UI (no auth integration yet)
3. **Then**: App shell with sidebar navigation
4. **Then**: Dashboard with mock data
5. **Then**: Auth integration with Keycloak
6. **Then**: API client and real data
7. **Then**: Invoice repository
8. **Then**: Query workbench

---

## Commit Strategy

- `feat(web): initial project setup with Vite + React + TypeScript`
- `feat(web): add Tailwind CSS design system`
- `feat(web): implement login page UI`
- `feat(web): add app shell layout with sidebar`
- `feat(web): implement dashboard with KPI cards`
- `feat(web): add auth integration with Keycloak`

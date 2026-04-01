# LLMSmartGate -- Frontend Low-Level Design (LLD)

> **Doc ID:** `LLD-FE-001`  
> **Version:** 1.0.0  
> **Date:** 2026-04-01  
> **Status:** Draft  
> **Classification:** Internal -- Engineering

| Field | Value |
|-------|-------|
| **Project** | LLMSmartGate Admin Console |
| **Stack** | SvelteKit 2.55 + Svelte 5.55 + shadcn-svelte 1.2 + TailwindCSS 4.x |
| **Target** | Self-hosted admin console for LLM API Gateway |

---

## Document Persona & Guidelines

| | |
|---|---|
| **Author Role** | Senior Svelte/Frontend Developer |
| **Perspective** | UI architecture, component design, state management, UX patterns |
| **Primary Audience** | Frontend developers implementing the Admin Console |
| **Secondary Audience** | Rust Developer (API contract), QA (E2E test design), Designer |

### How to Read This Document

| Symbol | Meaning |
|--------|---------|
| `src/routes/xxx/` | SvelteKit file-based route -- maps to URL path |
| `+page.server.ts` | Server-side data loading -- runs on SvelteKit server only |
| `+page.svelte` | Client-rendered page component |
| `$state()` / `$derived()` | Svelte 5 runes -- reactive primitives (not legacy stores) |
| `[SSR]` / `[CSR]` | Server-Side Rendered vs Client-Side Rendered |
| `[a11y]` | Accessibility consideration -- WCAG 2.2 AA compliance |
| `[sec]` | Security-sensitive UI pattern |

### Reading Order Suggestion

1. **Technology Stack (Section 1)** -- Understand the tools
2. **Project Structure (Section 2)** -- Map the codebase
3. **Authentication (Section 4)** -- How JWT/BFF works
4. **Page Designs (Section 5)** -- The core of the UI
5. **API Integration (Section 8)** -- How frontend talks to backend
6. Reference other sections as needed during implementation

### Document Relationships

```
PRD-001                -- UI requirements & wireframes
  ├── ARC-001          -- Frontend tech standards (Svelte 5, TailwindCSS 4)
  ├── HLD-001          -- Admin Console container in C4 diagram
  ├── LLD-BE-001       -- Admin API endpoints this UI consumes
  ├── LLD-FE-001 (this)-- Frontend implementation design
  └── TST-001          -- E2E & component test cases for UI
```

---

## Table of Contents

1. [Technology Stack & Versions](#1-technology-stack--versions)
2. [Project Structure](#2-project-structure)
3. [Route Architecture](#3-route-architecture)
4. [Authentication & Authorization](#4-authentication--authorization)
5. [Page Designs](#5-page-designs)
6. [Component Architecture](#6-component-architecture)
7. [State Management](#7-state-management)
8. [API Integration](#8-api-integration)
9. [Styling & Theming](#9-styling--theming)
10. [Accessibility](#10-accessibility)
11. [Performance](#11-performance)
12. [Testing](#12-testing)
13. [Build & Deploy](#13-build--deploy)
14. [Security](#14-security)
15. [Observability & Tracing](#15-observability--tracing)

---

## 1. Technology Stack & Versions

### Core Dependencies

| Package | Version | Purpose |
|---------|---------|---------|
| `svelte` | `^5.55` | UI framework with runes reactivity |
| `@sveltejs/kit` | `^2.55` | Full-stack framework (SSR, routing, form actions) |
| `@sveltejs/adapter-node` | `^5.x` | Node.js production adapter for Docker deployment |
| `typescript` | `^5.7` | Type safety |
| `vite` | `^6.x` | Build tool |

### UI & Styling

| Package | Version | Purpose |
|---------|---------|---------|
| `shadcn-svelte` | `^1.2.5` | Component primitives (copy-paste components) |
| `bits-ui` | `^1.x` | Headless accessible primitives (powers shadcn-svelte) |
| `tailwindcss` | `^4.x` | Utility-first CSS (CSS-first config, no JS config file) |
| `lucide-svelte` | `^0.475` | Icon library |
| `mode-watcher` | `^0.5` | Dark/light mode management with SSR support |

### Data & Forms

| Package | Version | Purpose |
|---------|---------|---------|
| `@tanstack/svelte-table` | `^9.x` | Headless data table (sorting, filtering, pagination) |
| `sveltekit-superforms` | `^2.x` | Server+client form validation with progressive enhancement |
| `formsnap` | `^2.x` | Accessible form component bindings for Superforms |
| `zod` | `^3.24` | Schema validation (shared client/server) |

### Charts & Visualization

| Package | Version | Purpose |
|---------|---------|---------|
| `layerchart` | `^2.x` (next) | Svelte 5 runes-compatible chart components built on LayerCake |
| `d3-scale` | `^4.x` | Scale utilities for charts |
| `d3-shape` | `^3.x` | Shape generators for SVG paths |
| `d3-time-format` | `^4.x` | Date/time formatting |

### API & Networking

| Package | Version | Purpose |
|---------|---------|---------|
| `openapi-typescript` | `^7.13` | Generate TypeScript types from Admin API OpenAPI spec |
| `openapi-fetch` | `^0.17` | Type-safe fetch client (~6 KB, zero runtime overhead) |

### Testing

| Package | Version | Purpose |
|---------|---------|---------|
| `vitest` | `^3.x` | Unit & component testing |
| `@vitest/browser` | `^3.x` | Browser-mode component tests with real DOM |
| `vitest-browser-svelte` | `^0.x` | Svelte component rendering in Vitest browser mode |
| `@playwright/test` | `^1.50` | E2E testing |
| `@testing-library/svelte` | `^5.x` | Component test utilities |
| `axe-core` | `^4.x` | Automated accessibility testing |

### Research Evidence

- Svelte 5.55 and SvelteKit 2.55 are the latest stable releases as of March 2026 ([npm: svelte](https://www.npmjs.com/package/svelte), [npm: @sveltejs/kit](https://www.npmjs.com/package/@sveltejs/kit)).
- shadcn-svelte 1.2.5 released March 25, 2026 with Svelte 5 + Tailwind v4 support ([shadcn-svelte changelog](https://www.shadcn-svelte.com/docs/changelog)).
- LayerChart 2.0 migrates to Svelte 5 runes ($state/$derived) and snippets, available as `layerchart@next` ([LayerChart changelog](https://www.layerchart.com/changelog)).
- openapi-fetch 0.17 provides type-safe fetch with ~6 KB bundle and native SvelteKit load function support ([openapi-ts docs](https://openapi-ts.dev/openapi-fetch/)).
- TailwindCSS v4 replaces `tailwind.config.js` with CSS-first configuration using `@theme` directive and ships a Rust-based incremental compiler with 60-80% faster cold builds ([Tailwind blog](https://tailwindcss.com/blog/tailwindcss-v4)).
- Vitest browser mode with Playwright is the recommended approach for component testing in 2026, replacing jsdom-based tests ([Svelte docs: Testing](https://svelte.dev/docs/svelte/testing)).

---

## 2. Project Structure

```
admin-console/
├── src/
│   ├── app.html                          # HTML shell
│   ├── app.css                           # TailwindCSS v4 imports + @theme tokens
│   ├── hooks.server.ts                   # Server hooks (auth guard, correlation ID)
│   ├── hooks.client.ts                   # Client hooks (global error handler)
│   ├── error.html                        # Static fallback error page
│   │
│   ├── lib/
│   │   ├── api/
│   │   │   ├── client.ts                 # openapi-fetch client singleton
│   │   │   ├── client.server.ts          # Server-side API client (with admin JWT)
│   │   │   ├── types.d.ts               # Generated from OpenAPI (openapi-typescript)
│   │   │   └── interceptors.ts           # Request/response interceptors
│   │   │
│   │   ├── auth/
│   │   │   ├── guard.ts                  # Route protection logic
│   │   │   ├── session.ts                # Session/JWT helpers
│   │   │   └── roles.ts                  # Role constants & permission checks
│   │   │
│   │   ├── components/
│   │   │   ├── ui/                       # shadcn-svelte generated components
│   │   │   │   ├── button/
│   │   │   │   ├── card/
│   │   │   │   ├── dialog/
│   │   │   │   ├── dropdown-menu/
│   │   │   │   ├── input/
│   │   │   │   ├── select/
│   │   │   │   ├── table/
│   │   │   │   ├── badge/
│   │   │   │   ├── toast/
│   │   │   │   ├── skeleton/
│   │   │   │   ├── sheet/
│   │   │   │   ├── tabs/
│   │   │   │   ├── command/
│   │   │   │   ├── popover/
│   │   │   │   ├── separator/
│   │   │   │   ├── switch/
│   │   │   │   ├── tooltip/
│   │   │   │   ├── alert/
│   │   │   │   ├── alert-dialog/
│   │   │   │   ├── breadcrumb/
│   │   │   │   ├── pagination/
│   │   │   │   ├── scroll-area/
│   │   │   │   └── chart/                # LayerChart wrappers
│   │   │   │
│   │   │   ├── layout/
│   │   │   │   ├── AppShell.svelte       # Main app chrome (sidebar + header + main)
│   │   │   │   ├── Sidebar.svelte        # Collapsible sidebar navigation
│   │   │   │   ├── SidebarNav.svelte     # Navigation items
│   │   │   │   ├── Header.svelte         # Top bar (breadcrumbs, user menu, theme)
│   │   │   │   ├── Breadcrumbs.svelte    # Dynamic breadcrumb trail
│   │   │   │   ├── UserMenu.svelte       # Dropdown: profile, logout
│   │   │   │   └── ThemeToggle.svelte    # Dark/light/system mode toggle
│   │   │   │
│   │   │   ├── data-table/
│   │   │   │   ├── DataTable.svelte      # Generic TanStack Table wrapper
│   │   │   │   ├── DataTablePagination.svelte
│   │   │   │   ├── DataTableToolbar.svelte
│   │   │   │   ├── DataTableColumnHeader.svelte
│   │   │   │   ├── DataTableFacetedFilter.svelte
│   │   │   │   └── DataTableViewOptions.svelte
│   │   │   │
│   │   │   ├── charts/
│   │   │   │   ├── TimeSeriesChart.svelte    # Usage over time
│   │   │   │   ├── BarChart.svelte           # Comparative bars
│   │   │   │   ├── DonutChart.svelte         # Distribution/breakdown
│   │   │   │   ├── SparkLine.svelte          # Inline mini charts
│   │   │   │   └── MetricCard.svelte         # KPI card with trend
│   │   │   │
│   │   │   ├── forms/
│   │   │   │   ├── FormField.svelte          # Formsnap field wrapper
│   │   │   │   ├── FormSelect.svelte         # Select with validation
│   │   │   │   ├── FormMultiSelect.svelte    # Tag-style multi select
│   │   │   │   ├── FormSwitch.svelte         # Toggle with label
│   │   │   │   ├── FormTextarea.svelte       # Textarea with counter
│   │   │   │   └── FormNumberInput.svelte    # Numeric input with bounds
│   │   │   │
│   │   │   ├── shared/
│   │   │   │   ├── StatusBadge.svelte        # active/suspended/revoked badges
│   │   │   │   ├── ConfirmDialog.svelte      # Destructive action confirmation
│   │   │   │   ├── EmptyState.svelte         # No-data placeholder
│   │   │   │   ├── PageHeader.svelte         # Title + description + actions
│   │   │   │   ├── SearchInput.svelte        # Debounced search with icon
│   │   │   │   ├── CopyButton.svelte         # Copy to clipboard
│   │   │   │   ├── RelativeTime.svelte       # "2 hours ago" display
│   │   │   │   ├── JsonViewer.svelte         # Collapsible JSON display
│   │   │   │   └── KeyboardShortcut.svelte   # Kbd display component
│   │   │   │
│   │   │   └── feedback/
│   │   │       ├── LoadingSkeleton.svelte    # Page-level skeleton
│   │   │       ├── InlineSpinner.svelte      # Button/inline loading
│   │   │       ├── ErrorPanel.svelte         # Error display with retry
│   │   │       └── Toaster.svelte            # Toast notification host
│   │   │
│   │   ├── stores/
│   │   │   ├── auth.svelte.ts            # Auth state (Svelte 5 runes class)
│   │   │   ├── sidebar.svelte.ts         # Sidebar collapse state
│   │   │   └── preferences.svelte.ts     # User preferences (locale, density)
│   │   │
│   │   ├── schemas/
│   │   │   ├── tenant.ts                 # Zod schemas for tenant forms
│   │   │   ├── service-account.ts
│   │   │   ├── key.ts
│   │   │   ├── policy.ts
│   │   │   ├── route.ts
│   │   │   └── shared.ts                 # Common validators (uuid, slug, etc.)
│   │   │
│   │   ├── utils/
│   │   │   ├── format.ts                 # Number, date, currency formatters
│   │   │   ├── debounce.ts               # Debounce utility
│   │   │   ├── url.ts                    # URL/query param helpers
│   │   │   ├── clipboard.ts              # Copy to clipboard
│   │   │   ├── export.ts                 # CSV/JSON export
│   │   │   └── correlation.ts            # Correlation ID management
│   │   │
│   │   └── constants/
│   │       ├── navigation.ts             # Sidebar nav items
│   │       ├── providers.ts              # Known provider list
│   │       ├── models.ts                 # Common model aliases
│   │       └── status.ts                 # Status enums + label maps
│   │
│   ├── routes/
│   │   ├── +layout.svelte               # Root layout (ModeWatcher, Toaster)
│   │   ├── +layout.server.ts            # Root server layout (auth check)
│   │   ├── +error.svelte                # Global error page
│   │   │
│   │   ├── login/
│   │   │   ├── +page.svelte             # Login form
│   │   │   └── +page.server.ts          # Login action (POST)
│   │   │
│   │   ├── (app)/                        # Route group: authenticated shell
│   │   │   ├── +layout.svelte           # AppShell (sidebar + header)
│   │   │   ├── +layout.server.ts        # Auth guard load function
│   │   │   │
│   │   │   ├── dashboard/
│   │   │   │   ├── +page.svelte
│   │   │   │   └── +page.server.ts
│   │   │   │
│   │   │   ├── tenants/
│   │   │   │   ├── +page.svelte              # Tenant list
│   │   │   │   ├── +page.server.ts           # List + create action
│   │   │   │   ├── new/
│   │   │   │   │   ├── +page.svelte          # Create tenant form
│   │   │   │   │   └── +page.server.ts
│   │   │   │   └── [tenantId]/
│   │   │   │       ├── +page.svelte          # Tenant detail
│   │   │   │       ├── +page.server.ts
│   │   │   │       └── edit/
│   │   │   │           ├── +page.svelte      # Edit tenant
│   │   │   │           └── +page.server.ts
│   │   │   │
│   │   │   ├── service-accounts/
│   │   │   │   ├── +page.svelte              # SA list (filterable by tenant)
│   │   │   │   ├── +page.server.ts
│   │   │   │   ├── new/
│   │   │   │   │   ├── +page.svelte
│   │   │   │   │   └── +page.server.ts
│   │   │   │   └── [accountId]/
│   │   │   │       ├── +page.svelte          # SA detail with keys tab
│   │   │   │       ├── +page.server.ts
│   │   │   │       ├── edit/
│   │   │   │       │   ├── +page.svelte
│   │   │   │       │   └── +page.server.ts
│   │   │   │       └── keys/
│   │   │   │           ├── +page.svelte      # Keys list for this SA
│   │   │   │           ├── +page.server.ts
│   │   │   │           └── register/
│   │   │   │               ├── +page.svelte  # Register new key form
│   │   │   │               └── +page.server.ts
│   │   │   │
│   │   │   ├── policies/
│   │   │   │   ├── +page.svelte              # Policy list
│   │   │   │   ├── +page.server.ts
│   │   │   │   ├── new/
│   │   │   │   │   ├── +page.svelte          # Policy builder
│   │   │   │   │   └── +page.server.ts
│   │   │   │   └── [policyId]/
│   │   │   │       ├── +page.svelte          # Policy detail
│   │   │   │       ├── +page.server.ts
│   │   │   │       └── edit/
│   │   │   │           ├── +page.svelte
│   │   │   │           └── +page.server.ts
│   │   │   │
│   │   │   ├── routes/
│   │   │   │   ├── +page.svelte              # Route config table
│   │   │   │   ├── +page.server.ts
│   │   │   │   ├── new/
│   │   │   │   │   ├── +page.svelte
│   │   │   │   │   └── +page.server.ts
│   │   │   │   └── [routeId]/
│   │   │   │       ├── edit/
│   │   │   │       │   ├── +page.svelte
│   │   │   │       │   └── +page.server.ts
│   │   │   │
│   │   │   ├── usage/
│   │   │   │   ├── +page.svelte              # Usage analytics
│   │   │   │   └── +page.server.ts
│   │   │   │
│   │   │   ├── audit/
│   │   │   │   ├── +page.svelte              # Audit log stream
│   │   │   │   └── +page.server.ts
│   │   │   │
│   │   │   └── settings/
│   │   │       ├── +page.svelte              # System settings
│   │   │       ├── +page.server.ts
│   │   │       └── profile/
│   │   │           ├── +page.svelte          # User profile
│   │   │           └── +page.server.ts
│   │   │
│   │   └── api/                              # SvelteKit API routes (BFF)
│   │       ├── auth/
│   │       │   ├── login/+server.ts          # POST: issue session cookie
│   │       │   ├── logout/+server.ts         # POST: clear session
│   │       │   └── refresh/+server.ts        # POST: refresh JWT
│   │       └── sse/
│   │           └── usage/+server.ts          # SSE: real-time usage stream
│   │
│   └── params/
│       └── uuid.ts                           # Param matcher for UUID routes
│
├── static/
│   ├── favicon.svg
│   └── logo.svg
│
├── tests/
│   ├── unit/                                 # Vitest unit tests
│   │   ├── utils/
│   │   └── schemas/
│   ├── component/                            # Vitest browser-mode component tests
│   │   ├── data-table/
│   │   └── forms/
│   └── e2e/                                  # Playwright E2E tests
│       ├── auth.spec.ts
│       ├── tenants.spec.ts
│       ├── policies.spec.ts
│       └── fixtures/
│
├── openapi/
│   └── admin-api.yaml                        # Admin API OpenAPI spec (source of truth)
│
├── svelte.config.js
├── vite.config.ts
├── tailwind.config.ts                        # Minimal -- most config in app.css @theme
├── tsconfig.json
├── playwright.config.ts
├── vitest.config.ts
├── Dockerfile
├── .dockerignore
├── .env.example
└── package.json
```

### Design Rationale

- **Route groups** `(app)/` separate authenticated pages from login, so the AppShell layout only wraps authenticated routes ([SvelteKit routing docs](https://svelte.dev/docs/kit/routing)).
- **`.svelte.ts` files** for stores leverage Svelte 5 runes outside components -- the `.svelte.ts` extension enables rune compilation in plain TS files ([Svelte docs: best practices](https://svelte.dev/docs/svelte/best-practices)).
- **BFF pattern** (`src/routes/api/`): the SvelteKit server acts as a Backend-For-Frontend, holding the JWT and proxying to the Rust admin API. The browser never touches the raw JWT.
- **Schemas directory**: Zod schemas are defined at module top-level (not inside load functions) for memoization by Superforms ([Superforms docs](https://superforms.rocks/get-started/zod)).

---

## 3. Route Architecture

### URL Map

| URL Path | Page | Data Source | SSR |
|----------|------|-------------|-----|
| `/login` | Login form | None (form action) | Yes |
| `/dashboard` | Dashboard overview | `GET /admin/usage` + aggregates | Yes |
| `/tenants` | Tenant list | `GET /admin/tenants` | Yes |
| `/tenants/new` | Create tenant | Form action | Yes |
| `/tenants/[tenantId]` | Tenant detail | `GET /admin/tenants/:id` | Yes |
| `/tenants/[tenantId]/edit` | Edit tenant | `GET + PUT /admin/tenants/:id` | Yes |
| `/service-accounts` | SA list | `GET /admin/service-accounts` | Yes |
| `/service-accounts/new` | Create SA | Form action | Yes |
| `/service-accounts/[accountId]` | SA detail | `GET /admin/service-accounts/:id` | Yes |
| `/service-accounts/[accountId]/edit` | Edit SA | `GET + PUT` | Yes |
| `/service-accounts/[accountId]/keys` | Keys list | `GET /admin/service-accounts/:id/keys` | Yes |
| `/service-accounts/[accountId]/keys/register` | Register key | Form action | Yes |
| `/policies` | Policy list | `GET /admin/policies` | Yes |
| `/policies/new` | Create policy | Form action | Yes |
| `/policies/[policyId]` | Policy detail | `GET /admin/policies/:id` | Yes |
| `/policies/[policyId]/edit` | Edit policy | `GET + PUT` | Yes |
| `/routes` | Route config list | `GET /admin/routes` | Yes |
| `/routes/new` | Create route | Form action | Yes |
| `/routes/[routeId]/edit` | Edit route | `GET + PUT` | Yes |
| `/usage` | Usage analytics | `GET /admin/usage` + params | CSR* |
| `/audit` | Audit log stream | `GET /admin/audit` + params | Yes |
| `/settings` | System settings | `GET /admin/settings` | Yes |
| `/settings/profile` | User profile | Session data | Yes |

*Usage page uses CSR for interactive chart rendering with heavy client-side filtering. Initial data is SSR, subsequent interactions are client-side.

### UUID Param Matcher

```typescript
// src/params/uuid.ts
export function match(param: string): boolean {
  return /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(param);
}
```

---

## 4. Authentication & Authorization

### Architecture Overview

The SvelteKit server (Node.js) acts as a secure BFF. The admin JWT is **never exposed to the browser**. Instead:

1. Browser submits credentials to SvelteKit form action
2. SvelteKit server calls Rust admin API `POST /admin/auth/login`
3. Rust returns a JWT + refresh token
4. SvelteKit stores the JWT in a server-side cookie (`HttpOnly`, `Secure`, `SameSite=Lax`)
5. Subsequent requests: SvelteKit `hooks.server.ts` reads cookie, attaches JWT to upstream API calls

```
Browser ──cookie──> SvelteKit Server ──JWT Bearer──> Rust Admin API
```

### Server Hook (Auth Middleware)

```typescript
// src/hooks.server.ts
import type { Handle } from '@sveltejs/kit';
import { redirect } from '@sveltejs/kit';
import { sequence } from '@sveltejs/kit/hooks';
import { createAdminApiClient } from '$lib/api/client.server';
import { nanoid } from 'nanoid';

const PUBLIC_PATHS = ['/login', '/api/auth/login'];

const correlationId: Handle = async ({ event, resolve }) => {
  const id = event.request.headers.get('x-correlation-id') ?? nanoid();
  event.locals.correlationId = id;
  const response = await resolve(event);
  response.headers.set('x-correlation-id', id);
  return response;
};

const authGuard: Handle = async ({ event, resolve }) => {
  const isPublic = PUBLIC_PATHS.some((p) => event.url.pathname.startsWith(p));

  if (isPublic) {
    return resolve(event);
  }

  const sessionToken = event.cookies.get('session');
  if (!sessionToken) {
    throw redirect(303, '/login');
  }

  try {
    // Validate session and extract admin user info
    const api = createAdminApiClient(sessionToken);
    const { data: user } = await api.GET('/admin/auth/me');

    event.locals.user = user;
    event.locals.api = api;
  } catch {
    // Token expired or invalid -- attempt refresh
    const refreshToken = event.cookies.get('refresh');
    if (refreshToken) {
      try {
        const { accessToken, newRefreshToken } = await refreshSession(refreshToken);
        event.cookies.set('session', accessToken, {
          path: '/',
          httpOnly: true,
          secure: true,
          sameSite: 'lax',
          maxAge: 60 * 15 // 15 minutes
        });
        event.cookies.set('refresh', newRefreshToken, {
          path: '/',
          httpOnly: true,
          secure: true,
          sameSite: 'lax',
          maxAge: 60 * 60 * 24 * 7 // 7 days
        });
        const api = createAdminApiClient(accessToken);
        const { data: user } = await api.GET('/admin/auth/me');
        event.locals.user = user;
        event.locals.api = api;
      } catch {
        event.cookies.delete('session', { path: '/' });
        event.cookies.delete('refresh', { path: '/' });
        throw redirect(303, '/login');
      }
    } else {
      throw redirect(303, '/login');
    }
  }

  return resolve(event);
};

export const handle = sequence(correlationId, authGuard);
```

### Login Form Action

```typescript
// src/routes/login/+page.server.ts
import type { Actions } from './$types';
import { fail, redirect } from '@sveltejs/kit';
import { superValidate, message } from 'sveltekit-superforms';
import { zod } from 'sveltekit-superforms/adapters';
import { loginSchema } from '$lib/schemas/shared';

export const actions: Actions = {
  default: async ({ request, cookies, fetch }) => {
    const form = await superValidate(request, zod(loginSchema));
    if (!form.valid) return fail(400, { form });

    const response = await fetch('/api/auth/login', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(form.data)
    });

    if (!response.ok) {
      return message(form, 'Invalid credentials', { status: 401 });
    }

    const { access_token, refresh_token } = await response.json();

    cookies.set('session', access_token, {
      path: '/',
      httpOnly: true,
      secure: true,
      sameSite: 'lax',
      maxAge: 60 * 15
    });

    cookies.set('refresh', refresh_token, {
      path: '/',
      httpOnly: true,
      secure: true,
      sameSite: 'lax',
      maxAge: 60 * 60 * 24 * 7
    });

    throw redirect(303, '/dashboard');
  }
};
```

### Type Declarations for `event.locals`

```typescript
// src/app.d.ts
import type { ApiClient } from '$lib/api/client.server';

declare global {
  namespace App {
    interface Locals {
      user: {
        id: string;
        email: string;
        name: string;
        role: 'admin' | 'viewer';
      } | null;
      api: ApiClient;
      correlationId: string;
    }
    interface Error {
      message: string;
      correlationId?: string;
    }
  }
}

export {};
```

### Role-Based UI Rendering

```svelte
<!-- Example: conditionally show destructive actions -->
<script lang="ts">
  import type { PageData } from './$types';

  let { data }: { data: PageData } = $props();
</script>

{#if data.user.role === 'admin'}
  <Button variant="destructive" onclick={handleDelete}>Delete Tenant</Button>
{/if}
```

**Evidence**: SvelteKit's `handle` hook runs on every server request and is the recommended place for auth middleware. The `event.locals` pattern propagates auth context to all load functions and form actions ([SvelteKit docs: Auth](https://svelte.dev/docs/kit/auth), [SvelteKit JWT middleware](https://dev.to/jais_mukesh/sveltekit-jwt-authentication-with-middleware-a-complete-implementation-51gm)).

---

## 5. Page Designs

### 5.1 Dashboard

**URL**: `/dashboard`

**Layout**: 4-column responsive grid with KPI cards at top, charts below.

**Widgets**:
1. **KPI Row** (4 `MetricCard` components):
   - Total Requests (24h) with trend sparkline
   - Total Cost (current month) with budget percentage
   - Active Service Accounts count
   - Error Rate (%) with alert threshold indicator

2. **Request Volume Chart** (`TimeSeriesChart`):
   - Stacked area chart showing requests over last 7 days
   - Breakdown by model alias
   - Hover tooltip with exact values

3. **Cost Breakdown** (`DonutChart`):
   - Cost by provider (OpenAI, Anthropic, etc.)
   - Legend with percentages

4. **Top Consumers Table**:
   - Top 10 service accounts by token usage
   - Columns: name, tenant, total tokens, cost, last active
   - Click to navigate to SA detail

5. **Recent Audit Events**:
   - Last 10 audit events in a compact list
   - Action badge, actor, target, relative timestamp
   - "View all" link to `/audit`

6. **Provider Health Status**:
   - Status cards for each configured provider
   - Green/yellow/red indicators
   - Average latency (p50) display

**Real-Time Updates**: The dashboard uses SSE to receive live metric updates. A `$effect` subscribes to the SSE endpoint on mount and updates `$state` variables.

```typescript
// src/routes/(app)/dashboard/+page.server.ts
import type { PageServerLoad } from './$types';

export const load: PageServerLoad = async ({ locals }) => {
  const [usageSummary, recentAudit, providerHealth] = await Promise.all([
    locals.api.GET('/admin/usage/summary', {
      params: { query: { period: '24h' } }
    }),
    locals.api.GET('/admin/audit', {
      params: { query: { limit: 10 } }
    }),
    locals.api.GET('/admin/providers/health')
  ]);

  return {
    usageSummary: usageSummary.data,
    recentAudit: recentAudit.data,
    providerHealth: providerHealth.data,
    user: locals.user
  };
};
```

```svelte
<!-- src/routes/(app)/dashboard/+page.svelte -->
<script lang="ts">
  import type { PageData } from './$types';
  import MetricCard from '$lib/components/charts/MetricCard.svelte';
  import TimeSeriesChart from '$lib/components/charts/TimeSeriesChart.svelte';
  import DonutChart from '$lib/components/charts/DonutChart.svelte';

  let { data }: { data: PageData } = $props();

  // SSE for real-time metrics
  let liveMetrics = $state(data.usageSummary);

  $effect(() => {
    const eventSource = new EventSource('/api/sse/usage');
    eventSource.onmessage = (event) => {
      liveMetrics = JSON.parse(event.data);
    };
    return () => eventSource.close();
  });
</script>

<div class="grid grid-cols-1 gap-4 md:grid-cols-2 xl:grid-cols-4">
  <MetricCard
    title="Total Requests (24h)"
    value={liveMetrics.totalRequests}
    trend={liveMetrics.requestsTrend}
  />
  <MetricCard
    title="Cost (This Month)"
    value={liveMetrics.monthlyCost}
    format="currency"
    alert={liveMetrics.budgetPercentage > 80}
  />
  <MetricCard
    title="Active Service Accounts"
    value={liveMetrics.activeAccounts}
  />
  <MetricCard
    title="Error Rate"
    value={liveMetrics.errorRate}
    format="percentage"
    alert={liveMetrics.errorRate > 5}
  />
</div>

<div class="mt-6 grid grid-cols-1 gap-6 lg:grid-cols-3">
  <div class="lg:col-span-2">
    <TimeSeriesChart
      data={liveMetrics.requestTimeSeries}
      title="Request Volume (7 days)"
    />
  </div>
  <DonutChart
    data={liveMetrics.costByProvider}
    title="Cost by Provider"
  />
</div>
```

### 5.2 Tenants

**URL**: `/tenants`

**List Page**:
- `DataTable` with columns: Name, Slug, Status, Service Accounts count, Created At
- Toolbar: search by name, filter by status (`active`/`suspended`/`deleted`), "Create Tenant" button
- Row click navigates to `/tenants/[tenantId]`
- Status rendered as colored `StatusBadge`
- Server-side pagination (cursor-based from API)

**Detail Page** (`/tenants/[tenantId]`):
- Header: tenant name, slug, status badge, action dropdown (Edit, Suspend, Activate, Delete)
- Tabs:
  - **Overview**: created/updated dates, metadata
  - **Service Accounts**: filtered SA list for this tenant
  - **Policies**: filtered policy list for this tenant
  - **Usage**: tenant-scoped usage mini-dashboard
- Delete requires `ConfirmDialog` with slug confirmation (type tenant slug to confirm)

**Create/Edit Form**:
- Fields: name (required), slug (required, auto-generated from name), status (edit only)
- Zod validation with Superforms progressive enhancement
- Slug uniqueness validated server-side

```typescript
// src/lib/schemas/tenant.ts
import { z } from 'zod';

export const createTenantSchema = z.object({
  name: z.string().min(2).max(100),
  slug: z.string().min(2).max(50).regex(/^[a-z0-9-]+$/, 'Slug must be lowercase alphanumeric with hyphens')
});

export const updateTenantSchema = createTenantSchema.extend({
  status: z.enum(['active', 'suspended', 'deleted']).optional()
});

export type CreateTenantInput = z.infer<typeof createTenantSchema>;
export type UpdateTenantInput = z.infer<typeof updateTenantSchema>;
```

### 5.3 Service Accounts

**URL**: `/service-accounts`

**List Page**:
- `DataTable` with columns: Name, Slug, Tenant, Environment, Status, Keys (count), Created At
- Toolbar: search, filter by tenant (dropdown), filter by environment (`production`/`staging`/`development`), filter by status
- Faceted filter chips showing active filters

**Detail Page** (`/service-accounts/[accountId]`):
- Header: SA name, environment badge, status badge, actions (Edit, Suspend, Activate, Revoke)
- Summary cards: key count, active policies, last activity
- Tabs:
  - **Overview**: metadata, description, default policy link
  - **Keys**: key list with register/revoke actions (see 5.4)
  - **Policies**: bound policies with bind/unbind controls
  - **Usage**: SA-scoped usage charts

**Create/Edit Form**:
- Fields: tenant (select), name, slug, environment (select), description, default policy (optional select)
- Environment tagged with colored badges

```typescript
// src/lib/schemas/service-account.ts
import { z } from 'zod';

export const createServiceAccountSchema = z.object({
  tenant_id: z.string().uuid(),
  name: z.string().min(2).max(100),
  slug: z.string().min(2).max(50).regex(/^[a-z0-9-]+$/),
  environment: z.enum(['production', 'staging', 'development']),
  description: z.string().max(500).optional(),
  default_policy_id: z.string().uuid().optional()
});
```

### 5.4 Keys

**URL**: `/service-accounts/[accountId]/keys`

**List Page**:
- Table columns: Key ID, Algorithm, Fingerprint (truncated + copy), Status, Expires At, Last Used At, Created At
- Status badges: `active` (green), `rotating` (amber), `revoked` (red), `expired` (gray)
- Actions per row: Revoke (with confirmation), Copy Fingerprint
- "Register New Key" button

**Register Key Form** (`/service-accounts/[accountId]/keys/register`):
- Fields:
  - Algorithm: `ed25519` (default, read-only for v1)
  - Public Key PEM: textarea with monospace font, paste-friendly
  - Expires At: optional date picker
- On success: display key_id in a highlighted box with copy button, warn user this is shown once
- Validation: PEM format check client-side, cryptographic validation server-side

**Revoke Flow**:
1. Click "Revoke" on key row
2. `ConfirmDialog` opens: "Are you sure? This action cannot be undone."
3. Shows key fingerprint for verification
4. On confirm: `POST /admin/service-accounts/:id/keys/:keyId/revoke`
5. Optimistic update: immediately mark as `revoked` in UI
6. Toast notification on success/failure

```svelte
<!-- Key revocation confirmation pattern -->
<script lang="ts">
  import { ConfirmDialog } from '$lib/components/shared';
  import { enhance } from '$app/forms';

  let revokeTarget = $state<{ keyId: string; fingerprint: string } | null>(null);
</script>

{#if revokeTarget}
  <ConfirmDialog
    title="Revoke Key"
    description="This will immediately invalidate all requests signed with this key."
    confirmLabel="Revoke Key"
    variant="destructive"
    onconfirm={() => { /* form submit */ }}
    oncancel={() => { revokeTarget = null; }}
  >
    <p class="text-sm text-muted-foreground">
      Fingerprint: <code>{revokeTarget.fingerprint}</code>
    </p>
  </ConfirmDialog>
{/if}
```

### 5.5 Policies

**URL**: `/policies`

**List Page**:
- Table: Name, Tenant, Allowed Models (tag chips), Max Tokens, Streaming, Tools, Budget, Created At
- Filter by tenant
- Click to view detail

**Policy Builder UI** (`/policies/new`, `/policies/[policyId]/edit`):

The policy form is divided into collapsible sections:

1. **Basic Info**:
   - Name (required), Description (optional), Tenant (select)

2. **Model Access Control**:
   - Allowed Models: multi-select with tag input (type to search/add model aliases)
   - Denied Models: multi-select (same pattern)
   - Visual: green tags for allowed, red tags for denied

3. **Token Limits**:
   - Max Input Tokens: number input with slider
   - Max Output Tokens: number input with slider
   - Visual min/max bounds display

4. **Feature Flags** (toggle switches):
   - Allow Streaming: `Switch` component
   - Allow Tools: `Switch` component
   - Allow Files: `Switch` component

5. **Rate Limits**:
   - RPM Limit: number input
   - Concurrency Limit: number input

6. **Budget**:
   - Daily Budget: currency input (USD)
   - Monthly Budget: currency input (USD)

7. **Region Restrictions**:
   - Allowed Regions: multi-select (us-east, us-west, eu-west, etc.)

```typescript
// src/lib/schemas/policy.ts
import { z } from 'zod';

export const createPolicySchema = z.object({
  tenant_id: z.string().uuid(),
  name: z.string().min(2).max(100),
  description: z.string().max(500).optional(),
  allowed_models_json: z.array(z.string()).default([]),
  denied_models_json: z.array(z.string()).default([]),
  max_input_tokens: z.number().int().positive().optional(),
  max_output_tokens: z.number().int().positive().optional(),
  allow_streaming: z.boolean().default(true),
  allow_tools: z.boolean().default(false),
  allow_files: z.boolean().default(false),
  rpm_limit: z.number().int().positive().optional(),
  concurrency_limit: z.number().int().positive().optional(),
  daily_budget: z.number().positive().optional(),
  monthly_budget: z.number().positive().optional(),
  allowed_regions_json: z.array(z.string()).default([])
});
```

### 5.6 Provider Routes

**URL**: `/routes`

**Route Configuration Table**:
- Grouped by `model_alias` (collapsible sections)
- Within each group, rows sorted by `priority` (lowest number = highest priority)
- Columns: Provider, Provider Model Name, Priority, Enabled, Timeout (ms), Max Retries, Fallback Group, Region
- Drag handle column for reordering priority within a group
- Toggle switch for enable/disable (instant update via form action)
- Edit button per row

**Priority Reorder**:
- Drag-and-drop rows within a model alias group
- On drop: submit new priority order via form action
- Optimistic reorder in UI

**Create/Edit Route Form**:
- Fields:
  - Tenant (optional select -- null = global route)
  - Model Alias (text input with autocomplete from existing aliases)
  - Provider (select: `openai`, `anthropic`, `azure_openai`, `local_vllm`)
  - Provider Model Name (text, e.g., `gpt-4o`, `claude-3.5-sonnet`)
  - Priority (number, 1-1000)
  - Enabled (toggle)
  - Timeout (ms, number, default 30000)
  - Max Retries (number, default 1)
  - Retry Backoff (ms, number, default 250)
  - Fallback Group (optional text)
  - Region (optional text)

```typescript
// src/lib/schemas/route.ts
import { z } from 'zod';

export const createRouteSchema = z.object({
  tenant_id: z.string().uuid().optional(),
  model_alias: z.string().min(1).max(100),
  provider: z.enum(['openai', 'anthropic', 'azure_openai', 'local_vllm']),
  provider_model_name: z.string().min(1).max(200),
  priority: z.number().int().min(1).max(1000).default(100),
  enabled: z.boolean().default(true),
  timeout_ms: z.number().int().min(1000).max(300000).default(30000),
  max_retries: z.number().int().min(0).max(5).default(1),
  retry_backoff_ms: z.number().int().min(0).max(10000).default(250),
  fallback_group: z.string().max(100).optional(),
  region: z.string().max(50).optional()
});
```

### 5.7 Usage Analytics

**URL**: `/usage`

**Layout**: filter panel (top) + chart area + data table (below)

**Filter Panel**:
- Tenant: select dropdown (multi-select)
- Service Account: select dropdown (cascading from tenant)
- Model: multi-select
- Provider: multi-select
- Date Range: start/end date picker with presets (Last 24h, 7d, 30d, Custom)
- "Apply Filters" button + "Reset" link
- Active filters shown as removable chips

**Charts Section**:
1. **Token Usage Over Time** (`TimeSeriesChart`):
   - Line chart with prompt_tokens and completion_tokens stacked
   - Configurable aggregation: hourly/daily/weekly

2. **Cost Over Time** (`TimeSeriesChart`):
   - Stacked area chart by provider
   - Cumulative cost line overlay

3. **Request Distribution** (`BarChart`):
   - Bar chart: requests by model_alias
   - Color-coded by final_status (success/error)

4. **Latency Percentiles** (`TimeSeriesChart`):
   - P50, P95, P99 latency lines

**Data Table** (below charts):
- All usage events matching filters
- Columns: Request ID (truncated + copy), Timestamp, Tenant, SA, Model, Provider, Tokens (in/out/total), Cost, Latency, Status, Streaming
- CSV export button
- Server-side pagination

**Implementation Notes**:
- Chart data is loaded server-side initially, then client-side filter changes trigger `fetch` calls
- Charts use LayerChart 2.0 components with D3 scales
- Export uses a streaming download endpoint to handle large datasets

```svelte
<!-- Filter-driven data fetching pattern -->
<script lang="ts">
  import type { PageData } from './$types';
  import { goto } from '$app/navigation';
  import { page } from '$app/stores';

  let { data }: { data: PageData } = $props();

  // Reactive filters from URL search params
  let filters = $derived({
    tenant_id: $page.url.searchParams.get('tenant_id') ?? '',
    from: $page.url.searchParams.get('from') ?? '',
    to: $page.url.searchParams.get('to') ?? '',
    model: $page.url.searchParams.get('model') ?? ''
  });

  function applyFilters(newFilters: Record<string, string>) {
    const params = new URLSearchParams();
    for (const [key, value] of Object.entries(newFilters)) {
      if (value) params.set(key, value);
    }
    goto(`?${params.toString()}`, { replaceState: true, noScroll: true });
  }
</script>
```

### 5.8 Audit Logs

**URL**: `/audit`

**Layout**: searchable, filterable event stream

**Filter Bar**:
- Search: full-text search on action, target_type, target_id
- Action filter: dropdown (created, updated, deleted, revoked, suspended, etc.)
- Actor filter: select dropdown
- Target Type filter: select (tenant, service_account, key, policy, route)
- Tenant filter: select dropdown
- Date range: picker with presets

**Event Stream**:
- Reverse-chronological list (newest first)
- Each event as a card/row:
  - Action badge (colored by category: create=green, update=blue, delete=red, revoke=orange)
  - Actor (name + type icon)
  - Description: "[Actor] [action] [target_type] [target_id]"
  - Relative timestamp ("2 minutes ago")
  - Expandable metadata section (`JsonViewer` for `metadata_json`)
- Infinite scroll or "Load More" pagination
- Real-time: new events prepended via SSE (optional, v2)

**Event Detail Panel**:
- Click an event to expand an inline detail panel (or side sheet on desktop)
- Full metadata JSON display
- Link to related resource (e.g., click tenant_id to navigate to tenant detail)

```typescript
// src/routes/(app)/audit/+page.server.ts
import type { PageServerLoad } from './$types';

export const load: PageServerLoad = async ({ locals, url }) => {
  const params = {
    tenant_id: url.searchParams.get('tenant_id') || undefined,
    action: url.searchParams.get('action') || undefined,
    from: url.searchParams.get('from') || undefined,
    to: url.searchParams.get('to') || undefined,
    limit: 50,
    cursor: url.searchParams.get('cursor') || undefined
  };

  const { data } = await locals.api.GET('/admin/audit', {
    params: { query: params }
  });

  return {
    events: data?.items ?? [],
    nextCursor: data?.next_cursor ?? null,
    user: locals.user
  };
};
```

### 5.9 Settings

**URL**: `/settings`

**System Settings** (admin-only):
- Gateway base URL display
- Provider API key status (configured/missing, never show actual keys)
- Global rate limit defaults
- Default timeout configuration

**Profile** (`/settings/profile`):
- User name, email (read-only)
- Change password form
- Session info: last login, active sessions

---

## 6. Component Architecture

### 6.1 AppShell Layout

```svelte
<!-- src/lib/components/layout/AppShell.svelte -->
<script lang="ts">
  import Sidebar from './Sidebar.svelte';
  import Header from './Header.svelte';
  import { sidebarState } from '$lib/stores/sidebar.svelte';
  import type { Snippet } from 'svelte';

  let { children }: { children: Snippet } = $props();
</script>

<div class="flex h-screen overflow-hidden bg-background">
  <Sidebar collapsed={sidebarState.collapsed} />
  <div class="flex flex-1 flex-col overflow-hidden">
    <Header />
    <main
      class="flex-1 overflow-y-auto p-6"
      id="main-content"
    >
      {@render children()}
    </main>
  </div>
</div>
```

### 6.2 Sidebar Navigation

```svelte
<!-- src/lib/components/layout/Sidebar.svelte -->
<script lang="ts">
  import { page } from '$app/stores';
  import { navItems } from '$lib/constants/navigation';
  import { sidebarState } from '$lib/stores/sidebar.svelte';
  import * as Tooltip from '$lib/components/ui/tooltip';

  let { collapsed }: { collapsed: boolean } = $props();
</script>

<aside
  class="flex h-full flex-col border-r bg-card transition-[width] duration-200
         {collapsed ? 'w-16' : 'w-64'}"
  role="navigation"
  aria-label="Main navigation"
>
  <div class="flex h-14 items-center border-b px-4">
    {#if !collapsed}
      <span class="text-lg font-semibold">LLMSmartGate</span>
    {:else}
      <span class="text-lg font-semibold">LG</span>
    {/if}
  </div>

  <nav class="flex-1 space-y-1 p-2">
    {#each navItems as item}
      {@const isActive = $page.url.pathname.startsWith(item.href)}
      <Tooltip.Root>
        <Tooltip.Trigger>
          <a
            href={item.href}
            class="flex items-center gap-3 rounded-md px-3 py-2 text-sm
                   transition-colors
                   {isActive
                     ? 'bg-primary/10 text-primary font-medium'
                     : 'text-muted-foreground hover:bg-accent hover:text-accent-foreground'}"
            aria-current={isActive ? 'page' : undefined}
          >
            <item.icon class="h-5 w-5 shrink-0" />
            {#if !collapsed}
              <span>{item.label}</span>
              {#if item.badge}
                <span class="ml-auto rounded-full bg-primary/10 px-2 py-0.5 text-xs">
                  {item.badge}
                </span>
              {/if}
            {/if}
          </a>
        </Tooltip.Trigger>
        {#if collapsed}
          <Tooltip.Content side="right">{item.label}</Tooltip.Content>
        {/if}
      </Tooltip.Root>
    {/each}
  </nav>

  <button
    onclick={() => sidebarState.toggle()}
    class="flex h-10 items-center justify-center border-t"
    aria-label={collapsed ? 'Expand sidebar' : 'Collapse sidebar'}
  >
    {#if collapsed}
      <ChevronRight class="h-4 w-4" />
    {:else}
      <ChevronLeft class="h-4 w-4" />
    {/if}
  </button>
</aside>
```

### 6.3 Navigation Constants

```typescript
// src/lib/constants/navigation.ts
import {
  LayoutDashboard,
  Building2,
  Users,
  Shield,
  Route,
  BarChart3,
  ScrollText,
  Settings
} from 'lucide-svelte';

export const navItems = [
  { label: 'Dashboard', href: '/dashboard', icon: LayoutDashboard },
  { label: 'Tenants', href: '/tenants', icon: Building2 },
  { label: 'Service Accounts', href: '/service-accounts', icon: Users },
  { label: 'Policies', href: '/policies', icon: Shield },
  { label: 'Routes', href: '/routes', icon: Route },
  { label: 'Usage', href: '/usage', icon: BarChart3 },
  { label: 'Audit Logs', href: '/audit', icon: ScrollText },
  { label: 'Settings', href: '/settings', icon: Settings }
] as const;
```

### 6.4 DataTable Component

The `DataTable` wraps `@tanstack/svelte-table` with shadcn-svelte's Table primitives, providing a consistent, reusable pattern across all list pages.

```svelte
<!-- src/lib/components/data-table/DataTable.svelte -->
<script lang="ts" generics="TData">
  import {
    type ColumnDef,
    type PaginationState,
    type SortingState,
    type ColumnFiltersState,
    type VisibilityState,
    getCoreRowModel,
    getPaginationRowModel,
    getSortedRowModel,
    getFilteredRowModel,
    createSvelteTable,
    FlexRender
  } from '@tanstack/svelte-table';
  import * as Table from '$lib/components/ui/table';
  import DataTablePagination from './DataTablePagination.svelte';
  import type { Snippet } from 'svelte';

  let {
    data,
    columns,
    toolbar,
    pageSize = 20,
    serverSide = false,
    totalCount = 0,
    onPageChange,
    onSortChange
  }: {
    data: TData[];
    columns: ColumnDef<TData, unknown>[];
    toolbar?: Snippet;
    pageSize?: number;
    serverSide?: boolean;
    totalCount?: number;
    onPageChange?: (page: number) => void;
    onSortChange?: (sorting: SortingState) => void;
  } = $props();

  let sorting = $state<SortingState>([]);
  let columnFilters = $state<ColumnFiltersState>([]);
  let columnVisibility = $state<VisibilityState>({});
  let pagination = $state<PaginationState>({
    pageIndex: 0,
    pageSize
  });

  const table = createSvelteTable({
    get data() { return data; },
    columns,
    getCoreRowModel: getCoreRowModel(),
    getPaginationRowModel: serverSide ? undefined : getPaginationRowModel(),
    getSortedRowModel: serverSide ? undefined : getSortedRowModel(),
    getFilteredRowModel: serverSide ? undefined : getFilteredRowModel(),
    state: {
      get sorting() { return sorting; },
      get columnFilters() { return columnFilters; },
      get columnVisibility() { return columnVisibility; },
      get pagination() { return pagination; }
    },
    onSortingChange(updater) {
      sorting = typeof updater === 'function' ? updater(sorting) : updater;
      onSortChange?.(sorting);
    },
    onPaginationChange(updater) {
      pagination = typeof updater === 'function' ? updater(pagination) : updater;
      onPageChange?.(pagination.pageIndex);
    },
    onColumnFiltersChange(updater) {
      columnFilters = typeof updater === 'function' ? updater(columnFilters) : updater;
    },
    onColumnVisibilityChange(updater) {
      columnVisibility = typeof updater === 'function' ? updater(columnVisibility) : updater;
    },
    ...(serverSide ? { manualPagination: true, pageCount: Math.ceil(totalCount / pageSize) } : {})
  });
</script>

{#if toolbar}
  {@render toolbar()}
{/if}

<div class="rounded-md border">
  <Table.Root>
    <Table.Header>
      {#each table.getHeaderGroups() as headerGroup}
        <Table.Row>
          {#each headerGroup.headers as header}
            <Table.Head>
              {#if !header.isPlaceholder}
                <FlexRender
                  content={header.column.columnDef.header}
                  context={header.getContext()}
                />
              {/if}
            </Table.Head>
          {/each}
        </Table.Row>
      {/each}
    </Table.Header>
    <Table.Body>
      {#each table.getRowModel().rows as row}
        <Table.Row>
          {#each row.getVisibleCells() as cell}
            <Table.Cell>
              <FlexRender
                content={cell.column.columnDef.cell}
                context={cell.getContext()}
              />
            </Table.Cell>
          {/each}
        </Table.Row>
      {:else}
        <Table.Row>
          <Table.Cell colspan={columns.length} class="h-24 text-center">
            No results found.
          </Table.Cell>
        </Table.Row>
      {/each}
    </Table.Body>
  </Table.Root>
</div>

<DataTablePagination {table} />
```

### 6.5 Toast/Notification System

Using shadcn-svelte's Sonner integration:

```svelte
<!-- src/routes/+layout.svelte -->
<script lang="ts">
  import { ModeWatcher } from 'mode-watcher';
  import { Toaster } from '$lib/components/ui/sonner';
  import type { Snippet } from 'svelte';

  let { children }: { children: Snippet } = $props();
</script>

<ModeWatcher />
<Toaster richColors closeButton />
{@render children()}
```

Usage anywhere:
```typescript
import { toast } from 'svelte-sonner';

// Success
toast.success('Tenant created successfully');

// Error with correlation ID
toast.error('Failed to create tenant', {
  description: `Correlation ID: ${correlationId}`
});

// Promise-based
toast.promise(createTenant(data), {
  loading: 'Creating tenant...',
  success: 'Tenant created!',
  error: 'Failed to create tenant'
});
```

### 6.6 Loading States & Skeletons

```svelte
<!-- src/lib/components/feedback/LoadingSkeleton.svelte -->
<script lang="ts">
  import { Skeleton } from '$lib/components/ui/skeleton';

  let { variant = 'table' }: { variant: 'table' | 'form' | 'cards' | 'detail' } = $props();
</script>

{#if variant === 'table'}
  <div class="space-y-3">
    <div class="flex items-center gap-4">
      <Skeleton class="h-10 w-64" />
      <Skeleton class="ml-auto h-10 w-32" />
    </div>
    <div class="rounded-md border">
      {#each { length: 5 } as _}
        <div class="flex items-center gap-4 border-b p-4">
          <Skeleton class="h-4 w-48" />
          <Skeleton class="h-4 w-24" />
          <Skeleton class="h-4 w-20" />
          <Skeleton class="ml-auto h-4 w-16" />
        </div>
      {/each}
    </div>
  </div>
{:else if variant === 'cards'}
  <div class="grid grid-cols-1 gap-4 md:grid-cols-2 xl:grid-cols-4">
    {#each { length: 4 } as _}
      <Skeleton class="h-32 rounded-xl" />
    {/each}
  </div>
{/if}
```

### 6.7 Error Display

```svelte
<!-- src/lib/components/feedback/ErrorPanel.svelte -->
<script lang="ts">
  import { AlertCircle } from 'lucide-svelte';
  import * as Alert from '$lib/components/ui/alert';
  import { Button } from '$lib/components/ui/button';

  let {
    title = 'Something went wrong',
    message,
    correlationId,
    onretry
  }: {
    title?: string;
    message: string;
    correlationId?: string;
    onretry?: () => void;
  } = $props();
</script>

<Alert.Root variant="destructive">
  <AlertCircle class="h-4 w-4" />
  <Alert.Title>{title}</Alert.Title>
  <Alert.Description>
    <p>{message}</p>
    {#if correlationId}
      <p class="mt-1 text-xs opacity-70">
        Correlation ID: <code>{correlationId}</code>
      </p>
    {/if}
    {#if onretry}
      <Button variant="outline" size="sm" class="mt-3" onclick={onretry}>
        Try Again
      </Button>
    {/if}
  </Alert.Description>
</Alert.Root>
```

---

## 7. State Management

### 7.1 Svelte 5 Runes Philosophy

Following Svelte 5 best practices for 2026:

- **`$state`**: reactive variables that trigger re-renders. Use `$state.raw` for large immutable objects (API responses) to avoid deep proxy overhead.
- **`$derived`**: pure computed values from state. Memoized, no side effects. Replaces computed getters.
- **`$effect`**: escape hatch for side effects (SSE subscriptions, DOM manipulation). Avoid updating `$state` inside effects when possible.
- **Reactive Classes**: replace Svelte 4 stores for cross-component state. Runes work inside `.svelte.ts` files.

**Evidence**: The Svelte docs explicitly state "only use the $state rune for variables that should be reactive" and "for large objects that are only ever reassigned, use $state.raw instead" ([Svelte docs: Best practices](https://svelte.dev/docs/svelte/best-practices)). Effects "are an escape hatch and should mostly be avoided" ([Svelte docs: $effect](https://svelte.dev/docs/svelte/$effect)).

### 7.2 Auth State (Reactive Class Pattern)

```typescript
// src/lib/stores/auth.svelte.ts

interface AdminUser {
  id: string;
  email: string;
  name: string;
  role: 'admin' | 'viewer';
}

class AuthState {
  user = $state<AdminUser | null>(null);
  isAuthenticated = $derived(this.user !== null);
  isAdmin = $derived(this.user?.role === 'admin');

  setUser(user: AdminUser | null) {
    this.user = user;
  }

  hasPermission(permission: string): boolean {
    if (!this.user) return false;
    if (this.user.role === 'admin') return true;
    // Viewer can only read
    return permission === 'read';
  }
}

export const authState = new AuthState();
```

### 7.3 Sidebar State (Persistent)

```typescript
// src/lib/stores/sidebar.svelte.ts
import { browser } from '$app/environment';

class SidebarState {
  collapsed = $state(false);

  constructor() {
    if (browser) {
      const stored = localStorage.getItem('sidebar-collapsed');
      if (stored !== null) {
        this.collapsed = stored === 'true';
      }
    }
  }

  toggle() {
    this.collapsed = !this.collapsed;
    if (browser) {
      localStorage.setItem('sidebar-collapsed', String(this.collapsed));
    }
  }
}

export const sidebarState = new SidebarState();
```

### 7.4 Server State vs Client State

| Concern | Location | Pattern |
|---------|----------|---------|
| List data (tenants, SAs, policies) | Server `+page.server.ts` | `load` function, returned as `data` prop |
| Form state | Client | Superforms `$form` rune + Zod schema |
| Filter/search params | URL | `$page.url.searchParams` + `goto()` |
| User session | Server `locals` | `hooks.server.ts` → `event.locals.user` |
| UI preferences (sidebar, theme) | Client | Reactive class + `localStorage` |
| Real-time metrics | Client | SSE → `$state` |
| Optimistic updates | Client | Temporary `$state` mutation, reverted on error |

### 7.5 Load Function Pattern

```typescript
// Standard load pattern for list pages
// src/routes/(app)/tenants/+page.server.ts
import type { PageServerLoad } from './$types';
import { superValidate } from 'sveltekit-superforms';
import { zod } from 'sveltekit-superforms/adapters';
import { createTenantSchema } from '$lib/schemas/tenant';

export const load: PageServerLoad = async ({ locals, url }) => {
  const search = url.searchParams.get('search') ?? '';
  const status = url.searchParams.get('status') ?? '';
  const page = parseInt(url.searchParams.get('page') ?? '1');

  const { data: tenants } = await locals.api.GET('/admin/tenants', {
    params: {
      query: {
        search: search || undefined,
        status: status || undefined,
        page,
        limit: 20
      }
    }
  });

  // Pre-create form for inline creation modal
  const form = await superValidate(zod(createTenantSchema));

  return {
    tenants: tenants ?? [],
    form,
    user: locals.user
  };
};
```

### 7.6 Optimistic Updates Pattern

```svelte
<script lang="ts">
  import { enhance } from '$app/forms';
  import { toast } from 'svelte-sonner';

  let { data } = $props();
  let tenants = $state(data.tenants);

  // Sync with server data on navigation
  $effect(() => {
    tenants = data.tenants;
  });
</script>

<form
  method="POST"
  action="?/updateStatus"
  use:enhance={({ formData }) => {
    const id = formData.get('id') as string;
    const newStatus = formData.get('status') as string;

    // Optimistic: update immediately
    const index = tenants.findIndex(t => t.id === id);
    const previousStatus = tenants[index]?.status;
    if (index !== -1) {
      tenants[index] = { ...tenants[index], status: newStatus };
    }

    return async ({ result }) => {
      if (result.type === 'failure') {
        // Revert on failure
        if (index !== -1 && previousStatus) {
          tenants[index] = { ...tenants[index], status: previousStatus };
        }
        toast.error('Failed to update status');
      } else {
        toast.success('Status updated');
      }
    };
  }}
>
  <!-- form fields -->
</form>
```

### 7.7 Cache Invalidation

SvelteKit's `invalidateAll()` and `invalidate(url)` are used for cache invalidation:

```typescript
import { invalidateAll, invalidate } from '$app/navigation';

// After a mutation, invalidate the current page data
await invalidateAll();

// Or invalidate a specific dependency
await invalidate('/admin/tenants');
```

For cross-page invalidation after form actions, SvelteKit automatically re-runs the load function for the current page. For mutations that affect other pages (e.g., deleting a tenant also invalidates the SA list), the `depends` mechanism is used:

```typescript
// In load function
export const load: PageServerLoad = async ({ depends }) => {
  depends('app:tenants');
  // ...
};

// After mutation
invalidate('app:tenants');
```

---

## 8. API Integration

### 8.1 Type Generation from OpenAPI

```bash
# Generate types from the Admin API OpenAPI spec
npx openapi-typescript ./openapi/admin-api.yaml -o ./src/lib/api/types.d.ts
```

This produces a complete TypeScript type map matching every endpoint, request body, and response schema defined in the Admin API OpenAPI spec (Section 10.2 of the backend LLD).

### 8.2 API Client (Server-Side)

```typescript
// src/lib/api/client.server.ts
import createClient from 'openapi-fetch';
import type { paths } from './types';
import { ADMIN_API_BASE_URL } from '$env/static/private';

export function createAdminApiClient(accessToken: string) {
  return createClient<paths>({
    baseUrl: ADMIN_API_BASE_URL,
    headers: {
      Authorization: `Bearer ${accessToken}`,
      'Content-Type': 'application/json'
    }
  });
}

export type ApiClient = ReturnType<typeof createAdminApiClient>;
```

### 8.3 Request/Response Interceptors

```typescript
// src/lib/api/interceptors.ts
import type { Middleware } from 'openapi-fetch';

export const correlationMiddleware: Middleware = {
  async onRequest({ request }) {
    // Attach correlation ID from AsyncLocalStorage or generate one
    const correlationId = crypto.randomUUID();
    request.headers.set('X-Correlation-Id', correlationId);
    return request;
  }
};

export const errorMiddleware: Middleware = {
  async onResponse({ response }) {
    if (!response.ok) {
      const body = await response.clone().json().catch(() => ({}));
      const error = {
        status: response.status,
        code: body?.error?.code ?? 'UNKNOWN_ERROR',
        message: body?.error?.message ?? response.statusText,
        correlationId: response.headers.get('x-correlation-id')
      };

      // Log server-side for observability
      console.error('[API Error]', JSON.stringify(error));
    }
    return response;
  }
};

export const retryMiddleware: Middleware = {
  async onResponse({ response, request, options }) {
    // Retry on 502/503/504 with exponential backoff (max 2 retries)
    const retryStatuses = [502, 503, 504];
    const maxRetries = 2;
    const retryCount = parseInt(request.headers.get('x-retry-count') ?? '0');

    if (retryStatuses.includes(response.status) && retryCount < maxRetries) {
      const delay = Math.pow(2, retryCount) * 500;
      await new Promise((resolve) => setTimeout(resolve, delay));
      request.headers.set('x-retry-count', String(retryCount + 1));
      return fetch(request);
    }

    return response;
  }
};
```

### 8.4 Applying Middleware to Client

```typescript
// src/lib/api/client.server.ts (extended)
import { correlationMiddleware, errorMiddleware, retryMiddleware } from './interceptors';

export function createAdminApiClient(accessToken: string) {
  const client = createClient<paths>({
    baseUrl: ADMIN_API_BASE_URL,
    headers: {
      Authorization: `Bearer ${accessToken}`,
      'Content-Type': 'application/json'
    }
  });

  client.use(correlationMiddleware);
  client.use(errorMiddleware);
  client.use(retryMiddleware);

  return client;
}
```

### 8.5 SSE Endpoint for Real-Time Data

```typescript
// src/routes/api/sse/usage/+server.ts
import type { RequestHandler } from './$types';

export const GET: RequestHandler = async ({ locals }) => {
  const stream = new ReadableStream({
    start(controller) {
      const encoder = new TextEncoder();

      const interval = setInterval(async () => {
        try {
          const { data } = await locals.api.GET('/admin/usage/summary', {
            params: { query: { period: '5m' } }
          });

          const event = `data: ${JSON.stringify(data)}\n\n`;
          controller.enqueue(encoder.encode(event));
        } catch (error) {
          // Silently continue on transient errors
          console.error('[SSE] Failed to fetch usage:', error);
        }
      }, 5000); // Poll every 5 seconds

      // Cleanup on disconnect
      return () => clearInterval(interval);
    }
  });

  return new Response(stream, {
    headers: {
      'Content-Type': 'text/event-stream',
      'Cache-Control': 'no-cache',
      Connection: 'keep-alive'
    }
  });
};
```

### 8.6 Error Handling Strategy

| Error Type | HTTP Status | Frontend Behavior |
|------------|-------------|-------------------|
| Validation error | 400/422 | Inline field errors via Superforms |
| Unauthorized | 401 | Redirect to `/login`, clear cookies |
| Forbidden | 403 | Toast error + disable action |
| Not found | 404 | SvelteKit `+error.svelte` page |
| Conflict (duplicate) | 409 | Toast with "already exists" message |
| Rate limited | 429 | Toast with retry-after countdown |
| Server error | 500 | `ErrorPanel` with correlation ID + retry button |
| Network error | N/A | Toast "Connection lost", auto-retry |

**Evidence**: openapi-fetch provides type-safe response handling with discriminated unions for success/error cases, and its middleware API supports request/response interception ([openapi-fetch docs](https://openapi-ts.dev/openapi-fetch/)).

---

## 9. Styling & Theming

### 9.1 TailwindCSS v4 Configuration

TailwindCSS v4 uses CSS-first configuration. The `tailwind.config.js` file is minimal or absent; all customization happens in `app.css`.

```css
/* src/app.css */
@import 'tailwindcss';

@theme {
  /* Color tokens -- shadcn-svelte uses oklch */
  --color-background: oklch(1 0 0);
  --color-foreground: oklch(0.145 0 0);
  --color-card: oklch(1 0 0);
  --color-card-foreground: oklch(0.145 0 0);
  --color-popover: oklch(1 0 0);
  --color-popover-foreground: oklch(0.145 0 0);
  --color-primary: oklch(0.205 0 0);
  --color-primary-foreground: oklch(0.985 0 0);
  --color-secondary: oklch(0.97 0 0);
  --color-secondary-foreground: oklch(0.205 0 0);
  --color-muted: oklch(0.97 0 0);
  --color-muted-foreground: oklch(0.556 0 0);
  --color-accent: oklch(0.97 0 0);
  --color-accent-foreground: oklch(0.205 0 0);
  --color-destructive: oklch(0.577 0.245 27.325);
  --color-destructive-foreground: oklch(0.577 0.245 27.325);
  --color-border: oklch(0.922 0 0);
  --color-input: oklch(0.922 0 0);
  --color-ring: oklch(0.708 0 0);

  /* Sidebar-specific tokens */
  --color-sidebar: oklch(0.985 0 0);
  --color-sidebar-foreground: oklch(0.145 0 0);
  --color-sidebar-accent: oklch(0.97 0 0);
  --color-sidebar-accent-foreground: oklch(0.205 0 0);
  --color-sidebar-border: oklch(0.922 0 0);

  /* Chart colors */
  --color-chart-1: oklch(0.646 0.222 41.116);
  --color-chart-2: oklch(0.6 0.118 184.714);
  --color-chart-3: oklch(0.398 0.07 227.392);
  --color-chart-4: oklch(0.828 0.189 84.429);
  --color-chart-5: oklch(0.769 0.188 70.08);

  /* Border radius */
  --radius-sm: 0.25rem;
  --radius-md: 0.375rem;
  --radius-lg: 0.5rem;
  --radius-xl: 0.75rem;

  /* Font */
  --font-sans: 'Inter', ui-sans-serif, system-ui, sans-serif;
  --font-mono: 'JetBrains Mono', ui-monospace, monospace;
}

/* Dark mode overrides */
.dark {
  --color-background: oklch(0.145 0 0);
  --color-foreground: oklch(0.985 0 0);
  --color-card: oklch(0.145 0 0);
  --color-card-foreground: oklch(0.985 0 0);
  --color-popover: oklch(0.145 0 0);
  --color-popover-foreground: oklch(0.985 0 0);
  --color-primary: oklch(0.985 0 0);
  --color-primary-foreground: oklch(0.205 0 0);
  --color-secondary: oklch(0.269 0 0);
  --color-secondary-foreground: oklch(0.985 0 0);
  --color-muted: oklch(0.269 0 0);
  --color-muted-foreground: oklch(0.708 0 0);
  --color-accent: oklch(0.269 0 0);
  --color-accent-foreground: oklch(0.985 0 0);
  --color-destructive: oklch(0.396 0.141 25.723);
  --color-destructive-foreground: oklch(0.637 0.237 25.331);
  --color-border: oklch(0.269 0 0);
  --color-input: oklch(0.269 0 0);
  --color-ring: oklch(0.439 0 0);
  --color-sidebar: oklch(0.175 0 0);
  --color-sidebar-foreground: oklch(0.985 0 0);
  --color-sidebar-accent: oklch(0.269 0 0);
  --color-sidebar-accent-foreground: oklch(0.985 0 0);
  --color-sidebar-border: oklch(0.269 0 0);
}
```

### 9.2 Dark/Light Mode Implementation

Using `mode-watcher` (recommended by shadcn-svelte documentation):

```svelte
<!-- src/lib/components/layout/ThemeToggle.svelte -->
<script lang="ts">
  import { toggleMode } from 'mode-watcher';
  import Sun from 'lucide-svelte/icons/sun';
  import Moon from 'lucide-svelte/icons/moon';
  import { Button } from '$lib/components/ui/button';
</script>

<Button
  variant="ghost"
  size="icon"
  onclick={toggleMode}
  aria-label="Toggle theme"
>
  <Sun class="h-5 w-5 rotate-0 scale-100 transition-all dark:-rotate-90 dark:scale-0" />
  <Moon class="absolute h-5 w-5 rotate-90 scale-0 transition-all dark:rotate-0 dark:scale-100" />
</Button>
```

The `ModeWatcher` component in the root layout handles:
- Initial theme detection from system preference or localStorage
- No flash of unstyled content (FOUC) on SSR/SSG
- Syncing theme across tabs via localStorage events
- Applying `dark` class to `<html>` element and `color-scheme` CSS property

**Evidence**: shadcn-svelte's official dark mode documentation recommends mode-watcher for Svelte apps ([shadcn-svelte: Dark Mode](https://www.shadcn-svelte.com/docs/dark-mode/svelte)).

### 9.3 Responsive Design Breakpoints

Using Tailwind v4 default breakpoints:

| Breakpoint | Width | Usage |
|------------|-------|-------|
| `sm` | 640px | Stack cards to 2-column |
| `md` | 768px | Show sidebar, 2-col grids |
| `lg` | 1024px | Sidebar + main content, 3-col grids |
| `xl` | 1280px | Full dashboard layout, 4-col KPIs |
| `2xl` | 1536px | Comfortable reading width |

**Mobile Strategy**: The sidebar collapses to a hamburger menu sheet on `md` and below. DataTable hides non-essential columns on smaller screens using `columnVisibility`.

---

## 10. Accessibility

### 10.1 WCAG 2.2 AA Compliance Strategy

The admin console targets WCAG 2.2 AA compliance for all interactive components.

**Evidence**: WCAG 2.2 AA is the globally accepted tier for accessibility compliance, required for EU and US public sector sites, and shadcn-svelte's underlying `bits-ui` primitives are built with WAI-ARIA patterns ([WCAG 2.2 Checklist](https://www.levelaccess.com/blog/wcag-2-2-aa-summary-and-checklist-for-website-owners/), [Bits UI](https://bits-ui.com/)).

### 10.2 Keyboard Navigation

| Context | Keys | Action |
|---------|------|--------|
| Sidebar | `Tab` / `Shift+Tab` | Navigate between items |
| Sidebar | `Enter` / `Space` | Activate link |
| DataTable | `Tab` | Move between interactive cells |
| DataTable | `Arrow Up/Down` | Navigate rows (when focused) |
| Dialog | `Escape` | Close dialog |
| Dialog | `Tab` | Cycle focus within dialog (focus trap) |
| Dropdown | `Arrow Up/Down` | Navigate options |
| Dropdown | `Enter` | Select option |
| Dropdown | `Escape` | Close dropdown |
| Global | `Ctrl+K` / `Cmd+K` | Open command palette (v2) |
| Form | `Tab` | Move between fields |
| Form | `Enter` | Submit (when on submit button) |

### 10.3 ARIA Strategy

- **Landmarks**: `<nav aria-label="Main navigation">`, `<main id="main-content">`, `<aside>`
- **Live regions**: Toast notifications use `aria-live="polite"`, error messages use `aria-live="assertive"`
- **Labels**: All form inputs have associated `<label>` elements via Formsnap
- **States**: `aria-current="page"` on active navigation item, `aria-expanded` on collapsible sections, `aria-sort` on sortable table columns
- **Descriptions**: `aria-describedby` links error messages to their inputs

### 10.4 Focus Management

- **Page navigation**: SvelteKit focuses `<body>` after navigation by default; custom `autofocus` on first heading or main content when appropriate
- **Skip link**: `<a href="#main-content" class="sr-only focus:not-sr-only">Skip to content</a>` in `+layout.svelte`
- **Dialog focus trap**: bits-ui dialogs automatically trap focus and return focus to trigger on close
- **After mutations**: focus moves to the success toast or the newly created item

```svelte
<!-- Skip to content link in root layout -->
<a
  href="#main-content"
  class="sr-only focus:not-sr-only focus:absolute focus:z-50 focus:bg-background
         focus:p-3 focus:text-foreground focus:shadow-lg"
>
  Skip to content
</a>
```

### 10.5 Screen Reader Considerations

- Status badges include visually hidden text: `<span class="sr-only">Status:</span> Active`
- Icon-only buttons always have `aria-label`
- Charts include `<table>` fallback with `aria-hidden="true"` on the visual chart and a summary for screen readers
- Loading skeletons use `aria-busy="true"` and `role="status"`
- DataTable row counts announced: `aria-label="Showing 1-20 of 150 tenants"`

**Evidence**: SvelteKit provides compile-time accessibility warnings and default focus management after navigation ([SvelteKit: Accessibility](https://svelte.dev/docs/kit/accessibility)).

---

## 11. Performance

### 11.1 SSR vs CSR Decisions

| Route | Rendering | Rationale |
|-------|-----------|-----------|
| `/login` | SSR | Fast first paint, form works without JS |
| `/dashboard` | SSR + CSR hydration | Initial metrics SSR, real-time SSE client-side |
| `/tenants`, `/service-accounts`, `/policies`, `/routes` | SSR | List data fetched server-side, SEO not needed but fast paint is |
| `/usage` | SSR initial + CSR interactions | Heavy chart interactivity requires client JS |
| `/audit` | SSR | Event list is text-heavy, SSR renders fast |
| All detail/edit pages | SSR | Data fetched in `load`, forms progressively enhanced |

### 11.2 Code Splitting

SvelteKit handles route-level code splitting automatically via Vite. Each route's `.svelte` and `.ts` files produce separate chunks.

Additional manual splitting for heavy components:

```typescript
// Lazy-load chart library only on dashboard/usage pages
const TimeSeriesChart = await import('$lib/components/charts/TimeSeriesChart.svelte');
```

```svelte
<!-- Conditional dynamic import for heavy components -->
<script lang="ts">
  import { onMount } from 'svelte';

  let ChartComponent = $state<typeof import('$lib/components/charts/TimeSeriesChart.svelte').default | null>(null);

  $effect(() => {
    import('$lib/components/charts/TimeSeriesChart.svelte').then((mod) => {
      ChartComponent = mod.default;
    });
  });
</script>

{#if ChartComponent}
  <ChartComponent data={chartData} />
{:else}
  <Skeleton class="h-64 w-full" />
{/if}
```

### 11.3 Preloading

SvelteKit's link preloading is enabled by default (`data-sveltekit-preload-data="hover"` on `<body>`). When users hover over sidebar navigation links, the target page's `load` function runs ahead of the click, reducing perceived navigation time.

For critical paths:

```svelte
<!-- Eager preload for common navigation -->
<a href="/dashboard" data-sveltekit-preload-data="eager">Dashboard</a>
```

### 11.4 Bundle Size Monitoring

```typescript
// vite.config.ts
import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

export default defineConfig({
  plugins: [sveltekit()],
  build: {
    rollupOptions: {
      output: {
        manualChunks: {
          // Keep chart libraries in a separate chunk
          charts: ['layerchart', 'd3-scale', 'd3-shape', 'd3-time-format']
        }
      }
    }
  }
});
```

**Target bundle sizes**:
- Initial JS (login page): < 80 KB gzipped
- App shell (sidebar + header): < 40 KB gzipped
- Chart chunk: < 60 KB gzipped (loaded on demand)
- Total per-page JS: < 50 KB gzipped (excluding shared chunks)

### 11.5 Image Optimization

- SVG for logos and icons (Lucide provides tree-shakeable SVG components)
- No raster images in the admin console (data-driven UI)
- `@sveltejs/enhanced-img` for any future marketing/help images

### 11.6 API Response Caching

```typescript
// SvelteKit load functions support cache headers
export const load: PageServerLoad = async ({ locals, setHeaders }) => {
  // Cache tenant list for 30 seconds (stale-while-revalidate)
  setHeaders({
    'cache-control': 'private, max-age=30, stale-while-revalidate=60'
  });

  const { data } = await locals.api.GET('/admin/tenants');
  return { tenants: data ?? [] };
};
```

**Evidence**: SvelteKit automatically handles route-level code splitting, and `data-sveltekit-preload-data` is configured by default on new projects ([SvelteKit: Performance](https://svelte.dev/docs/kit/performance)).

---

## 12. Testing

### 12.1 Unit Testing (Vitest)

Target: utility functions, schemas, formatters, business logic.

```typescript
// tests/unit/schemas/tenant.test.ts
import { describe, it, expect } from 'vitest';
import { createTenantSchema } from '$lib/schemas/tenant';

describe('createTenantSchema', () => {
  it('accepts valid tenant data', () => {
    const result = createTenantSchema.safeParse({
      name: 'Acme Corp',
      slug: 'acme-corp'
    });
    expect(result.success).toBe(true);
  });

  it('rejects slug with uppercase', () => {
    const result = createTenantSchema.safeParse({
      name: 'Acme',
      slug: 'Acme-Corp'
    });
    expect(result.success).toBe(false);
  });

  it('rejects empty name', () => {
    const result = createTenantSchema.safeParse({
      name: '',
      slug: 'acme'
    });
    expect(result.success).toBe(false);
  });
});
```

```typescript
// tests/unit/utils/format.test.ts
import { describe, it, expect } from 'vitest';
import { formatCurrency, formatNumber, formatRelativeTime } from '$lib/utils/format';

describe('formatCurrency', () => {
  it('formats USD values', () => {
    expect(formatCurrency(1234.56)).toBe('$1,234.56');
  });

  it('handles zero', () => {
    expect(formatCurrency(0)).toBe('$0.00');
  });
});
```

### 12.2 Component Testing (Vitest Browser Mode)

Using `@vitest/browser` with Playwright for real browser rendering:

```typescript
// tests/component/data-table/DataTable.test.ts
import { render } from 'vitest-browser-svelte';
import { describe, it, expect } from 'vitest';
import DataTable from '$lib/components/data-table/DataTable.svelte';

const columns = [
  { accessorKey: 'name', header: 'Name' },
  { accessorKey: 'status', header: 'Status' }
];

const data = [
  { name: 'Tenant A', status: 'active' },
  { name: 'Tenant B', status: 'suspended' }
];

describe('DataTable', () => {
  it('renders rows from data', async () => {
    const { getByText } = render(DataTable, { props: { data, columns } });
    expect(getByText('Tenant A')).toBeTruthy();
    expect(getByText('Tenant B')).toBeTruthy();
  });

  it('shows empty state when no data', async () => {
    const { getByText } = render(DataTable, { props: { data: [], columns } });
    expect(getByText('No results found.')).toBeTruthy();
  });
});
```

### 12.3 E2E Testing (Playwright)

```typescript
// tests/e2e/tenants.spec.ts
import { test, expect } from '@playwright/test';

test.describe('Tenants', () => {
  test.beforeEach(async ({ page }) => {
    // Login via API (skip UI login for speed)
    const response = await page.request.post('/api/auth/login', {
      data: { email: 'admin@example.com', password: 'testpassword' }
    });
    const cookies = response.headers()['set-cookie'];
    // Set cookies on the page context
    await page.context().addCookies(
      parseCookies(cookies, 'localhost')
    );
  });

  test('displays tenant list', async ({ page }) => {
    await page.goto('/tenants');
    await expect(page.getByRole('heading', { name: 'Tenants' })).toBeVisible();
    await expect(page.getByRole('table')).toBeVisible();
  });

  test('creates a new tenant', async ({ page }) => {
    await page.goto('/tenants/new');
    await page.getByLabel('Name').fill('Test Tenant');
    await page.getByLabel('Slug').fill('test-tenant');
    await page.getByRole('button', { name: 'Create Tenant' }).click();

    // Should redirect to tenant detail
    await expect(page).toHaveURL(/\/tenants\/[0-9a-f-]+$/);
    await expect(page.getByText('Test Tenant')).toBeVisible();
  });

  test('searches tenants', async ({ page }) => {
    await page.goto('/tenants');
    await page.getByPlaceholder('Search tenants').fill('acme');
    await expect(page.getByRole('table')).toContainText('Acme');
  });
});
```

### 12.4 Accessibility Testing

```typescript
// tests/e2e/accessibility.spec.ts
import { test, expect } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';

const pages = ['/dashboard', '/tenants', '/policies', '/routes', '/usage', '/audit'];

for (const path of pages) {
  test(`${path} has no accessibility violations`, async ({ page }) => {
    // Login first (reuse helper)
    await loginAsAdmin(page);
    await page.goto(path);
    await page.waitForLoadState('networkidle');

    const results = await new AxeBuilder({ page })
      .withTags(['wcag2a', 'wcag2aa', 'wcag22aa'])
      .analyze();

    expect(results.violations).toEqual([]);
  });
}
```

### 12.5 Test Configuration

```typescript
// vitest.config.ts
import { defineConfig } from 'vitest/config';
import { sveltekit } from '@sveltejs/kit/vite';

export default defineConfig({
  plugins: [sveltekit()],
  test: {
    include: ['tests/unit/**/*.test.ts'],
    environment: 'node',
    alias: {
      $lib: './src/lib'
    }
  }
});
```

```typescript
// vitest.config.browser.ts
import { defineConfig } from 'vitest/config';
import { sveltekit } from '@sveltejs/kit/vite';

export default defineConfig({
  plugins: [sveltekit()],
  test: {
    include: ['tests/component/**/*.test.ts'],
    browser: {
      enabled: true,
      provider: 'playwright',
      name: 'chromium'
    }
  }
});
```

**Evidence**: Vitest browser mode with Playwright is the current best practice for Svelte component testing, replacing jsdom-based approaches for more accurate rendering ([Svelte docs: Testing](https://svelte.dev/docs/svelte/testing)).

---

## 13. Build & Deploy

### 13.1 SvelteKit Adapter

Using `@sveltejs/adapter-node` for Docker deployment:

```javascript
// svelte.config.js
import adapter from '@sveltejs/adapter-node';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

/** @type {import('@sveltejs/kit').Config} */
const config = {
  preprocess: vitePreprocess(),
  kit: {
    adapter: adapter({
      out: 'build',
      precompress: true,    // gzip + brotli pre-compression
      envPrefix: 'PUBLIC_'
    }),
    alias: {
      $components: 'src/lib/components',
      $api: 'src/lib/api'
    }
  }
};

export default config;
```

### 13.2 Environment Variables

```bash
# .env.example

# Private (server-side only, never sent to browser)
ADMIN_API_BASE_URL=http://gateway:8080
JWT_SECRET=<used-for-session-signing-only>

# Public (available in browser via $env/static/public)
PUBLIC_APP_NAME=LLMSmartGate
PUBLIC_APP_VERSION=1.0.0
```

| Variable | Scope | Description |
|----------|-------|-------------|
| `ADMIN_API_BASE_URL` | Server | Rust admin API base URL |
| `JWT_SECRET` | Server | Secret for signing session cookies |
| `PUBLIC_APP_NAME` | Client | App display name |
| `PUBLIC_APP_VERSION` | Client | Displayed in footer/settings |
| `ORIGIN` | Server | SvelteKit origin for CSRF protection |
| `PORT` | Server | HTTP listen port (default 3000) |
| `HOST` | Server | Bind address (default 0.0.0.0) |

### 13.3 Dockerfile

```dockerfile
# Dockerfile
FROM node:22-alpine AS builder

WORKDIR /app

# Install dependencies
COPY package.json package-lock.json ./
RUN npm ci

# Copy source and build
COPY . .
RUN npm run build

# Prune dev dependencies
RUN npm prune --production

# ---- Production stage ----
FROM node:22-alpine AS runner

WORKDIR /app

# Security: run as non-root
RUN addgroup --system --gid 1001 nodejs && \
    adduser --system --uid 1001 sveltekit

# Copy built output and production deps
COPY --from=builder --chown=sveltekit:nodejs /app/build ./build
COPY --from=builder --chown=sveltekit:nodejs /app/node_modules ./node_modules
COPY --from=builder --chown=sveltekit:nodejs /app/package.json ./

USER sveltekit

# SvelteKit adapter-node defaults
ENV PORT=3000
ENV HOST=0.0.0.0
ENV NODE_ENV=production

EXPOSE 3000

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
  CMD wget --no-verbose --tries=1 --spider http://localhost:3000/login || exit 1

CMD ["node", "build/index.js"]
```

```
# .dockerignore
node_modules
.svelte-kit
build
.env
.env.*
!.env.example
tests
*.md
.git
```

### 13.4 Docker Compose (Development)

```yaml
# docker-compose.dev.yml
version: '3.8'
services:
  admin-console:
    build:
      context: ./admin-console
      dockerfile: Dockerfile
    ports:
      - "3000:3000"
    environment:
      - ADMIN_API_BASE_URL=http://gateway:8080
      - ORIGIN=http://localhost:3000
    depends_on:
      - gateway
    networks:
      - llmsmartgate

  gateway:
    # Rust backend container
    image: llmsmartgate/gateway:latest
    ports:
      - "8080:8080"
    networks:
      - llmsmartgate

networks:
  llmsmartgate:
    driver: bridge
```

### 13.5 Build Scripts

```json
{
  "scripts": {
    "dev": "vite dev",
    "build": "vite build",
    "preview": "vite preview",
    "check": "svelte-kit sync && svelte-check --tsconfig ./tsconfig.json",
    "check:watch": "svelte-kit sync && svelte-check --tsconfig ./tsconfig.json --watch",
    "test:unit": "vitest run --config vitest.config.ts",
    "test:component": "vitest run --config vitest.config.browser.ts",
    "test:e2e": "playwright test",
    "test:a11y": "playwright test tests/e2e/accessibility.spec.ts",
    "test": "npm run test:unit && npm run test:component && npm run test:e2e",
    "lint": "eslint . && prettier --check .",
    "format": "prettier --write .",
    "generate:api-types": "openapi-typescript ./openapi/admin-api.yaml -o ./src/lib/api/types.d.ts",
    "docker:build": "docker build -t llmsmartgate/admin-console:latest .",
    "docker:run": "docker run -p 3000:3000 --env-file .env llmsmartgate/admin-console:latest"
  }
}
```

**Evidence**: adapter-node produces a self-contained Express-based server ideal for Docker. Multi-stage builds keep images between 140-200MB ([Docker + SvelteKit guide, Feb 2026](https://oneuptime.com/blog/post/2026-02-08-how-to-containerize-a-sveltekit-application-with-docker/view)).

---

## 14. Security

### 14.1 XSS Prevention

- **Svelte auto-escaping**: All `{expression}` bindings in Svelte templates are HTML-escaped by default. The `{@html}` directive is never used in this project.
- **Content Security Policy**: Set via SvelteKit hooks:

```typescript
// In hooks.server.ts (within resolve options)
const response = await resolve(event, {
  transformPageChunk: ({ html }) => html
});

response.headers.set(
  'Content-Security-Policy',
  [
    "default-src 'self'",
    "script-src 'self'",
    "style-src 'self' 'unsafe-inline'",  // Required for Tailwind
    "img-src 'self' data:",
    "connect-src 'self'",
    "font-src 'self'",
    "frame-ancestors 'none'",
    "base-uri 'self'",
    "form-action 'self'"
  ].join('; ')
);

return response;
```

### 14.2 CSRF Protection

SvelteKit has built-in CSRF protection for form actions. It checks that the `Origin` header matches the configured `ORIGIN` environment variable. This is enabled by default and should not be disabled.

```javascript
// svelte.config.js
const config = {
  kit: {
    csrf: {
      checkOrigin: true  // default, do not disable
    }
  }
};
```

### 14.3 Cookie Security

All cookies use:
- `HttpOnly`: prevents JavaScript access (mitigates XSS token theft)
- `Secure`: only sent over HTTPS
- `SameSite: Lax`: prevents CSRF from third-party sites while allowing normal navigation
- Short-lived access tokens (15 minutes) with longer refresh tokens (7 days)

### 14.4 Security Headers

```typescript
// Additional security headers in hooks.server.ts
response.headers.set('X-Frame-Options', 'DENY');
response.headers.set('X-Content-Type-Options', 'nosniff');
response.headers.set('Referrer-Policy', 'strict-origin-when-cross-origin');
response.headers.set('Permissions-Policy', 'camera=(), microphone=(), geolocation=()');
response.headers.set('Strict-Transport-Security', 'max-age=63072000; includeSubDomains; preload');
```

### 14.5 Input Validation

- All user input is validated both client-side (immediate feedback) and server-side (authoritative) using shared Zod schemas
- Superforms validates in the form action before any data reaches the API
- The Rust backend performs its own validation as a defense-in-depth measure
- PEM key input is validated for format before submission, and cryptographically validated server-side

### 14.6 Secrets Management

- No secrets stored in client-side code or localStorage
- JWT tokens never exposed to browser JavaScript (HttpOnly cookies)
- Provider API keys never shown in the admin UI (only status: configured/missing)
- Environment variables prefixed `PUBLIC_` are the only ones bundled to the client

---

## 15. Observability & Tracing

### 15.1 Correlation ID Flow

Every request through the admin console carries a correlation ID:

```
Browser Request
  → SvelteKit hooks.server.ts (generate/propagate X-Correlation-Id)
    → openapi-fetch middleware (attach to outbound API call)
      → Rust Admin API (logs with correlation ID)
    ← Response includes X-Correlation-Id
  ← Response to browser includes X-Correlation-Id
```

On error, the correlation ID is:
1. Logged server-side by SvelteKit
2. Displayed to the user in `ErrorPanel` and toast notifications
3. Available in the Rust backend audit/error logs for cross-referencing

### 15.2 Client Error Handling

```typescript
// src/hooks.client.ts
import type { HandleClientError } from '@sveltejs/kit';

export const handleError: HandleClientError = async ({ error, event, status, message }) => {
  const correlationId = crypto.randomUUID();

  // Log to console in development, send to error reporting service in production
  console.error(`[Client Error] ${correlationId}:`, { error, status, message, url: event.url.href });

  return {
    message: status === 404 ? 'Page not found' : 'An unexpected error occurred',
    correlationId
  };
};
```

### 15.3 Server Error Handling

```typescript
// src/hooks.server.ts (add handleError export)
import type { HandleServerError } from '@sveltejs/kit';

export const handleError: HandleServerError = async ({ error, event, status, message }) => {
  const correlationId = event.locals.correlationId ?? 'unknown';

  console.error(`[Server Error] ${correlationId}:`, {
    error,
    status,
    message,
    url: event.url.href,
    method: event.request.method
  });

  return {
    message: 'An internal error occurred',
    correlationId
  };
};
```

### 15.4 Structured Logging

All server-side logs use structured JSON format for aggregation:

```typescript
// src/lib/utils/logger.ts
export function log(level: 'info' | 'warn' | 'error', message: string, meta?: Record<string, unknown>) {
  const entry = {
    timestamp: new Date().toISOString(),
    level,
    message,
    service: 'admin-console',
    ...meta
  };
  console[level](JSON.stringify(entry));
}
```

### 15.5 Performance Monitoring

```typescript
// Server timing headers for debugging
const startTime = performance.now();
const response = await resolve(event);
const duration = performance.now() - startTime;
response.headers.set('Server-Timing', `total;dur=${duration.toFixed(1)}`);
```

---

## Appendix A: Complete Component Inventory

| Component | Source | Custom? |
|-----------|--------|---------|
| Button, Input, Select, Textarea | shadcn-svelte | No |
| Card, Dialog, Sheet, Popover | shadcn-svelte | No |
| Table, Badge, Separator | shadcn-svelte | No |
| Tabs, Command, Tooltip | shadcn-svelte | No |
| Alert, AlertDialog | shadcn-svelte | No |
| Breadcrumb, Pagination | shadcn-svelte | No |
| Skeleton, Switch, ScrollArea | shadcn-svelte | No |
| Dropdown Menu | shadcn-svelte | No |
| Sonner (Toast) | shadcn-svelte | No |
| AppShell, Sidebar, Header | Custom | Yes |
| DataTable (TanStack wrapper) | Custom | Yes |
| TimeSeriesChart, BarChart, DonutChart | Custom (LayerChart) | Yes |
| MetricCard, SparkLine | Custom | Yes |
| StatusBadge, ConfirmDialog, EmptyState | Custom | Yes |
| PageHeader, SearchInput, CopyButton | Custom | Yes |
| FormField, FormSelect, FormSwitch | Custom (Formsnap) | Yes |
| ErrorPanel, LoadingSkeleton | Custom | Yes |
| ThemeToggle | Custom (mode-watcher) | Yes |
| JsonViewer | Custom | Yes |

---

## Appendix B: Form Action Patterns

### Standard CRUD Form Action

```typescript
// src/routes/(app)/tenants/new/+page.server.ts
import type { Actions, PageServerLoad } from './$types';
import { fail, redirect } from '@sveltejs/kit';
import { superValidate, message } from 'sveltekit-superforms';
import { zod } from 'sveltekit-superforms/adapters';
import { createTenantSchema } from '$lib/schemas/tenant';

// Define schema at module top level for memoization
const schema = zod(createTenantSchema);

export const load: PageServerLoad = async () => {
  const form = await superValidate(schema);
  return { form };
};

export const actions: Actions = {
  default: async ({ request, locals }) => {
    const form = await superValidate(request, schema);
    if (!form.valid) return fail(400, { form });

    const { data: tenant, error } = await locals.api.POST('/admin/tenants', {
      body: form.data
    });

    if (error) {
      if (error.code === 'CONFLICT') {
        return message(form, 'A tenant with this slug already exists', { status: 409 });
      }
      return message(form, error.message ?? 'Failed to create tenant', { status: 500 });
    }

    throw redirect(303, `/tenants/${tenant.id}`);
  }
};
```

### Corresponding Svelte Form Component

```svelte
<!-- src/routes/(app)/tenants/new/+page.svelte -->
<script lang="ts">
  import type { PageData } from './$types';
  import { superForm } from 'sveltekit-superforms';
  import { zodClient } from 'sveltekit-superforms/adapters';
  import { createTenantSchema } from '$lib/schemas/tenant';
  import * as Form from '$lib/components/ui/form';
  import { Input } from '$lib/components/ui/input';
  import { Button } from '$lib/components/ui/button';
  import PageHeader from '$lib/components/shared/PageHeader.svelte';
  import { toast } from 'svelte-sonner';

  let { data }: { data: PageData } = $props();

  const form = superForm(data.form, {
    validators: zodClient(createTenantSchema),
    onUpdated({ form }) {
      if (form.message) {
        toast.error(form.message);
      }
    }
  });

  const { form: formData, enhance, delayed } = form;

  // Auto-generate slug from name
  let slugManuallyEdited = $state(false);
  $effect(() => {
    if (!slugManuallyEdited && $formData.name) {
      $formData.slug = $formData.name
        .toLowerCase()
        .replace(/[^a-z0-9]+/g, '-')
        .replace(/^-|-$/g, '');
    }
  });
</script>

<PageHeader
  title="Create Tenant"
  description="Add a new tenant to the gateway"
/>

<form method="POST" use:enhance class="max-w-lg space-y-6">
  <Form.Field {form} name="name">
    <Form.Control>
      {#snippet children({ props })}
        <Form.Label>Name</Form.Label>
        <Input {...props} bind:value={$formData.name} placeholder="Acme Corp" />
      {/snippet}
    </Form.Control>
    <Form.Description>The display name for this tenant.</Form.Description>
    <Form.FieldErrors />
  </Form.Field>

  <Form.Field {form} name="slug">
    <Form.Control>
      {#snippet children({ props })}
        <Form.Label>Slug</Form.Label>
        <Input
          {...props}
          bind:value={$formData.slug}
          placeholder="acme-corp"
          oninput={() => { slugManuallyEdited = true; }}
        />
      {/snippet}
    </Form.Control>
    <Form.Description>URL-friendly identifier. Auto-generated from name.</Form.Description>
    <Form.FieldErrors />
  </Form.Field>

  <div class="flex gap-3">
    <Button type="submit" disabled={$delayed}>
      {#if $delayed}
        Creating...
      {:else}
        Create Tenant
      {/if}
    </Button>
    <Button variant="outline" href="/tenants">Cancel</Button>
  </div>
</form>
```

---

## Appendix C: Svelte 5 Error Boundary Pattern

```svelte
<!-- Using svelte:boundary for rendering error isolation -->
<svelte:boundary>
  <TimeSeriesChart data={chartData} />

  {#snippet failed(error, reset)}
    <div class="flex h-64 items-center justify-center rounded-md border border-destructive/20 bg-destructive/5">
      <div class="text-center">
        <p class="text-sm text-destructive">Failed to render chart</p>
        <Button variant="outline" size="sm" class="mt-2" onclick={reset}>
          Retry
        </Button>
      </div>
    </div>
  {/snippet}
</svelte:boundary>
```

**Evidence**: Svelte 5 introduces `<svelte:boundary>` as a native error boundary with `failed` snippet for recovery UI ([Svelte docs: svelte:boundary](https://svelte.dev/docs/svelte/svelte-boundary)).

---

## Appendix D: References

### Official Documentation
- [Svelte 5 Docs: Runes](https://svelte.dev/docs/svelte/$state)
- [Svelte 5 Docs: Best Practices](https://svelte.dev/docs/svelte/best-practices)
- [SvelteKit Docs: Routing](https://svelte.dev/docs/kit/routing)
- [SvelteKit Docs: Form Actions](https://svelte.dev/docs/kit/form-actions)
- [SvelteKit Docs: Auth](https://svelte.dev/docs/kit/auth)
- [SvelteKit Docs: Performance](https://svelte.dev/docs/kit/performance)
- [SvelteKit Docs: Accessibility](https://svelte.dev/docs/kit/accessibility)
- [SvelteKit Docs: Errors](https://svelte.dev/docs/kit/errors)
- [shadcn-svelte Docs: Components](https://www.shadcn-svelte.com/docs/components)
- [shadcn-svelte Docs: Dark Mode](https://www.shadcn-svelte.com/docs/dark-mode/svelte)
- [shadcn-svelte Docs: Data Table](https://www.shadcn-svelte.com/docs/components/data-table)
- [TailwindCSS v4 Blog](https://tailwindcss.com/blog/tailwindcss-v4)
- [TanStack Table: Svelte Adapter](https://tanstack.com/table/latest/docs/framework/svelte/svelte-table)
- [Superforms Docs](https://superforms.rocks/)
- [openapi-fetch Docs](https://openapi-ts.dev/openapi-fetch/)
- [LayerChart Docs](https://www.layerchart.com/)
- [Bits UI Docs](https://bits-ui.com/)
- [mode-watcher](https://github.com/svecosystem/mode-watcher)

### Community & Research
- [Svelte Best Practices in 2026](https://onehorizon.ai/blog/svelte-best-practices-in-2026-scaling-with-runes-snippets-and-pure-reactivity)
- [Best Chart Libraries for Svelte 2026](https://weavelinx.com/best-chart-libraries-for-svelte-projects-in-2026/)
- [Web Accessibility in 2026: Frontend Developer Guide](https://www.codewithseb.com/blog/web-accessibility-2026-eaa-ada-wcag-guide)
- [WCAG 2.2 AA Compliance Checklist](https://www.levelaccess.com/blog/wcag-2-2-aa-summary-and-checklist-for-website-owners/)
- [Tailwind CSS v4 Migration Guide](https://www.digitalapplied.com/blog/tailwind-css-v4-migration-new-features-guide)
- [SvelteKit JWT Auth with Middleware](https://dev.to/jais_mukesh/sveltekit-jwt-authentication-with-middleware-a-complete-implementation-51gm)
- [Dockerizing SvelteKit Applications](https://oneuptime.com/blog/post/2026-02-08-how-to-containerize-a-sveltekit-application-with-docker/view)
- [Svelte Testing with Vitest Browser Mode](https://scottspence.com/posts/testing-with-vitest-browser-svelte-guide)
- [SvelteKit SSE Real-Time Apps](https://sveltetalk.com/posts/building-real-time-sveltekit-apps-with-server-sent-events)

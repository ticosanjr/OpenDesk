# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

OpenDesk ships as a native desktop app (Tauri v2 + WebView) for Linux, with
Windows and macOS builds on the roadmap. The visual system is web-native and is
validated in Chromium; the packaged app uses WebKitGTK/WebView2.

## Users

Primary: technical operators and small AI teams who run OpenCode agents on real
projects and need to see what ran, why it ran, what it cost, and what it
produced. They work in long sessions, keep many workspaces open, and switch
between planning, execution, review, and governance.

Secondary: the same operator acting as a reviewer or approver — handling
permission requests, plan approvals, HITL questions, proposals, and failed runs
without leaving the app.

## Product Purpose

OpenDesk is the visual control plane for OpenCode: a desktop app that governs
agents, tasks, automations, artifacts, queues, costs, and provenance while
keeping OpenCode configuration in native files on disk. Success means an
operator can trust what the system is doing, intervene at the right moment, and
trace any delivered artifact back to the run and agent that produced it.

## Positioning

OpenDesk is not a chat wrapper and not a hosted agent platform. Its durable
claim is local-first governance: configuration and workspaces live on disk,
execution is delegated to the OpenCode V2 daemon, and every operational action
(tasks, runs, approvals, costs, artifacts) is observable and auditable in one
desktop surface.

## Operating Context

- Workspaces point at real project folders; `input/` and `output/` are part of
  the contract.
- The OpenCode daemon may be local, shared, or offline. The UI must stay useful
  and explicit when the engine is unavailable.
- Runs are long-lived and asynchronous. Users leave the app open, use the tray,
  and return to review results, failures, or pending decisions.
- Approvals arrive from multiple channels (UI, email, Signal) and must resolve
  to one canonical state.
- Dense operational data is normal: queues, timelines, tables, logs, diffs,
  model costs, and token counts.
- Configuration is SSOT on disk (`opencode.jsonc`, `.opencode/agents`,
  `.opencode/skills`, `.opencode/commands`, `mcp.servers`); SQLite stores
  operational state only.

## Capabilities and Constraints

- OpenCode V2 only: `/api/*`, Basic auth, `permissions`, `commands`,
  `mcp.servers`, `disabled`, `type: "remote"`.
- Kanban workflow: Fila, Em execução, Atenção, Concluído, Cancelado.
- Task execution with Plan → Build → Audit phases, budget guard, and
  deterministic output folders.
- Routines with cron/recurrence, missed-run policy, timezone, notifications, and
  dry-run.
- Recipes with HITL checkpoints, permissions, artifacts, and sinks.
- Studio for agents, skills, commands, MCP, and raw JSONC.
- Governance: org chart, goals, budgets, quotas, performance, delegation.
- Observability: latency, failures, queue health, model mix, cache, cost.
- Local-only secrets in `0600` files or keyring; never log passwords, tokens,
  full prompts, or `.env` content.
- Accessibility target: WCAG 2.1 AA for the desktop UI, with visible focus,
  readable body text, and reduced-motion alternatives.

## Brand Commitments

- Name: OpenDesk. Dark zinc surfaces with a workspace-driven blue accent.
- Tone: precise, operational, calm. Portuguese (pt-BR) interface copy; technical
  identifiers stay in their native form when they are real API values.
- Identity stays restrained: the workspace accent marks structure, not content.
- The product is a tool, not a marketing page. Familiarity and density beat
  ornament.

## Evidence on Hand

- Working Tauri app, React frontend, Rust backend, SQLite database, E2E suite.
- `docs/decisions/` contains the architecture record; `docs/progress.md` records
  shipped behavior.
- No public testimonials, customer logos, benchmarks, or pricing claims are
  available. Do not invent them.

## Product Principles

1. **Truth over decoration.** Status, cost, provenance, and failure must be
   visible and accurate before the interface is polished.
2. **Local-first, delegated execution.** OpenDesk governs; OpenCode executes.
3. **Intervene at the right moment.** Attention is a first-class surface:
   approvals, questions, permissions, and failures are actionable in place.
4. **Dense but legible.** Technical users deserve compact layouts with a
   readable floor, clear hierarchy, and no decorative noise.
5. **Deterministic delivery.** Every automated output lands in a predictable
   folder and remains traceable to its run.

## Accessibility & Inclusion

- WCAG 2.1 AA contrast and focus visibility.
- No functional text below 11px; body text at 13.5–14px.
- Keyboard reachable controls, escape/focus trap in dialogs, and labeled
  icon-only actions.
- Respect `prefers-reduced-motion` without removing essential progress
  feedback.

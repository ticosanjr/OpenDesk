# Design System

OpenDesk is a desktop control plane. The interface is **Operate**: it serves
people doing focused technical work. Ambition shows as precision, hierarchy,
and restraint — not as decoration.

This document records the incumbent system and the target vocabulary. New work
should use it instead of inventing local variants.

## Principles

1. **One job per zone.** Page title → primary action → content → secondary
   metadata. Never let filters, status chips, or help text compete with the
   task.
2. **Dense but legible.** Compact layouts are welcome; unreadable text is not.
   Body copy is 13.5–14px, metadata 12px, overline 11px.
3. **Status is a system.** One badge per object, one icon, one color, one label.
   No parallel status languages.
4. **The workspace accent marks structure.** It appears in focus rings, active
   navigation, dividers, and surface tint — never in body text.
5. **Motion explains state.** 150–220ms for feedback. No page choreography.
   Reduced motion keeps progress feedback.

## Layout

| Role | Value |
|---|---|
| Sidebar | 236px expanded, 64px collapsed |
| Settings / Integrations rail | 220px vertical, grouped |
| Topbar | 56px |
| Page gutter | 20px (default), 28px at ≥1024px |
| Maximum content width | 1440px, centered |
| Section gap | 20px (default), 24px at ≥1024px |
| Card gap | 12–16px |
| Card padding | 16px (compact), 20px (default) |
| Inline gap | 4–8px |
| Label → field | 6px |

`PageBody` owns the gutter and maximum width. `PageBodyFill` is for full-height
surfaces (Kanban, Chat, Studio) where children manage their own scrollers.

## Typography

Family: **Inter** for UI; **JetBrains Mono** for numbers, IDs, paths, and code.

| Role | Size / line | Weight | Use |
|---|---|---|---|
| Page | 24 / 32 | 600 | page title |
| Section | 20 / 28 | 600 | major section |
| Card title | 15 / 22 | 600 | card and panel headings |
| Body | 13.5 / 21 | 400 | default copy, inputs |
| Meta | 12 / 18 | 400/500 | timestamps, helpers, table headers |
| Overline | 11 / 16 | 500 | uppercase mono labels, max 3–4 per screen |
| KPI | 28 / 36 | 500 (mono) | highlighted numbers |

Rules:
- Do not use 700; Inter 600 is the strongest loaded weight.
- Uppercase is reserved for overlines, badges, and short status labels.
- Mono is for data, not prose.
- Legacy `text-[8px]`…`text-[13px]` classes are remapped by `src/styles.css`
  to the floor above. New code should use `.type-*` roles or Tailwind's
  `text-xs`/`text-sm`.

## Color

Semantic tokens live in `@theme` and CSS variables.

| Token | Value | Use |
|---|---|---|
| surface-0 | `#09090b` | app background |
| surface-1 | `#0f0f12` | shell panels |
| surface-2 | `#151518` | raised panels |
| line | `#27272a` | borders |
| line-soft | `#202024` | internal dividers |
| text primary | `#fafafa` / `zinc-100` | titles, values |
| text secondary | `zinc-400` | descriptions |
| text meta | `zinc-500` (`#85858f`) | timestamps, hints |
| accent | workspace accent, default `#3b82f6` | action, focus, active nav |
| success | emerald 400/300 | success and online |
| warning | amber 400/300 | attention, budget, pending |
| danger | red 400/300 | failures, destructive |
| info | blue 400/300 | running, neutral links |

- Borders sit at 60–80% opacity on the near-black surfaces.
- Avoid filling large areas with saturated color. Use a 4–10% tint.
- Text contrast target: 4.5:1 for body, 3:1 for large text/icons.

## Surfaces

- **Card**: 12px radius, `border-zinc-800/70`, `bg-zinc-900/30`, one subtle
  inset highlight. No shadow by default.
- **Inset row**: 8px radius, `border-zinc-800/70`, `bg-zinc-950/40`.
- **Dialog**: 16px radius, `bg-zinc-950/95`, one shadow, backdrop blur.
- **Popover**: 12px radius, `border-zinc-700`, solid `bg-zinc-950`.

Do not nest cards more than one level. Prefer dividers and insets inside a
card.

## Controls

| Size | Height | Text | Use |
|---|---|---|---|
| xs | 28px | 11.5px | dense toolbar / icon-only |
| sm | 32px | 12.5px | secondary, compact inputs |
| md | 36px | 14px | default |
| lg | 40px | 14px | primary action |

- Minimum interactive target: 28×28px, with 32×32 preferred.
- Form fields: 36px default, 32px compact, 14px text.
- Buttons use an icon + label. Icon-only buttons require `aria-label` and a
  tooltip.
- Focus: 2px workspace-accent ring with a 1–2px offset.
- Disabled: 50% opacity, no pointer events.
- Loading: spinner plus a stable label; never replace the label entirely.

## Components

Shared primitives live in `src/components/ui/`:

`Button`, `Badge`, `Card`, `Modal`, `Tooltip`, `EmptyState`,
`LoadingState`, `IconLabel`, `StatusDot`, `WorkflowStatusBadge`, `FilterBar`,
`FilterSearch`, `FilterSelect`, `SectionTabs`, `Switch`, `ConfirmDialog`.

### Status badges
One canonical badge per object. `Badge` is 11px mono uppercase, 20px tall.
`WorkflowStatusBadge` is the only task-workflow badge. Run statuses use the
same tones and human labels.

### Modals
- `size="sm"` (28rem): confirmations.
- `size="md"` (42rem): short forms.
- `size="lg"` (56rem): detail and history.
- `size="xl"` (64rem): complex editors and viewers.

No forced minimum width or height. Max height is `100dvh - 24px`; the dialog
scrolls internally. The header is sticky and bordered. Focus goes to the first
field by default; detail-heavy dialogs use `initialFocus="dialog"` so focusing
an embedded form does not scroll the header content away.

### HITL no card
A task with a pending interaction, permission, approval, failure or manual
block shows an amber panel inside the card, with:
- a reason-specific title;
- a one-line detail (question, permission/action, failure or block reason);
- one primary action.

“Responder” opens a focused modal with the full `InteractionCard` or
`PermissionRequestCard`. Resolving reloads the board and removes the pending
state. Approval is direct from the card. The card itself is not a `button`;
its title and the “Abrir tarefa” action are buttons, avoiding nested
interactive controls.

### Viewers
- The parent page owns the path, size, metadata and actions.
- Embedded viewers render only preview/source tabs, warnings and download.
- The artifact viewer uses the shared `Modal` with a sticky header, tabs,
  metadata and actions; Escape and focus trapping come from the modal.
- Empty, loading and error states inside a viewer must keep the same frame and
  offer a recovery action.

### Favoritos e recomendados
- Favorites are global (`opendesk.favorites.v1`) and apply to packs and
  recommended skills.
- The recommended list reuses the Studio preview/apply pipeline; only the
  selected skill is previewed and installed after confirmation.
- Projects with their own installer (Impeccable) show “Abrir instalador”
  instead of pretending to install an incomplete skill.

### Empty states
Icon, title, one sentence of guidance, and a next action when one exists. Keep
them under 160px tall inside cards. Do not use a 440px void.

### Errors
Say what failed, why (when known), and the recovery action. Prefer inline
errors near the affected control; use a toast only for cross-screen results.

## Copy

- pt-BR for interface labels, actions, and explanations.
- Technical identifiers stay as-is when they are real API values (`opencode`,
  `mcp.servers`, model refs).
- Translate internal states in the UI: `RUNNING` → “executando”,
  `SUCCESS` → “sucesso”, `FAILED` → “falhou”.
- Buttons are verbs: “Executar”, “Salvar”, “Aprovar”, “Reenfileirar”.
- Avoid “runs” in headings when “execuções” fits; keep “run” only where it is
  the domain term.
- Empty states suggest the next step; errors include recovery.

## Motion

- 150ms for hover/focus, 200ms for panels and dialogs.
- Easing: `cubic-bezier(0.22, 1, 0.36, 1)`.
- Animate opacity/transform, not layout.
- Continuous motion only for live execution (gear/spinner) and progress.
- `prefers-reduced-motion`: stop decorative loops, slow spinners instead of
  freezing them, keep state changes visible.

## Accessibility

- Semantic landmarks: `header`, `nav`, `main`, `aside`.
- Every icon-only control has an accessible name and tooltip.
- Dialogs trap focus, close on Escape, and restore focus.
- Tabs use `role="tablist"`, `role="tab"`, `aria-selected`.
- Switches use `role="switch"` with a verb-based label.
- Tables have headers; numeric columns are right-aligned and tabular.
- No horizontal page overflow at 1024×768. Kanban scrolls inside its board.
- Target WCAG 2.1 AA; axe must report no critical or serious violations.

## Verification

- `npm run check`, `npm run lint`, `npm run test:ui` for daily work.
- Visual review at 1024×768, 1440×900, and 1920×1080.
- `npm run test:features` and the full gate only on request/release.
- Impeccable detector: no warnings, or documented false positives.

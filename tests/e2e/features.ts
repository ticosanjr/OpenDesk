export interface FeatureEntry {
  id: string;
  area: string;
  spec: string;
  test: string;
}

/**
 * Functional coverage manifest. A feature is covered when at least one
 * Playwright test whose title contains `test` passes in `spec`.
 */
export const FEATURES: FeatureEntry[] = [
  { id: "foundation.workspace.blank", area: "Fundação", spec: "workspace-ui.spec.ts", test: "workspace dialog offers blank" },
  { id: "foundation.workspace.template", area: "Fundação", spec: "packs-ui.spec.ts", test: "workspace template creates" },
  { id: "foundation.workspace.existing", area: "Fundação", spec: "opendesk.spec.ts", test: "adds an existing workspace" },
  { id: "foundation.workspace.edit-delete", area: "Fundação", spec: "opendesk.spec.ts", test: "edits and removes a workspace" },
  { id: "foundation.theme", area: "Fundação", spec: "workspace-ui.spec.ts", test: "workspace color drives" },
  { id: "foundation.backup", area: "Fundação", spec: "backups-ui.spec.ts", test: "workspace backup can be created" },

  { id: "ops.dashboard", area: "Operação", spec: "dashboard-data.spec.ts", test: "dashboard shows KPIs" },
  { id: "ops.attention", area: "Operação", spec: "attention-full.spec.ts", test: "attention card explains HITL" },
  { id: "ops.search", area: "Operação", spec: "opendesk.spec.ts", test: "global search finds tasks" },
  { id: "ops.capture", area: "Operação", spec: "capture-blueprints.spec.ts", test: "quick capture" },
  { id: "ops.calendar", area: "Operação", spec: "calendar.spec.ts", test: "calendar creates" },

  { id: "work.task.create", area: "Trabalho", spec: "opendesk.spec.ts", test: "creates a Kanban task" },
  { id: "work.task.move", area: "Trabalho", spec: "opendesk.spec.ts", test: "kanban move control" },
  { id: "work.task.cancel", area: "Trabalho", spec: "opendesk.spec.ts", test: "task can be cancelled" },
  { id: "work.task.dedupe", area: "Trabalho", spec: "kanban-origins.spec.ts", test: "creating an identical task" },
  { id: "work.task.origins", area: "Trabalho", spec: "kanban-origins.spec.ts", test: "kanban separates human work" },
  { id: "work.task.fork", area: "Trabalho", spec: "attention-fork.spec.ts", test: "attention card, task fork" },
  { id: "work.queue.settings", area: "Trabalho", spec: "kanban-origins.spec.ts", test: "queue settings persist" },
  { id: "work.notifications", area: "Trabalho", spec: "opendesk.spec.ts", test: "notification preferences" },

  { id: "runs.list-detail", area: "Execuções", spec: "runs-detail-ui.spec.ts", test: "runs list opens detail" },
  { id: "runs.export", area: "Execuções", spec: "runs-detail-ui.spec.ts", test: "runs list opens detail" },
  { id: "chat.thread", area: "Chat", spec: "opendesk.spec.ts", test: "creates a chat thread" },

  { id: "automation.routine.create", area: "Automação", spec: "routines-ui.spec.ts", test: "routine accordion" },
  { id: "automation.routine.disabled", area: "Automação", spec: "defaults-off.spec.ts", test: "new routines and recipes start disabled" },
  { id: "automation.routine.recurrence", area: "Automação", spec: "opendesk.spec.ts", test: "creates and edits a recurrence" },
  { id: "automation.recipe.visual", area: "Automação", spec: "recipes-ui.spec.ts", test: "recipe visual flow" },
  { id: "automation.recipe.run", area: "Automação", spec: "recipes-ui.spec.ts", test: "recipe run modal" },
  { id: "automation.recipe.disabled", area: "Automação", spec: "defaults-off.spec.ts", test: "new routines and recipes start disabled" },
  { id: "automation.recipe.crud", area: "Automação", spec: "recipes.spec.ts", test: "recipes CRUD" },
  { id: "automation.io-bus", area: "Automação", spec: "io-bus.spec.ts", test: "io bus inbound" },
  { id: "automation.webhook", area: "Automação", spec: "io-bus.spec.ts", test: "signed webhook" },

  { id: "library.files", area: "Biblioteca", spec: "files-search-ui.spec.ts", test: "files page searches" },
  { id: "library.files.preview", area: "Biblioteca", spec: "files-search-ui.spec.ts", test: "files explorer lists" },
  { id: "library.artifacts", area: "Biblioteca", spec: "artifacts-ui.spec.ts", test: "artifact can be opened" },
  { id: "library.packs.item", area: "Biblioteca", spec: "packs-ui.spec.ts", test: "package tabs" },
  { id: "library.packs.repair", area: "Biblioteca", spec: "packs-ui.spec.ts", test: "starter pack repair" },
  { id: "library.packs.install", area: "Biblioteca", spec: "packs.spec.ts", test: "installs and removes a starter pack" },
  { id: "library.packs.conflict", area: "Biblioteca", spec: "packs-ui.spec.ts", test: "starter pack conflict" },
  { id: "library.catalog", area: "Biblioteca", spec: "packs-ui.spec.ts", test: "external capability catalog" },
  { id: "library.factory", area: "Biblioteca", spec: "factory.spec.ts", test: "default factory workspace" },
  { id: "library.packs.control", area: "Biblioteca", spec: "control-mcp.spec.ts", test: "new workspace installs the control pack" },

  { id: "studio.agent-skill", area: "Studio", spec: "studio-edit.spec.ts", test: "studio keeps agent and skill drafts" },
  { id: "studio.import", area: "Studio", spec: "opendesk.spec.ts", test: "studio imports local frontmatter" },
  { id: "studio.copy-toggle", area: "Studio", spec: "opendesk.spec.ts", test: "studio copies, toggles" },
  { id: "studio.backup", area: "Studio", spec: "opendesk.spec.ts", test: "studio backup lists" },
  { id: "studio.command", area: "Studio", spec: "studio-command-ui.spec.ts", test: "studio creates a command" },
  { id: "studio.mcp.control", area: "Studio", spec: "control-mcp.spec.ts", test: "new workspace installs the control pack" },

  { id: "governance.profile", area: "Governança", spec: "governance-ui.spec.ts", test: "governance edits profiles" },
  { id: "governance.delegation", area: "Governança", spec: "governance-ui.spec.ts", test: "governance edits profiles" },
  { id: "governance.heartbeat", area: "Governança", spec: "governance-ui.spec.ts", test: "governance heartbeat" },

  { id: "learning.scan", area: "Aprendizado", spec: "learning-loop.spec.ts", test: "learning scans a failure" },
  { id: "learning.promote", area: "Aprendizado", spec: "learning-loop.spec.ts", test: "learning scans a failure" },
  { id: "learning.remember", area: "Aprendizado", spec: "learning-loop.spec.ts", test: "learning scans a failure" },

  { id: "observability.metrics", area: "Observabilidade", spec: "observability-data.spec.ts", test: "observability shows latency" },
  { id: "observability.empty", area: "Observabilidade", spec: "observability-ui.spec.ts", test: "observability page exposes" },

  { id: "security.audit", area: "Segurança", spec: "audit-ui.spec.ts", test: "sensitive API calls" },
  { id: "security.proposals", area: "Segurança", spec: "proposals.spec.ts", test: "agent configuration proposals" },
  { id: "security.webhook", area: "Segurança", spec: "io-bus.spec.ts", test: "signed webhook" },

  { id: "integrations.smtp", area: "Integrações", spec: "integrations-ui.spec.ts", test: "SMTP setup tests" },
  { id: "integrations.browse", area: "Integrações", spec: "browse-ui.spec.ts", test: "browse integration activates" },

  { id: "settings.views", area: "Configurações", spec: "settings-views.spec.ts", test: "settings expose general" },
  { id: "models.ref", area: "Modelos", spec: "model-ref.spec.ts", test: "model references" },
  { id: "ui.modals", area: "UI", spec: "ui-audit.spec.ts", test: "key modals" },
  { id: "ui.modal.focus", area: "UI", spec: "modal-focus.spec.ts", test: "typing in a modal keeps focus" },
  { id: "ui.routes", area: "UI", spec: "ui-audit.spec.ts", test: "all main routes" },
  { id: "ui.scroll", area: "UI", spec: "scroll.spec.ts", test: "every route scrolls" },
  { id: "ui.geometry", area: "UI", spec: "cards.spec.ts", test: "task, routine and attention cards" },
  { id: "ui.sync", area: "UI", spec: "opendesk.spec.ts", test: "workspace, settings, Studio and external edit sync" },
  { id: "attention.dismiss", area: "Atenção", spec: "attention-ui.spec.ts", test: "attention dismisses" },
];

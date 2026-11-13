import { Suspense, useEffect, useMemo, useRef, useState, type CSSProperties } from "react";
import { Outlet, useLocation, useNavigate } from "react-router-dom";
import { AlertTriangle, RefreshCw, Settings, X } from "lucide-react";
import { Sidebar } from "@/components/layout/Sidebar";
import { Topbar } from "@/components/layout/Topbar";
import { CommandPalette } from "@/components/layout/CommandPalette";
import { WorkspaceDialog } from "@/components/layout/WorkspaceDialog";
import { Button } from "@/components/ui/button";
import { ConfirmDialog } from "@/components/ui/confirm-dialog";
import { ErrorBoundary } from "@/components/ui/error-boundary";
import { Modal } from "@/components/ui/modal";
import { Toaster } from "@/components/ui/toaster";
import { useAppStore } from "@/store/app-store";
import { useUiStore } from "@/store/ui-store";
import { toast, useToastStore } from "@/store/toast-store";
import { formatUsd } from "@/lib/format";
import { links } from "@/lib/links";
import { isEditableTarget } from "@/lib/keyboard";
import { apiGet } from "@/lib/api";
import type { Schedule } from "@/lib/types";
import {
  artifactNotificationsEnabled,
  isTerminalTaskStatus,
  markNotificationOnce,
  notificationEnabled,
  sendNativeNotification,
} from "@/lib/notification-policy";

const ROUTE_LABELS: Array<[string, string]> = [
  ["/search", "Buscar"],
  ["/kanban", "Kanban"],
  ["/calendar", "Calendário"],
  ["/runs/", "Execução"],
  ["/runs", "Execuções"],
  ["/chat", "Chat"],
  ["/artifacts", "Artefatos"],
  ["/files", "Arquivos"],
  ["/routines", "Rotinas"],
  ["/recipes", "Receitas"],
  ["/packs", "Pacotes"],
  ["/studio", "Studio"],
  ["/governance", "Governança"],
  ["/observability", "Observabilidade"],
  ["/learning", "Aprendizado"],
  ["/router", "Roteador"],
  ["/integrations", "Integrações"],
  ["/settings", "Configurações"],
];

function routeLabel(pathname: string): string {
  if (pathname === "/") return "Dashboard";
  return ROUTE_LABELS.find(([prefix]) => pathname.startsWith(prefix))?.[1] ?? "OpenDesk";
}

export function AppShell() {
  const mainRef = useRef<HTMLElement | null>(null);
  const autoCollapsedRef = useRef(false);
  const boot = useAppStore((state) => state.boot);
  const loading = useAppStore((state) => state.loading);
  const error = useAppStore((state) => state.error);
  const workspaces = useAppStore((state) => state.workspaces);
  const activeWorkspaceId = useAppStore((state) => state.activeWorkspaceId);
  const daemon = useAppStore((state) => state.daemon);
  const restartDaemon = useAppStore((state) => state.restartDaemon);
  const refresh = useAppStore((state) => state.refresh);
  const setCommandOpen = useUiStore((state) => state.setCommandOpen);
  const setSidebarCollapsed = useUiStore((state) => state.setSidebarCollapsed);
  const setTaskDialogOpen = useUiStore((state) => state.setTaskDialogOpen);
  const navigate = useNavigate();
  const budgetAlert = useAppStore((state) => state.budgetAlert);
  const lastRunEvent = useAppStore((state) => state.lastRunEvent);
  const lastEngineUpdate = useAppStore((state) => state.lastEngineUpdate);
  const lastArtifact = useAppStore((state) => state.lastArtifact);
  const lastTaskEvent = useAppStore((state) => state.lastTaskEvent);
  const lastGoogleEvent = useAppStore((state) => state.lastGoogleEvent);
  const lastScheduleEvent = useAppStore((state) => state.lastScheduleEvent);
  const location = useLocation();
  const [dismissedBudget, setDismissedBudget] = useState<string | null>(null);
  const [shortcutsOpen, setShortcutsOpen] = useState(false);

  useEffect(() => {
    setDismissedBudget(null);
  }, [budgetAlert?.message]);

  // At the app's compact width the fixed sidebar eats too much of the view.
  // Collapse automatically when entering that width; the manual toggle keeps
  // working and the persisted preference is not overwritten.
  useEffect(() => {
    const query = window.matchMedia("(max-width: 1199px)");
    const apply = () => {
      if (query.matches) {
        if (!useUiStore.getState().sidebarCollapsed) {
          autoCollapsedRef.current = true;
          setSidebarCollapsed(true);
        }
      } else if (autoCollapsedRef.current) {
        autoCollapsedRef.current = false;
        setSidebarCollapsed(false);
      }
    };
    apply();
    query.addEventListener("change", apply);
    return () => query.removeEventListener("change", apply);
  }, [setSidebarCollapsed]);

  useEffect(() => {
    mainRef.current?.scrollTo({ top: 0 });
  }, [location.pathname]);

  useEffect(() => {
    if (!lastRunEvent) return;
    // Tasks go through Plan/Build/Audit runs. Only the terminal task status
    // notifies; runs without a task (schedules) notify once per run.
    if (lastRunEvent.task_id) return;
    const status = lastRunEvent.status;
    const enabled =
      status === "SUCCESS"
        ? notificationEnabled("run_finished")
        : notificationEnabled("run_failed");
    if (!enabled) return;
    const key = `run:${lastRunEvent.run_id}:${status}`;
    if (!markNotificationOnce(key)) return;
    const title =
      status === "SUCCESS"
        ? "Execução concluída"
        : status === "FAILED"
          ? "Execução falhou"
          : "Execução cancelada";
    const description =
      status === "FAILED"
        ? "Abra a execução para ver o diagnóstico."
        : status === "CANCELLED"
          ? "O run foi interrompido."
          : "O run terminou com sucesso.";
    toast({
      tone: status === "SUCCESS" ? "success" : status === "FAILED" ? "error" : "warning",
      title,
      description,
      href: links.run(lastRunEvent.run_id),
      key,
    });
    void sendNativeNotification(title, description);
  }, [lastRunEvent]);

  useEffect(() => {
    const event = lastTaskEvent;
    if (!event || !isTerminalTaskStatus(event.stage)) return;
    const enabled =
      event.stage === "DONE"
        ? notificationEnabled("run_finished")
        : notificationEnabled("run_failed");
    if (!enabled) return;
    const key = `task:${event.task_id}:${event.stage}`;
    if (!markNotificationOnce(key)) return;
    const title =
      event.stage === "DONE"
        ? "Tarefa concluída"
        : event.stage === "BLOCKED"
          ? "Tarefa bloqueada"
          : "Tarefa cancelada";
    const description =
      event.phase_status === "failed"
        ? "A execução falhou. Abra a tarefa para ver os detalhes."
        : event.stage === "DONE"
          ? "A tarefa foi concluída."
          : "A tarefa foi encerrada.";
    toast({
      tone: event.stage === "DONE" ? "success" : event.stage === "BLOCKED" ? "error" : "warning",
      title,
      description,
      href: links.task(event.task_id),
      key,
    });
    void sendNativeNotification(title, description);
  }, [lastTaskEvent]);

  useEffect(() => {
    if (!lastEngineUpdate) return;
    if (!["done", "failed"].includes(lastEngineUpdate.phase)) return;
    const key = `engine:${lastEngineUpdate.phase}:${lastEngineUpdate.message ?? ""}`;
    if (!markNotificationOnce(key)) return;
    const title =
      lastEngineUpdate.ok === false || lastEngineUpdate.phase === "failed"
        ? "Atualização falhou"
        : "Atualização concluída";
    toast({
      tone: title.includes("falhou") ? "error" : "success",
      title,
      description: lastEngineUpdate.message ?? undefined,
      href: links.settings("engine"),
      key,
    });
    void sendNativeNotification(title, lastEngineUpdate.message ?? "");
  }, [lastEngineUpdate]);

  useEffect(() => {
    if (!lastArtifact || location.pathname === "/artifacts") return;
    if (!artifactNotificationsEnabled()) return;
    const key = `artifacts:${lastArtifact.run_id || lastArtifact.id}`;
    if (!markNotificationOnce(key)) return;
    toast({
      tone: "info",
      title: "Artefato criado",
      description: lastArtifact.file_path,
      href: links.artifact(lastArtifact.id),
      key,
      duration: 3200,
    });
  }, [lastArtifact, location.pathname]);

  useEffect(() => {
    if (!lastGoogleEvent) return;
    const key = `google:${lastGoogleEvent.workspace_id}:${lastGoogleEvent.connected}`;
    if (!markNotificationOnce(key)) return;
    const title = lastGoogleEvent.connected
      ? "Google conectado"
      : "Google desconectado";
    toast({
      tone: lastGoogleEvent.connected ? "success" : "warning",
      title,
      description: lastGoogleEvent.email ?? undefined,
      href: links.integration("google"),
      key,
    });
    void sendNativeNotification(title, lastGoogleEvent.email ?? "");
  }, [lastGoogleEvent]);

  useEffect(() => {
    if (!lastScheduleEvent) return;
    if (!notificationEnabled("schedule_skips")) return;
    let cancelled = false;
    void apiGet<Schedule[]>(
      `/api/schedules?workspace_id=${encodeURIComponent(lastScheduleEvent.workspace_id)}`,
    )
      .then((list) => {
        if (cancelled) return;
        const schedule = list.find(
          (item) => item.id === lastScheduleEvent.schedule_id,
        );
        const status = schedule?.last_run_status ?? "";
        const stamp = schedule?.last_run_at ?? "";
        if (
          !stamp ||
          !["FAILED", "REJECTED", "TIMED_OUT", "SKIPPED_OFFLINE", "INVALID_CRON"].includes(
            status,
          )
        ) {
          return;
        }
        const key = `schedule:${lastScheduleEvent.schedule_id}:${stamp}:${status}`;
        if (!markNotificationOnce(key)) return;
        const title =
          status === "SKIPPED_OFFLINE"
            ? "Rotina pulada: engine offline"
            : status === "REJECTED"
              ? "Rotina rejeitada no checkpoint"
              : status === "TIMED_OUT"
                ? "Rotina expirou no checkpoint"
                : "Rotina falhou";
        toast({
          tone: status === "SKIPPED_OFFLINE" ? "warning" : "error",
          title,
          description: schedule?.name,
          href: links.schedule(lastScheduleEvent.schedule_id),
          key,
        });
        void sendNativeNotification(title, schedule?.name ?? "");
      })
      .catch(() => undefined);
    return () => {
      cancelled = true;
    };
  }, [lastScheduleEvent]);

  useEffect(() => {
    if (!budgetAlert) return;
    const key = `budget:${budgetAlert.level}:${budgetAlert.message}`;
    if (!markNotificationOnce(key)) return;
    const title =
      budgetAlert.level === "hard"
        ? "Orçamento bloqueado"
        : "Orçamento próximo do limite";
    toast({
      tone: budgetAlert.level === "hard" ? "error" : "warning",
      title,
      description: budgetAlert.message,
      href: "/router",
      key,
      duration: budgetAlert.level === "hard" ? 0 : 6000,
    });
    void sendNativeNotification(title, budgetAlert.message);
  }, [budgetAlert]);

  useEffect(() => {
    const clear = () => useToastStore.getState().clear();
    const onVisibility = () => {
      if (document.hidden) clear();
    };
    document.addEventListener("visibilitychange", onVisibility);
    window.addEventListener("beforeunload", clear);
    return () => {
      document.removeEventListener("visibilitychange", onVisibility);
      window.removeEventListener("beforeunload", clear);
    };
  }, []);

  const engineOffline =
    daemon !== null &&
    !daemon.online &&
    !["starting", "restarting"].includes(daemon.state);

  useEffect(() => {
    void boot();
  }, [boot]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.defaultPrevented) return;
      const editable = isEditableTarget(event.target);
      const modifier = event.metaKey || event.ctrlKey;
      if (modifier && event.key.toLowerCase() === "k") {
        event.preventDefault();
        setCommandOpen(!useUiStore.getState().commandOpen);
        return;
      }
      // Never steal printable keys (?, /, letters, space) while the user is
      // typing in a field, textarea, select or contenteditable.
      if (editable) return;
      if (modifier && event.key.toLowerCase() === "n") {
        event.preventDefault();
        navigate("/kanban");
        setTaskDialogOpen(true);
        return;
      }
      if (
        modifier &&
        event.shiftKey &&
        event.code === "Space" &&
        !("__TAURI_INTERNALS__" in window)
      ) {
        event.preventDefault();
        navigate("/capture");
        return;
      }
      if (
        modifier &&
        event.shiftKey &&
        event.key.toLowerCase() === "r"
      ) {
        event.preventDefault();
        void refresh();
        return;
      }
      if (event.key === "?" && !modifier && !event.altKey) {
        event.preventDefault();
        setShortcutsOpen((current) => !current);
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [navigate, refresh, setCommandOpen, setTaskDialogOpen]);

  const activeWorkspace = workspaces.find(
    (workspace) => workspace.id === activeWorkspaceId,
  );
  const accent = safeHexColor(activeWorkspace?.color_hex);
  const workspaceStyle = useMemo(
    () =>
      ({
        "--workspace-accent": accent,
        "--workspace-border": `color-mix(in srgb, ${accent} 26%, #27272a)`,
        "--workspace-border-soft": `color-mix(in srgb, ${accent} 14%, #27272a)`,
        "--workspace-glow": `color-mix(in srgb, ${accent} 10%, transparent)`,
        "--workspace-surface": `color-mix(in srgb, ${accent} 7%, #18181b)`,
      }) as CSSProperties,
    [accent],
  );

  useEffect(() => {
    const label = routeLabel(location.pathname);
    document.title = activeWorkspace?.name
      ? `${label} · ${activeWorkspace.name} · OpenDesk`
      : `${label} · OpenDesk`;
  }, [activeWorkspace?.name, location.pathname]);

  return (
    <div
      className="workspace-scope flex h-dvh w-screen overflow-hidden bg-[#09090b] text-zinc-100"
      style={workspaceStyle}
    >
      <Sidebar />
      <div className="flex min-w-0 flex-1 flex-col">
        <Topbar />
        {loading ? (
          <div className="h-0.5 w-full overflow-hidden bg-zinc-900">
            <div className="h-full w-1/3 animate-[loading_1.2s_ease-in-out_infinite] bg-blue-500/70" />
          </div>
        ) : null}
        {error ? (
          <div className="flex items-center gap-3 border-b border-red-500/20 bg-red-500/10 px-4 py-2 text-[13px] text-red-300">
            <AlertTriangle size={14} />
            <span className="flex-1">{error}</span>
            <Button
              variant="destructive"
              size="sm"
              onClick={() => void boot()}
              className="h-7"
            >
              <RefreshCw size={12} />
              Tentar novamente
            </Button>
          </div>
        ) : null}
        {engineOffline ? (
          <div className="flex flex-wrap items-center gap-3 border-b border-amber-500/20 bg-amber-500/10 px-4 py-2 text-[13px] text-amber-200">
            <AlertTriangle size={14} className="shrink-0" />
            <span className="min-w-0 flex-1">
              <strong className="font-semibold">OpenCode offline.</strong>{" "}
              {daemon?.message ??
                "Nenhum binário foi detectado e nenhum serviço compartilhado respondeu."}{" "}
              Nada será enfileirado até o motor voltar.
            </span>
            <Button
              variant="outline"
              size="sm"
              className="h-7"
              onClick={() => navigate("/settings?section=engine")}
            >
              <Settings size={12} />
              Configurar binário
            </Button>
            <Button
              variant="outline"
              size="sm"
              className="h-7"
              onClick={() => {
                void restartDaemon().finally(() => void refresh());
              }}
            >
              <RefreshCw size={12} />
              Tentar novamente
            </Button>
          </div>
        ) : null}
        {budgetAlert && dismissedBudget !== budgetAlert.message ? (
          <div
            className={
              budgetAlert.level === "hard"
                ? "flex flex-wrap items-center gap-3 border-b border-red-500/25 bg-red-500/10 px-4 py-2 text-[13px] text-red-200"
                : "flex flex-wrap items-center gap-3 border-b border-amber-500/25 bg-amber-500/10 px-4 py-2 text-[13px] text-amber-200"
            }
          >
            <AlertTriangle size={14} className="shrink-0" />
            <span className="min-w-0 flex-1">
              <strong className="font-semibold">
                {budgetAlert.level === "hard"
                  ? "Orçamento bloqueado."
                  : "Orçamento próximo do limite."}{" "}
              </strong>
              {budgetAlert.message} ({formatUsd(budgetAlert.daily_spent)} de{" "}
              {formatUsd(budgetAlert.daily_limit)})
            </span>
            <Button
              variant="outline"
              size="sm"
              className="h-7"
              onClick={() => navigate("/router")}
            >
              Ajustar limite
            </Button>
            <button
              type="button"
              aria-label="Fechar aviso de orçamento"
              className="rounded p-1 opacity-60 hover:opacity-100"
              onClick={() => setDismissedBudget(budgetAlert.message)}
            >
              <X size={13} />
            </button>
          </div>
        ) : null}
        <main ref={mainRef} className="app-main min-h-0 flex-1">
          <Suspense
            fallback={
              <div className="space-y-3 p-5 lg:p-7">
                <div className="h-6 w-48 animate-pulse rounded bg-zinc-800/70" />
                <div className="h-24 animate-pulse rounded-xl border border-zinc-800/60 bg-zinc-900/30" />
                <div className="grid gap-3 md:grid-cols-2">
                  <div className="h-40 animate-pulse rounded-xl border border-zinc-800/60 bg-zinc-900/30" />
                  <div className="h-40 animate-pulse rounded-xl border border-zinc-800/60 bg-zinc-900/30" />
                </div>
              </div>
            }
          >
            <ErrorBoundary resetKey={location.pathname}>
              <Outlet />
            </ErrorBoundary>
          </Suspense>
        </main>
      </div>
      <CommandPalette />
      <WorkspaceDialog />
      <ConfirmDialog />
      <Modal
        open={shortcutsOpen}
        onClose={() => setShortcutsOpen(false)}
        title="Atalhos do OpenDesk"
        description="Produtividade sem sair do teclado."
        size="md"
      >
        <div className="space-y-1.5">
          {[
            ["Ctrl/Cmd + K", "Abrir busca global / command palette"],
            ["Ctrl/Cmd + N", "Criar nova tarefa"],
            ["Ctrl/Cmd + Shift + Space", "Quick Capture (texto, link ou print)"],
            ["Ctrl/Cmd + Shift + R", "Atualizar dados do workspace"],
            ["?", "Mostrar ou esconder esta ajuda"],
            ["Esc", "Fechar modal, drawer ou paleta"],
            ["Enter", "Abrir tarefa selecionada no Kanban"],
          ].map(([keys, label]) => (
            <div
              key={keys}
              className="flex items-center justify-between gap-3 rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2"
            >
              <span className="text-[13.5px] text-zinc-300">{label}</span>
              <kbd className="rounded border border-zinc-700 bg-zinc-900 px-1.5 py-0.5 font-mono text-[12px] text-zinc-400">
                {keys}
              </kbd>
            </div>
          ))}
        </div>
      </Modal>
      <Toaster />
    </div>
  );
}

const FALLBACK_ACCENT = "#3b82f6";

/** Accept only #rgb/#rrggbb so a bad workspace value cannot break CSS vars. */
export function safeHexColor(value?: string | null): string {
  if (typeof value !== "string") return FALLBACK_ACCENT;
  const trimmed = value.trim();
  if (/^#(?:[0-9a-f]{3}|[0-9a-f]{6})$/i.test(trimmed)) return trimmed;
  return FALLBACK_ACCENT;
}

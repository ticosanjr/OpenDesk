import { Suspense, useEffect, useState } from "react";
import { Outlet, useLocation, useNavigate } from "react-router-dom";
import { AlertTriangle, RefreshCw, Settings, X } from "lucide-react";
import { Sidebar } from "@/components/layout/Sidebar";
import { Topbar } from "@/components/layout/Topbar";
import { CommandPalette } from "@/components/layout/CommandPalette";
import { WorkspaceDialog } from "@/components/layout/WorkspaceDialog";
import { Button } from "@/components/ui/button";
import { ConfirmDialog } from "@/components/ui/confirm-dialog";
import { Toaster } from "@/components/ui/toaster";
import { useAppStore } from "@/store/app-store";
import { useUiStore } from "@/store/ui-store";
import { toast } from "@/store/toast-store";
import { formatUsd } from "@/lib/format";
import { links } from "@/lib/links";
import { apiGet } from "@/lib/api";
import type { Schedule } from "@/lib/types";
import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from "@tauri-apps/plugin-notification";

export function AppShell() {
  const boot = useAppStore((state) => state.boot);
  const loading = useAppStore((state) => state.loading);
  const error = useAppStore((state) => state.error);
  const daemon = useAppStore((state) => state.daemon);
  const restartDaemon = useAppStore((state) => state.restartDaemon);
  const refresh = useAppStore((state) => state.refresh);
  const setCommandOpen = useUiStore((state) => state.setCommandOpen);
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

  useEffect(() => {
    setDismissedBudget(null);
  }, [budgetAlert?.message]);

  useEffect(() => {
    if (!lastRunEvent) return;
    const status = lastRunEvent.status;
    void (async () => {
      try {
        let granted = await isPermissionGranted();
        if (!granted) {
          granted = (await requestPermission()) === "granted";
        }
        if (!granted) return;
        if (status === "SUCCESS") {
          sendNotification({
            title: "OpenDesk · execução concluída",
            body: lastRunEvent.task_id
              ? "A tarefa terminou com sucesso."
              : "O run terminou com sucesso.",
          });
        } else if (status === "FAILED") {
          sendNotification({
            title: "OpenDesk · execução falhou",
            body: "Abra os logs para ver o diagnóstico.",
          });
        }
      } catch {
        // Native notifications are best-effort.
      }
    })();
    if (status === "SUCCESS") {
      toast({
        tone: "success",
        title: "Execução concluída",
        description: lastRunEvent.task_id
          ? "A tarefa terminou com sucesso."
          : "O run terminou com sucesso.",
        href: links.run(lastRunEvent.run_id),
        key: `run:${lastRunEvent.run_id}:${status}`,
      });
    } else if (status === "FAILED") {
      toast({
        tone: "error",
        title: "Execução falhou",
        description: "Abra os logs para ver o diagnóstico e tentar de novo.",
        href: links.run(lastRunEvent.run_id),
        key: `run:${lastRunEvent.run_id}:${status}`,
      });
    } else if (status === "CANCELLED") {
      toast({
        tone: "warning",
        title: "Execução cancelada",
        description: "O run foi interrompido.",
        href: links.run(lastRunEvent.run_id),
        key: `run:${lastRunEvent.run_id}:${status}`,
      });
    }
  }, [lastRunEvent, navigate]);

  useEffect(() => {
    if (!lastEngineUpdate) return;
    if (lastEngineUpdate.phase === "done") {
      toast({
        tone: lastEngineUpdate.ok === false ? "error" : "success",
        title: lastEngineUpdate.ok === false ? "Atualização falhou" : "Atualização concluída",
        description: lastEngineUpdate.message ?? undefined,
        href: links.settings("engine"),
        key: `engine-update:${lastEngineUpdate.phase}:${lastEngineUpdate.message ?? ""}`,
      });
    } else if (lastEngineUpdate.phase === "failed") {
      toast({
        tone: "error",
        title: "Atualização falhou",
        description: lastEngineUpdate.message ?? undefined,
        href: links.settings("engine"),
        key: `engine-update:${lastEngineUpdate.phase}:${lastEngineUpdate.message ?? ""}`,
      });
    }
  }, [lastEngineUpdate]);

  useEffect(() => {
    if (!lastArtifact || location.pathname === "/artifacts") return;
    toast({
      tone: "info",
      title: "Artefato criado",
      description: lastArtifact.file_path,
      href: links.artifact(lastArtifact.id),
      key: `artifact:${lastArtifact.id}`,
      duration: 3200,
    });
  }, [lastArtifact, location.pathname]);

  useEffect(() => {
    if (!lastTaskEvent?.phase_status) return;
    const status = lastTaskEvent.phase_status;
    if (!status.includes("fail")) return;
    toast({
      tone: "error",
      title: "Tarefa precisa de atenção",
      description: status,
      href: links.task(lastTaskEvent.task_id),
      key: `task:${lastTaskEvent.task_id}:${status}`,
    });
  }, [lastTaskEvent]);

  useEffect(() => {
    if (!lastGoogleEvent) return;
    toast({
      tone: lastGoogleEvent.connected ? "success" : "warning",
      title: lastGoogleEvent.connected
        ? "Google conectado"
        : "Google desconectado",
      description: lastGoogleEvent.email ?? undefined,
      href: links.integration("google"),
      key: `google:${lastGoogleEvent.workspace_id}:${lastGoogleEvent.connected}`,
    });
  }, [lastGoogleEvent]);

  useEffect(() => {
    if (!lastScheduleEvent) return;
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
        if (!["FAILED", "SKIPPED_OFFLINE", "INVALID_CRON"].includes(status)) return;
        toast({
          tone: status === "SKIPPED_OFFLINE" ? "warning" : "error",
          title:
            status === "SKIPPED_OFFLINE"
              ? "Rotina pulada: engine offline"
              : "Rotina falhou",
          description: schedule?.name,
          href: links.schedule(lastScheduleEvent.schedule_id),
          key: `schedule:${lastScheduleEvent.schedule_id}:${status}`,
        });
      })
      .catch(() => undefined);
    return () => {
      cancelled = true;
    };
  }, [lastScheduleEvent]);

  useEffect(() => {
    if (!budgetAlert) return;
    toast({
      tone: budgetAlert.level === "hard" ? "error" : "warning",
      title:
        budgetAlert.level === "hard"
          ? "Orçamento bloqueado"
          : "Orçamento próximo do limite",
      description: budgetAlert.message,
      href: "/router",
      key: `budget:${budgetAlert.level}:${budgetAlert.message}`,
      duration: budgetAlert.level === "hard" ? 0 : 6000,
    });
  }, [budgetAlert]);

  const engineOffline =
    daemon !== null &&
    !daemon.online &&
    !["starting", "restarting"].includes(daemon.state);

  useEffect(() => {
    void boot();
  }, [boot]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        setCommandOpen(!useUiStore.getState().commandOpen);
        return;
      }
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "n") {
        event.preventDefault();
        navigate("/kanban");
        setTaskDialogOpen(true);
        return;
      }
      if (
        (event.metaKey || event.ctrlKey) &&
        event.shiftKey &&
        event.key.toLowerCase() === "r"
      ) {
        event.preventDefault();
        void refresh();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [navigate, refresh, setCommandOpen, setTaskDialogOpen]);

  return (
    <div className="flex h-screen w-screen overflow-hidden bg-[#09090b] text-zinc-100">
      <Sidebar />
      <div className="flex min-w-0 flex-1 flex-col">
        <Topbar />
        {loading ? (
          <div className="h-0.5 w-full overflow-hidden bg-zinc-900">
            <div className="h-full w-1/3 animate-[loading_1.2s_ease-in-out_infinite] bg-blue-500/70" />
          </div>
        ) : null}
        {error ? (
          <div className="flex items-center gap-3 border-b border-red-500/20 bg-red-500/10 px-4 py-2 text-[11px] text-red-300">
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
          <div className="flex flex-wrap items-center gap-3 border-b border-amber-500/20 bg-amber-500/10 px-4 py-2 text-[11px] text-amber-200">
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
              onClick={() => navigate("/settings")}
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
                ? "flex flex-wrap items-center gap-3 border-b border-red-500/25 bg-red-500/10 px-4 py-2 text-[11px] text-red-200"
                : "flex flex-wrap items-center gap-3 border-b border-amber-500/25 bg-amber-500/10 px-4 py-2 text-[11px] text-amber-200"
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
        <main className="min-h-0 flex-1 overflow-y-auto">
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
            <Outlet />
          </Suspense>
        </main>
      </div>
      <CommandPalette />
      <WorkspaceDialog />
      <ConfirmDialog />
      <Toaster />
    </div>
  );
}

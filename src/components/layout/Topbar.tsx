import { Check, ChevronsUpDown, ListChecks, Plus, RefreshCw, Search } from "lucide-react";
import { useNavigate } from "react-router-dom";
import { useEffect, useMemo, useRef, useState } from "react";
import { useAppStore } from "@/store/app-store";
import { useUiStore } from "@/store/ui-store";
import { confirmAction } from "@/store/confirm-store";
import { Button } from "@/components/ui/button";
import { StatusDot, statusTone } from "@/components/ui/status-dot";
import { cn } from "@/lib/utils";
import { OnboardingDialog } from "@/components/onboarding/OnboardingDialog";
import { formatUsd } from "@/lib/format";
import { links } from "@/lib/links";
import { allQueueCounts } from "@/lib/queue";

const FALLBACK_DAILY_LIMIT = 5;

export function Topbar() {
  const daemon = useAppStore((state) => state.daemon);
  const dashboard = useAppStore((state) => state.dashboard);
  const budget = useAppStore((state) => state.budget);
  const workspaces = useAppStore((state) => state.workspaces);
  const activeWorkspaceId = useAppStore((state) => state.activeWorkspaceId);
  const selectWorkspace = useAppStore((state) => state.selectWorkspace);
  const restartDaemon = useAppStore((state) => state.restartDaemon);
  const setCommandOpen = useUiStore((state) => state.setCommandOpen);
  const setWorkspaceDialogOpen = useUiStore(
    (state) => state.setWorkspaceDialogOpen,
  );
  const setTaskDialogOpen = useUiStore((state) => state.setTaskDialogOpen);
  const navigate = useNavigate();
  const [workspaceOpen, setWorkspaceOpen] = useState(false);
  const [restarting, setRestarting] = useState(false);
  const [workspaceQuery, setWorkspaceQuery] = useState("");
  const [onboardingOpen, setOnboardingOpen] = useState(false);
  const workspaceRef = useRef<HTMLDivElement | null>(null);

  const activeWorkspace = workspaces.find(
    (workspace) => workspace.id === activeWorkspaceId,
  );

  const filteredWorkspaces = useMemo(() => {
    const needle = workspaceQuery.trim().toLowerCase();
    if (!needle) return workspaces;
    return workspaces.filter((workspace) =>
      `${workspace.name} ${workspace.root_path}`.toLowerCase().includes(needle),
    );
  }, [workspaceQuery, workspaces]);

  useEffect(() => {
    if (!workspaceOpen) return;
    const onPointerDown = (event: MouseEvent) => {
      if (!workspaceRef.current?.contains(event.target as Node)) {
        setWorkspaceOpen(false);
      }
    };
    window.addEventListener("mousedown", onPointerDown);
    return () => window.removeEventListener("mousedown", onPointerDown);
  }, [workspaceOpen]);
  const queue = useAppStore((state) => state.queue);
  const queueStatus = useAppStore((state) => state.queueStatus);
  const queueCounts = allQueueCounts(queue);
  const spent = dashboard?.daily_cost_usd ?? budget?.daily_spent ?? 0;
  const dailyLimit = budget?.daily_limit ?? FALLBACK_DAILY_LIMIT;
  const spentPct = Math.min(100, (spent / Math.max(dailyLimit, 0.01)) * 100);
  const running = queueStatus?.running ?? queueCounts.running;
  const waiting = queueStatus?.waiting ?? queueCounts.waiting;
  const activeJobs = daemon?.active_jobs ?? 0;

  async function requestRestart() {
    if (activeJobs > 0 || running > 0) {
      const confirmed = await confirmAction({
        title: "Reiniciar o OpenCode?",
        description: `Há ${Math.max(activeJobs, running)} job(s) em execução. Reiniciar o motor interrompe o trabalho atual.`,
        confirmLabel: "Reiniciar mesmo assim",
        tone: "destructive",
      });
      if (!confirmed) return;
    }
    setRestarting(true);
    try {
      await restartDaemon();
    } finally {
      setRestarting(false);
    }
  }

  return (
    <>
    <header className="flex h-14 shrink-0 items-center gap-1.5 border-b workspace-divider bg-[#0b0b0d]/90 px-2 backdrop-blur sm:gap-3 sm:px-4">
      <div ref={workspaceRef} className="relative min-w-0 shrink-0">
        <button
          type="button"
          onClick={() => {
            setWorkspaceQuery("");
            setWorkspaceOpen((current) => !current);
          }}
          className="flex h-9 w-[132px] items-center gap-2 rounded-lg sm:w-[220px] border border-zinc-800 bg-zinc-900/50 px-3 text-left text-[14px] font-medium text-zinc-200 outline-none transition-colors hover:border-zinc-700 focus:border-blue-500/60"
          title={activeWorkspace?.root_path}
          aria-expanded={workspaceOpen}
          aria-label={`Workspace ativo: ${activeWorkspace?.name ?? "nenhum"}`}
        >
          <span
            className="h-2 w-2 shrink-0 rounded-full"
            style={{ background: activeWorkspace?.color_hex ?? "#52525b" }}
          />
          <span className="min-w-0 flex-1 truncate">
            {activeWorkspace?.name ?? "Nenhum workspace"}
          </span>
          <ChevronsUpDown size={14} className="shrink-0 text-zinc-500" />
        </button>
        {workspaceOpen ? (
          <div className="absolute left-0 top-10 z-40 w-[340px] overflow-hidden rounded-xl border border-zinc-700 bg-zinc-950 shadow-2xl">
            <div className="border-b border-zinc-800/70 p-2">
              <input
                autoFocus
                value={workspaceQuery}
                onChange={(event) => setWorkspaceQuery(event.target.value)}
                placeholder="Buscar workspace ou caminho…"
                className="h-8 w-full rounded-lg border border-zinc-800 bg-zinc-900/60 px-2.5 text-[13.5px] text-zinc-100 outline-none placeholder:text-zinc-500 focus:border-blue-500/60"
              />
            </div>
            <div className="max-h-72 overflow-y-auto p-1.5">
              {filteredWorkspaces.map((workspace) => (
                <button
                  key={workspace.id}
                  type="button"
                  onClick={() => {
                    setWorkspaceOpen(false);
                    if (workspace.id !== activeWorkspaceId) {
                      void selectWorkspace(workspace.id);
                    }
                  }}
                  className="flex w-full items-center gap-2 rounded-lg px-2 py-2 text-left hover:bg-zinc-900"
                >
                  <span
                    className="h-2.5 w-2.5 shrink-0 rounded-full"
                    style={{ background: workspace.color_hex }}
                  />
                  <span className="min-w-0 flex-1">
                    <span className="block truncate text-[13.5px] text-zinc-200">
                      {workspace.name}
                    </span>
                    <span className="block truncate font-mono text-[12px] text-zinc-500">
                      {workspace.root_path}
                    </span>
                  </span>
                  {workspace.id === activeWorkspaceId ? (
                    <Check size={13} className="text-emerald-400" />
                  ) : null}
                </button>
              ))}
              {filteredWorkspaces.length === 0 ? (
                <p className="px-2 py-3 text-center text-[13px] text-zinc-500">
                  Nenhum workspace encontrado.
                </p>
              ) : null}
            </div>
            <div className="grid grid-cols-2 gap-1 border-t border-zinc-800/70 p-1.5">
              <button
                type="button"
                onClick={() => {
                  setWorkspaceOpen(false);
                  setWorkspaceDialogOpen(true);
                }}
                className="flex items-center justify-center gap-1.5 rounded-lg px-2 py-1.5 text-[13px] text-zinc-400 hover:bg-zinc-900 hover:text-zinc-200"
              >
                <Plus size={12} />
                Novo workspace
              </button>
              <button
                type="button"
                onClick={() => {
                  setWorkspaceOpen(false);
                  navigate("/settings?section=workspaces");
                }}
                className="flex items-center justify-center gap-1.5 rounded-lg px-2 py-1.5 text-[13px] text-zinc-400 hover:bg-zinc-900 hover:text-zinc-200"
              >
                Gerenciar
              </button>
            </div>
          </div>
        ) : null}
      </div>

      <Button
        variant="ghost"
        size="icon"
        className="shrink-0"
        aria-label="Novo workspace"
        title="Novo workspace"
        onClick={() => setWorkspaceDialogOpen(true)}
      >
        <Plus size={18} />
      </Button>

      <div className="ml-auto flex items-center gap-2">
        <div className="hidden items-center gap-2 rounded-lg border border-zinc-800/70 bg-zinc-900/30 px-2.5 py-1.5 2xl:flex">
          <span className="font-mono text-[12px] uppercase tracking-wide text-zinc-500">
            hoje
          </span>
          <div className="h-1.5 w-16 overflow-hidden rounded-full bg-zinc-800">
            <div
              className={cn(
                "h-full rounded-full transition-all",
                spentPct >= 80 ? "bg-amber-400" : "bg-blue-400",
              )}
              style={{ width: `${spent > 0 ? Math.max(spentPct, 3) : 0}%` }}
            />
          </div>
          <span className="whitespace-nowrap font-mono text-[12px] text-zinc-400">
            {formatUsd(spent)} / {formatUsd(dailyLimit)}
          </span>
        </div>

        <button
          type="button"
          onClick={() => navigate(links.queue())}
          className="hidden items-center gap-2 rounded-lg border border-zinc-800/70 bg-zinc-900/30 px-2.5 py-1.5 transition-colors hover:border-zinc-700 hover:bg-zinc-900/60 2xl:flex"
          title={`${running} execução(ões) ativa(s) e ${waiting} job(s) na fila — abrir a fila`}
        >
          <span className="font-mono text-[12px] uppercase tracking-wide text-zinc-500">
            fila
          </span>
          <span className="whitespace-nowrap text-[13px] text-zinc-300">
            {running} {running === 1 ? "ativo" : "ativos"}
          </span>
          <span className="text-zinc-600">·</span>
          <span className="whitespace-nowrap text-[13px] text-zinc-400">
            {waiting} na fila
          </span>
        </button>

        <button
          type="button"
          onClick={() => void requestRestart()}
          className="flex h-9 items-center gap-2 rounded-lg border border-zinc-800/70 bg-zinc-900/30 px-2.5 text-[13px] text-zinc-400 transition-colors hover:border-zinc-700 hover:text-zinc-200"
          title="Reiniciar OpenCode Daemon"
          aria-label={`Reiniciar OpenCode Daemon — ${daemon?.online ? "online" : daemon?.state ?? "offline"}`}
        >
          <StatusDot
            tone={statusTone(daemon?.online ?? false, daemon?.state)}
          />
          <span className="hidden font-mono text-[12px] uppercase tracking-wide lg:inline">
            {daemon?.online ? "engine online" : daemon?.state ?? "offline"}
          </span>
          <RefreshCw
            size={12}
            className={
              restarting ? "animate-spin text-blue-300" : "text-zinc-500"
            }
          />
        </button>

        <Button
          variant="outline"
          size="sm"
          className="h-9 gap-2 px-2.5 text-zinc-400"
          onClick={() => setOnboardingOpen(true)}
          title="Primeiros passos e verificação das integrações"
        >
          <ListChecks size={14} />
          <span className="hidden text-[12.5px] 2xl:inline">Primeiros passos</span>
          <span className="sr-only">Primeiros passos</span>
        </Button>

        <Button
          variant="outline"
          size="sm"
          className="h-9 gap-2 px-2.5 text-zinc-400"
          onClick={() => setCommandOpen(true)}
          title="Buscar e executar comandos (Ctrl/Cmd+K)"
        >
          <Search size={14} />
          <span className="hidden text-[12.5px] sm:inline">Buscar</span>
          <span className="sr-only"> e executar comandos</span>
        </Button>

        <Button
          size="sm"
          className="h-9"
          onClick={() => {
            navigate(links.kanban());
            setTaskDialogOpen(true);
          }}
          title="Nova tarefa (Ctrl/Cmd+N)"
        >
          <Plus size={15} />
          <span className="hidden sm:inline">Nova tarefa</span>
        </Button>
      </div>
    </header>
    <OnboardingDialog open={onboardingOpen} onClose={() => setOnboardingOpen(false)} />
    </>
  );
}

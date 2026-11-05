import { ChevronDown, Command, Plus, RefreshCw, Search } from "lucide-react";
import { useAppStore } from "@/store/app-store";
import { useUiStore } from "@/store/ui-store";
import { Button } from "@/components/ui/button";
import { StatusDot, statusTone } from "@/components/ui/status-dot";
import { cn } from "@/lib/utils";
import { formatUsd, shortPath } from "@/lib/format";
import { ROUTER_POLICY_LABELS } from "@/lib/types";

const DAILY_LIMIT = 5;

export function Topbar() {
  const daemon = useAppStore((state) => state.daemon);
  const dashboard = useAppStore((state) => state.dashboard);
  const workspaces = useAppStore((state) => state.workspaces);
  const activeWorkspaceId = useAppStore((state) => state.activeWorkspaceId);
  const selectWorkspace = useAppStore((state) => state.selectWorkspace);
  const restartDaemon = useAppStore((state) => state.restartDaemon);
  const setCommandOpen = useUiStore((state) => state.setCommandOpen);
  const setWorkspaceDialogOpen = useUiStore(
    (state) => state.setWorkspaceDialogOpen,
  );

  const activeWorkspace = workspaces.find(
    (workspace) => workspace.id === activeWorkspaceId,
  );
  const spent = dashboard?.estimated_cost_usd ?? 0;
  const spentPct = Math.min(100, (spent / DAILY_LIMIT) * 100);

  return (
    <header className="flex h-14 shrink-0 items-center gap-3 border-b border-zinc-800/70 bg-[#0b0b0d]/90 px-4 backdrop-blur">
      <div className="flex min-w-0 items-center gap-2">
        <div className="relative min-w-0">
          <select
            value={activeWorkspaceId ?? ""}
            onChange={(event) => {
              if (event.target.value) void selectWorkspace(event.target.value);
            }}
            className="h-9 w-[220px] appearance-none rounded-lg border border-zinc-800 bg-zinc-900/50 pl-3 pr-8 text-[13px] font-medium text-zinc-200 outline-none transition-colors hover:border-zinc-700 focus:border-blue-500/60"
            title={activeWorkspace?.root_path}
          >
            {workspaces.length === 0 ? (
              <option value="">Nenhum workspace</option>
            ) : null}
            {workspaces.map((workspace) => (
              <option key={workspace.id} value={workspace.id}>
                {workspace.name}
              </option>
            ))}
          </select>
          <ChevronDown
            size={14}
            className="pointer-events-none absolute right-2.5 top-1/2 -translate-y-1/2 text-zinc-500"
          />
        </div>

        <Button
          variant="ghost"
          size="icon"
          title="Novo workspace"
          onClick={() => setWorkspaceDialogOpen(true)}
        >
          <Plus size={17} />
        </Button>

        {activeWorkspace ? (
          <div className="hidden min-w-0 border-l border-zinc-800/70 pl-3 xl:block">
            <p className="truncate font-mono text-[10px] text-zinc-600">
              {shortPath(activeWorkspace.root_path, 56)}
            </p>
            <p className="truncate text-[10px] text-zinc-700">
              {ROUTER_POLICY_LABELS[activeWorkspace.default_router_policy] ??
                activeWorkspace.default_router_policy}{" "}
              · {activeWorkspace.timezone}
            </p>
          </div>
        ) : null}
      </div>

      <div className="ml-auto flex items-center gap-2">
        <div className="hidden items-center gap-2 rounded-lg border border-zinc-800/70 bg-zinc-900/30 px-2.5 py-1.5 lg:flex">
          <span className="font-mono text-[10px] uppercase tracking-wide text-zinc-600">
            hoje
          </span>
          <div className="h-1.5 w-16 overflow-hidden rounded-full bg-zinc-800">
            <div
              className={cn(
                "h-full rounded-full transition-all",
                spentPct >= 80 ? "bg-amber-400" : "bg-blue-400",
              )}
              style={{ width: `${spentPct}%` }}
            />
          </div>
          <span className="font-mono text-[10px] text-zinc-400">
            {formatUsd(spent)} / {formatUsd(DAILY_LIMIT)}
          </span>
        </div>

        <div className="hidden items-center gap-2 rounded-lg border border-zinc-800/70 bg-zinc-900/30 px-2.5 py-1.5 md:flex">
          <span className="font-mono text-[10px] uppercase tracking-wide text-zinc-600">
            fila
          </span>
          <span className="font-mono text-[10px] text-zinc-300">
            {dashboard?.queue_running ?? 0}● {dashboard?.queue_waiting ?? 0}○
          </span>
        </div>

        <button
          type="button"
          onClick={() => void restartDaemon()}
          className="flex h-9 items-center gap-2 rounded-lg border border-zinc-800/70 bg-zinc-900/30 px-2.5 text-[11px] text-zinc-400 transition-colors hover:border-zinc-700 hover:text-zinc-200"
          title="Reiniciar OpenCode Daemon"
        >
          <StatusDot
            tone={statusTone(daemon?.online ?? false, daemon?.state)}
          />
          <span className="hidden font-mono text-[10px] uppercase tracking-wide lg:inline">
            {daemon?.online ? "engine online" : daemon?.state ?? "offline"}
          </span>
          <RefreshCw size={12} className="text-zinc-600" />
        </button>

        <Button
          variant="outline"
          size="sm"
          className="hidden h-9 gap-2 px-2.5 text-zinc-500 xl:flex"
          onClick={() => setCommandOpen(true)}
        >
          <Search size={14} />
          <span className="text-[11px]">Buscar</span>
          <span className="ml-2 flex items-center gap-0.5 rounded border border-zinc-800 bg-zinc-950 px-1 py-0.5 font-mono text-[9px] text-zinc-600">
            <Command size={9} />K
          </span>
        </Button>

        <Button
          size="sm"
          className="h-9"
          onClick={() => setCommandOpen(true)}
          title="Nova tarefa (disponível na Fase 3)"
        >
          <Plus size={15} />
          <span className="hidden sm:inline">Nova tarefa</span>
        </Button>
      </div>
    </header>
  );
}

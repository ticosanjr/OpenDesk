import { Check, ChevronsUpDown, Command, Plus, RefreshCw, Search } from "lucide-react";
import { Link, useNavigate } from "react-router-dom";
import { useEffect, useMemo, useRef, useState } from "react";
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
  const setTaskDialogOpen = useUiStore((state) => state.setTaskDialogOpen);
  const navigate = useNavigate();
  const [workspaceOpen, setWorkspaceOpen] = useState(false);
  const [workspaceQuery, setWorkspaceQuery] = useState("");
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
  const spent = dashboard?.daily_cost_usd ?? 0;
  const spentPct = Math.min(100, (spent / DAILY_LIMIT) * 100);

  return (
    <header className="flex h-14 shrink-0 items-center gap-3 border-b workspace-divider bg-[#0b0b0d]/90 px-4 backdrop-blur">
      <div className="flex min-w-0 items-center gap-2">
        <div ref={workspaceRef} className="relative min-w-0 shrink-0">
          <button
            type="button"
            onClick={() => {
              setWorkspaceQuery("");
              setWorkspaceOpen((current) => !current);
            }}
            className="flex h-9 w-[220px] items-center gap-2 rounded-lg border border-zinc-800 bg-zinc-900/50 px-3 text-left text-[13px] font-medium text-zinc-200 outline-none transition-colors hover:border-zinc-700 focus:border-blue-500/60"
            title={activeWorkspace?.root_path}
            aria-expanded={workspaceOpen}
            aria-label="Workspace ativo"
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
                  className="h-8 w-full rounded-lg border border-zinc-800 bg-zinc-900/60 px-2.5 text-[11px] text-zinc-100 outline-none placeholder:text-zinc-600 focus:border-blue-500/60"
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
                      <span className="block truncate text-[11px] text-zinc-200">
                        {workspace.name}
                      </span>
                      <span className="block truncate font-mono text-[9px] text-zinc-600">
                        {shortPath(workspace.root_path, 48)}
                      </span>
                    </span>
                    {workspace.id === activeWorkspaceId ? (
                      <Check size={13} className="text-emerald-400" />
                    ) : null}
                  </button>
                ))}
                {filteredWorkspaces.length === 0 ? (
                  <p className="px-2 py-3 text-center text-[10px] text-zinc-600">
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
                  className="flex items-center justify-center gap-1.5 rounded-lg px-2 py-1.5 text-[10px] text-zinc-400 hover:bg-zinc-900 hover:text-zinc-200"
                >
                  <Plus size={12} />
                  Novo workspace
                </button>
                <button
                  type="button"
                  onClick={() => {
                    setWorkspaceOpen(false);
                    navigate("/settings");
                  }}
                  className="flex items-center justify-center gap-1.5 rounded-lg px-2 py-1.5 text-[10px] text-zinc-400 hover:bg-zinc-900 hover:text-zinc-200"
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
          aria-label="Novo workspace"
          title="Novo workspace"
          className="shrink-0"
          onClick={() => setWorkspaceDialogOpen(true)}
        >
          <Plus size={17} />
        </Button>

        {activeWorkspace ? (
          <div className="hidden min-w-0 border-l border-zinc-800/70 pl-3 xl:block">
            <Link
              to="/files"
              className="block truncate font-mono text-[10px] text-zinc-600 hover:text-blue-300"
              title={activeWorkspace.root_path}
            >
              {shortPath(activeWorkspace.root_path, 56)}
            </Link>
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
          onClick={() => {
            navigate("/kanban");
            setTaskDialogOpen(true);
          }}
          title="Nova tarefa"
        >
          <Plus size={15} />
          <span className="hidden sm:inline">Nova tarefa</span>
        </Button>
      </div>
    </header>
  );
}

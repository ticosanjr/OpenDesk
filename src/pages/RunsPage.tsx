import { useCallback, useEffect, useMemo, useState } from "react";
import {
  Activity,
  CheckCircle2,
  Clock3,
  Cpu,
  Loader2,
  RefreshCw,
  Search,
  XCircle,
} from "lucide-react";
import { Link } from "react-router-dom";
import { PageBody, PageHeader } from "@/components/layout/PageHeader";
import { Card, CardContent } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { EmptyState } from "@/components/ui/empty-state";
import { LoadingState } from "@/components/ui/loading-state";
import { useAppStore } from "@/store/app-store";
import { useUiStore } from "@/store/ui-store";
import { apiGet } from "@/lib/api";
import type { TaskRun } from "@/lib/types";
import { links } from "@/lib/links";
import {
  formatDuration,
  formatNumber,
  formatUsd,
  relativeTime,
} from "@/lib/format";
import { cn } from "@/lib/utils";

const STATUSES = ["", "SUCCESS", "FAILED", "RUNNING", "CANCELLED"];

export function RunsPage() {
  const activeWorkspaceId = useAppStore((state) => state.activeWorkspaceId);
  const workspaces = useAppStore((state) => state.workspaces);
  const setWorkspaceDialogOpen = useUiStore(
    (state) => state.setWorkspaceDialogOpen,
  );
  const lastRunEvent = useAppStore((state) => state.lastRunEvent);
  const workspace = workspaces.find((item) => item.id === activeWorkspaceId);
  const [runs, setRuns] = useState<TaskRun[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [status, setStatus] = useState("");
  const [limit, setLimit] = useState(100);

  const load = useCallback(async () => {
    if (!activeWorkspaceId) {
      setRuns([]);
      return;
    }
    setLoading(true);
    try {
      const list = await apiGet<TaskRun[]>(
        `/api/runs?workspace_id=${encodeURIComponent(activeWorkspaceId)}&limit=${limit}`,
      );
      setRuns(list);
      setError(null);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setLoading(false);
    }
  }, [activeWorkspaceId, limit]);

  useEffect(() => {
    void load();
  }, [load, lastRunEvent?.run_id, lastRunEvent?.status]);

  const filtered = useMemo(() => {
    const needle = query.trim().toLowerCase();
    return runs.filter((run) => {
      if (status && run.status !== status) return false;
      if (!needle) return true;
      return `${run.task_title ?? ""} ${run.agent_name} ${run.model_id} ${run.id}`
        .toLowerCase()
        .includes(needle);
    });
  }, [query, runs, status]);

  if (!activeWorkspaceId) {
    return (
      <PageBody>
        <PageHeader eyebrow="auditoria" title="Execuções" />
        <EmptyState
          icon={Activity}
          title="Nenhum workspace ativo"
          description="Selecione um workspace para ver os runs e logs."
          action={
            <Button size="sm" onClick={() => setWorkspaceDialogOpen(true)}>
              Criar workspace
            </Button>
          }
        />
      </PageBody>
    );
  }

  return (
    <PageBody>
      <PageHeader
        eyebrow="auditoria"
        title="Execuções"
        description={`${workspace?.name ?? "Workspace"} · runs, duração, tokens e logs estruturados.`}
        actions={
          <Button variant="outline" size="sm" onClick={() => void load()}>
            <RefreshCw size={13} className={loading ? "animate-spin" : undefined} />
            Atualizar
          </Button>
        }
      />

      {error ? (
        <p className="rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[11px] text-red-300">
          {error}
        </p>
      ) : null}

      <div className="flex flex-wrap items-center gap-2 rounded-xl border border-zinc-800/70 bg-zinc-900/20 px-3 py-2">
        <div className="flex h-8 min-w-[220px] flex-1 items-center gap-2 rounded-lg border border-zinc-800 bg-zinc-950/40 px-2.5">
          <Search size={13} className="text-zinc-500" />
          <input
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="Buscar por tarefa, agente, modelo ou run id…"
            className="h-full flex-1 bg-transparent text-[11px] outline-none placeholder:text-zinc-500"
          />
        </div>
        <select
          value={status}
          onChange={(event) => setStatus(event.target.value)}
          className="h-8 rounded-lg border border-zinc-800 bg-zinc-900/50 px-2 text-[10px] text-zinc-300"
          aria-label="Filtrar por status"
        >
          {STATUSES.map((value) => (
            <option key={value || "all"} value={value}>
              {value || "todos os status"}
            </option>
          ))}
        </select>
        <Badge variant="default">
          {filtered.length} de {runs.length}
        </Badge>
      </div>

      <Card>
        <CardContent className="p-0">
          {loading && runs.length === 0 ? (
            <LoadingState compact label="Carregando execuções…" rows={4} />
          ) : filtered.length === 0 ? (
            <EmptyState
              icon={Activity}
              title={runs.length === 0 ? "Nenhuma execução ainda" : "Nada encontrado"}
              description={
                runs.length === 0
                  ? "Execute uma tarefa ou rotina para ver o histórico aqui."
                  : "Ajuste a busca ou o filtro de status."
              }
              className="border-0 bg-transparent"
            />
          ) : (
            <div className="divide-y divide-zinc-800/60">
              {filtered.map((run) => (
                <Link
                  key={run.id}
                  to={links.run(run.id)}
                  className="flex flex-wrap items-center gap-3 px-4 py-3 transition-colors hover:bg-zinc-900/40"
                >
                  {run.status === "SUCCESS" ? (
                    <CheckCircle2 size={15} className="shrink-0 text-emerald-400" />
                  ) : run.status === "FAILED" ? (
                    <XCircle size={15} className="shrink-0 text-red-400" />
                  ) : (
                    <Loader2
                      size={15}
                      className={cn(
                        "shrink-0 text-blue-400",
                        run.status === "RUNNING" && "animate-spin",
                      )}
                    />
                  )}
                  <div className="min-w-0 flex-1">
                    <div className="flex flex-wrap items-center gap-2">
                      <p className="truncate text-[12px] font-medium text-zinc-200">
                        {run.task_title ?? run.agent_name}
                      </p>
                      <Badge
                        variant={
                          run.status === "SUCCESS"
                            ? "green"
                            : run.status === "FAILED"
                              ? "red"
                              : run.status === "RUNNING"
                                ? "blue"
                                : "yellow"
                        }
                      >
                        {run.status}
                      </Badge>
                      <Badge variant="default">{run.phase ?? "run"}</Badge>
                    </div>
                    <p className="mt-0.5 flex flex-wrap items-center gap-x-3 gap-y-0.5 font-mono text-[9px] text-zinc-500">
                      <span>@{run.agent_name}</span>
                      <span>{run.model_id}</span>
                      <span className="inline-flex items-center gap-1">
                        <Clock3 size={9} />
                        {formatDuration(run.duration_ms)}
                      </span>
                      <span className="inline-flex items-center gap-1">
                        <Cpu size={9} />
                        {formatNumber(run.tokens_input + run.tokens_output)} tokens
                      </span>
                      <span>{formatUsd(run.estimated_cost_usd)}</span>
                    </p>
                    {run.error_message ? (
                      <p className="mt-1 truncate font-mono text-[9px] text-red-300/80">
                        {run.error_message}
                      </p>
                    ) : null}
                  </div>
                  <div className="flex shrink-0 flex-col items-end gap-1 text-right">
                    <span className="font-mono text-[9px] text-zinc-500">
                      {relativeTime(run.started_at)}
                    </span>
                    <span className="font-mono text-[9px] text-zinc-500">
                      abrir logs →
                    </span>
                  </div>
                </Link>
              ))}
            </div>
          )}
          {filtered.length >= limit && runs.length >= limit ? (
            <div className="border-t border-zinc-800/60 p-3 text-center">
              <Button variant="outline" size="sm" onClick={() => setLimit((value) => value + 100)}>
                Carregar mais
              </Button>
            </div>
          ) : null}
        </CardContent>
      </Card>
    </PageBody>
  );
}

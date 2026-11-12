import { useCallback, useEffect, useMemo, useState } from "react";
import {
  Activity,
  CheckCircle2,
  ChevronRight,
  Clock3,
  Cpu,
  Loader2,
  RefreshCw,
  XCircle,
} from "lucide-react";
import { Link } from "react-router-dom";
import { PageBody, PageHeader } from "@/components/layout/PageHeader";
import { Card, CardContent } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { EmptyState } from "@/components/ui/empty-state";
import { FilterBar, FilterSearch, FilterSelect } from "@/components/ui/filter-bar";
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
import { runStatusLabel } from "@/lib/task-ui";

const STATUSES: Array<[string, string]> = [
  ["", "todos os status"],
  ["SUCCESS", "sucesso"],
  ["FAILED", "falhou"],
  ["RUNNING", "executando"],
  ["CANCELLED", "cancelada"],
];

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

  const grouped = useMemo(() => {
    const groups = new Map<string, TaskRun[]>();
    for (const run of filtered) {
      const key = run.task_id ?? run.id;
      groups.set(key, [...(groups.get(key) ?? []), run]);
    }
    return Array.from(groups.values())
      .map((attempts) => {
        const ordered = attempts
          .slice()
          .sort((left, right) => left.started_at.localeCompare(right.started_at));
        const latest = ordered[ordered.length - 1]!;
        return {
          latest,
          attempts: ordered,
          tokensInput: ordered.reduce(
            (total, run) => total + run.tokens_input,
            0,
          ),
          tokensOutput: ordered.reduce(
            (total, run) => total + run.tokens_output,
            0,
          ),
          cost: ordered.reduce(
            (total, run) => total + run.estimated_cost_usd,
            0,
          ),
          duration: ordered.reduce((total, run) => total + run.duration_ms, 0),
        };
      })
      .sort((left, right) =>
        right.latest.started_at.localeCompare(left.latest.started_at),
      );
  }, [filtered]);

  const threadCount = useMemo(
    () => new Set(runs.map((run) => run.task_id ?? run.id)).size,
    [runs],
  );

  if (!activeWorkspaceId) {
    return (
      <PageBody>
        <PageHeader title="Execuções" />
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

      <FilterBar>
        <FilterSearch
          value={query}
          onChange={setQuery}
          placeholder="Buscar por tarefa, agente, modelo ou run id…"
          ariaLabel="Buscar execuções"
        />
        <FilterSelect
          value={status}
          onChange={setStatus}
          ariaLabel="Filtrar por status"
        >
          {STATUSES.map(([value, label]) => (
            <option key={value || "all"} value={value}>
              {label}
            </option>
          ))}
        </FilterSelect>
        <Badge variant="default">
          {grouped.length} de {threadCount} execuções
        </Badge>
      </FilterBar>

      <Card>
        <CardContent className="p-0">
          {loading && runs.length === 0 ? (
            <LoadingState compact label="Carregando execuções…" rows={4} />
          ) : grouped.length === 0 ? (
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
              {grouped.map(({ latest: run, attempts, tokensInput, tokensOutput, cost, duration }) => (
                <Link
                  key={run.task_id ?? run.id}
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
                      <p className="truncate text-[14px] font-medium text-zinc-100">
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
                        {runStatusLabel(run.status)}
                      </Badge>
                      {attempts.length > 1 ? (
                        <Badge
                          variant="purple"
                          title="Tentativas internas da mesma execução"
                        >
                          {attempts.length} tentativas
                        </Badge>
                      ) : null}
                      <Badge variant="default">{run.phase ?? "run"}</Badge>
                    </div>
                    <p className="mt-1 flex flex-wrap items-center gap-x-3 gap-y-1 font-mono text-[11.5px] text-zinc-500">
                      <span>@{run.agent_name}</span>
                      <span>{run.model_id}</span>
                      <span className="inline-flex items-center gap-1">
                        <Clock3 size={11} />
                        {formatDuration(duration)}
                      </span>
                      <span className="inline-flex items-center gap-1">
                        <Cpu size={11} />
                        ↑ {formatNumber(tokensInput)} · ↓{" "}
                        {formatNumber(tokensOutput)} · Σ{" "}
                        {formatNumber(tokensInput + tokensOutput)} tokens
                      </span>
                      <span>{formatUsd(cost)}</span>
                    </p>
                    {run.error_message ? (
                      <p className="mt-1 truncate font-mono text-[11px] text-red-300/80">
                        {run.error_message}
                      </p>
                    ) : null}
                  </div>
                  <div className="flex shrink-0 items-center gap-2 text-right">
                    <span className="font-mono text-[11.5px] text-zinc-500">
                      {relativeTime(run.started_at)}
                    </span>
                    <ChevronRight size={16} className="text-zinc-600" />
                  </div>
                </Link>
              ))}
            </div>
          )}
          {grouped.length >= limit && runs.length >= limit ? (
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

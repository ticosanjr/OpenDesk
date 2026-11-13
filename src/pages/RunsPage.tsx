import { useCallback, useEffect, useMemo, useState } from "react";
import {
  Activity,
  ChevronRight,
  Clock3,
  Cpu,
  LayoutList,
  RefreshCw,
  Rows3,
} from "lucide-react";
import { Link, useNavigate } from "react-router-dom";
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
import { runStatusLabel, runStatusTone } from "@/lib/status";
import { RunStatusIcon } from "@/components/ui/run-status";
import {
  RUN_SCOPE_LABEL,
  RUN_SCOPE_OPTIONS,
  matchesRunScope,
  runScope,
} from "@/lib/run-scope";

const STATUSES: Array<[string, string]> = [
  ["", "todos os status"],
  ["SUCCESS", "sucesso"],
  ["FAILED", "falhou"],
  ["RUNNING", "executando"],
  ["CANCELLED", "cancelada"],
];

type SortKey = "recent" | "duration" | "cost" | "tokens";
type RunsView = "cards" | "table";

const SORTS: Array<[SortKey, string]> = [
  ["recent", "mais recentes"],
  ["duration", "maior duração"],
  ["cost", "maior custo"],
  ["tokens", "mais tokens"],
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
  const [scope, setScope] = useState("");
  const [limit, setLimit] = useState(100);
  const [sort, setSort] = useState<SortKey>("recent");
  const [view, setView] = useState<RunsView>("cards");
  const navigate = useNavigate();

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
      if (!matchesRunScope(run, scope)) return false;
      if (!needle) return true;
      return `${run.task_title ?? ""} ${run.agent_name} ${run.model_id} ${run.id}`
        .toLowerCase()
        .includes(needle);
    });
  }, [query, runs, scope, status]);

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

  const ordered = useMemo(() => {
    const list = [...grouped];
    if (sort === "duration") {
      list.sort((left, right) => right.duration - left.duration);
    } else if (sort === "cost") {
      list.sort((left, right) => right.cost - left.cost);
    } else if (sort === "tokens") {
      list.sort(
        (left, right) =>
          right.tokensInput +
          right.tokensOutput -
          (left.tokensInput + left.tokensOutput),
      );
    } else {
      list.sort((left, right) =>
        right.latest.started_at.localeCompare(left.latest.started_at),
      );
    }
    return list;
  }, [grouped, sort]);

  const threadCount = useMemo(
    () => new Set(runs.map((run) => run.task_id ?? run.id)).size,
    [runs],
  );
  const summary = useMemo(
    () => ({
      success: grouped.filter((item) => item.latest.status === "SUCCESS").length,
      failed: grouped.filter((item) => item.latest.status === "FAILED").length,
      running: grouped.filter((item) => item.latest.status === "RUNNING").length,
    }),
    [grouped],
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
        <p className="rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[13px] text-red-300">
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
        <FilterSelect
          value={scope}
          onChange={setScope}
          ariaLabel="Filtrar por origem da execução"
        >
          {RUN_SCOPE_OPTIONS.map(([value, label]) => (
            <option key={value || "all"} value={value}>
              {label}
            </option>
          ))}
        </FilterSelect>
        <FilterSelect
          value={sort}
          onChange={(value) => setSort(value as SortKey)}
          ariaLabel="Ordenar execuções"
        >
          {SORTS.map(([value, label]) => (
            <option key={value} value={value}>
              {label}
            </option>
          ))}
        </FilterSelect>
        <div
          className="flex rounded-lg border border-zinc-800 bg-zinc-950/40 p-0.5"
          role="group"
          aria-label="Modo de visualização"
        >
          <button
            type="button"
            aria-pressed={view === "cards"}
            aria-label="Ver em cartões"
            title="Ver em cartões"
            onClick={() => setView("cards")}
            className={cn(
              "inline-flex h-7 items-center rounded-md px-2",
              view === "cards"
                ? "bg-zinc-800 text-zinc-100"
                : "text-zinc-500 hover:text-zinc-200",
            )}
          >
            <Rows3 size={13} />
          </button>
          <button
            type="button"
            aria-pressed={view === "table"}
            aria-label="Ver em tabela"
            title="Ver em tabela"
            onClick={() => setView("table")}
            className={cn(
              "inline-flex h-7 items-center rounded-md px-2",
              view === "table"
                ? "bg-zinc-800 text-zinc-100"
                : "text-zinc-500 hover:text-zinc-200",
            )}
          >
            <LayoutList size={13} />
          </button>
        </div>
        <div className="flex items-center gap-2 text-[13.5px] text-zinc-500">
          <Badge variant="default">
            {grouped.length} de {threadCount} execuções
          </Badge>
          <span className="hidden items-center gap-1 text-emerald-300/80 sm:inline-flex">
            {summary.success} sucesso
          </span>
          <span className="hidden items-center gap-1 text-red-300/80 sm:inline-flex">
            {summary.failed} falhas
          </span>
          {summary.running > 0 ? (
            <span className="inline-flex items-center gap-1 text-blue-300/80">
              {summary.running} em andamento
            </span>
          ) : null}
        </div>
      </FilterBar>

      <Card>
        <CardContent className="p-0">
          {loading && runs.length === 0 ? (
            <LoadingState compact label="Carregando execuções…" rows={4} />
          ) : ordered.length === 0 ? (
            <EmptyState
              icon={Activity}
              title={runs.length === 0 ? "Nenhuma execução ainda" : "Nada encontrado"}
              description={
                runs.length === 0
                  ? "Execute uma tarefa ou rotina para ver o histórico aqui."
                  : "Ajuste a busca ou o filtro de status."
              }
              compact
              className="border-0 bg-transparent"
            />
          ) : view === "table" ? (
            <div className="scroll-area overflow-x-auto">
              <table className="w-full min-w-[900px] text-left text-[13px]">
                <thead className="border-b border-zinc-800/70 text-[11.5px] uppercase tracking-wide text-zinc-500">
                  <tr>
                    <th className="px-4 py-2">Execução</th>
                    <th className="px-3 py-2">Status</th>
                    <th className="px-3 py-2">Origem</th>
                    <th className="px-3 py-2">Agente</th>
                    <th className="px-3 py-2 text-right">Duração</th>
                    <th className="px-3 py-2 text-right">Tokens</th>
                    <th className="px-3 py-2 text-right">Custo</th>
                    <th className="px-4 py-2 text-right">Quando</th>
                  </tr>
                </thead>
                <tbody>
                  {ordered.map(
                    ({ latest: run, attempts, tokensInput, tokensOutput, cost, duration }) => (
                      <tr
                        key={run.task_id ?? run.id}
                        className="cursor-pointer border-b border-zinc-800/50 transition-colors hover:bg-zinc-900/40"
                        onClick={() => navigate(links.run(run.id))}
                      >
                        <td className="max-w-[320px] px-4 py-2">
                          <Link
                            to={links.run(run.id)}
                            className="block truncate font-medium text-zinc-100 hover:text-blue-200"
                            onClick={(event) => event.stopPropagation()}
                          >
                            {run.task_title ?? run.agent_name}
                          </Link>
                          <span className="font-mono text-[11.5px] text-zinc-500">
                            {run.phase ?? "run"}
                            {attempts.length > 1
                              ? ` · ${attempts.length} tentativas`
                              : ""}
                          </span>
                        </td>
                        <td className="px-3 py-2">
                          <Badge variant={runStatusTone(run.status)}>
                            {runStatusLabel(run.status)}
                          </Badge>
                        </td>
                        <td className="px-3 py-2">
                          <Badge variant={runScope(run) === "work" ? "default" : "purple"}>
                            {RUN_SCOPE_LABEL[runScope(run)]}
                          </Badge>
                        </td>
                        <td className="px-3 py-2">
                          <span className="block font-mono text-[12px] text-zinc-300">
                            @{run.agent_name}
                          </span>
                          <span className="block truncate font-mono text-[11.5px] text-zinc-500">
                            {run.model_id}
                          </span>
                        </td>
                        <td className="px-3 py-2 text-right font-mono text-zinc-400">
                          {formatDuration(duration)}
                        </td>
                        <td className="px-3 py-2 text-right font-mono text-zinc-400">
                          {formatNumber(tokensInput + tokensOutput)}
                        </td>
                        <td className="px-3 py-2 text-right font-mono text-zinc-300">
                          {formatUsd(cost)}
                        </td>
                        <td className="px-4 py-2 text-right font-mono text-[11.5px] text-zinc-500">
                          {relativeTime(run.started_at)}
                        </td>
                      </tr>
                    ),
                  )}
                </tbody>
              </table>
            </div>
          ) : (
            <div className="divide-y divide-zinc-800/60">
              {ordered.map(({ latest: run, attempts, tokensInput, tokensOutput, cost, duration }) => (
                <Link
                  key={run.task_id ?? run.id}
                  to={links.run(run.id)}
                  className="flex flex-wrap items-center gap-3 px-4 py-3 transition-colors hover:bg-zinc-900/40"
                >
                  <RunStatusIcon status={run.status} size={15} />
                  <div className="min-w-0 flex-1">
                    <div className="flex flex-wrap items-center gap-2">
                      <p className="truncate text-[14px] font-medium text-zinc-100">
                        {run.task_title ?? run.agent_name}
                      </p>
                      <Badge variant={runStatusTone(run.status)}>
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
                      {runScope(run) !== "work" ? (
                        <Badge variant="purple">{RUN_SCOPE_LABEL[runScope(run)]}</Badge>
                      ) : null}
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
                      <p className="mt-1 truncate font-mono text-[13px] text-red-300/80">
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
          {ordered.length >= limit && runs.length >= limit ? (
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

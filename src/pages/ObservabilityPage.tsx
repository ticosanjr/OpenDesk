import type { ReactNode } from "react";
import { useCallback, useEffect, useMemo, useState } from "react";
import {
  Activity,
  AlertTriangle,
  Clock3,
  Coins,
  Gauge,
  RefreshCw,
  Timer,
  TrendingUp,
} from "lucide-react";
import { PageBody, PageHeader } from "@/components/layout/PageHeader";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/ui/empty-state";
import { LoadingState } from "@/components/ui/loading-state";
import { apiGet } from "@/lib/api";
import { formatNumber, formatPercent, formatUsd } from "@/lib/format";
import {
  barPercent,
  dailyMax,
  failureLabel,
  fillDailySeries,
  formatMs,
  successTone,
} from "@/lib/metrics";
import type { Metrics } from "@/lib/types";
import { useAppStore } from "@/store/app-store";
import { useUiStore } from "@/store/ui-store";
import { cn } from "@/lib/utils";

const RANGES = [7, 30, 90] as const;

export function ObservabilityPage() {
  const activeWorkspaceId = useAppStore((state) => state.activeWorkspaceId);
  const workspaces = useAppStore((state) => state.workspaces);
  const setWorkspaceDialogOpen = useUiStore(
    (state) => state.setWorkspaceDialogOpen,
  );
  const workspace = workspaces.find((item) => item.id === activeWorkspaceId);
  const [days, setDays] = useState<(typeof RANGES)[number]>(30);
  const [metrics, setMetrics] = useState<Metrics | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const query = useMemo(
    () =>
      activeWorkspaceId
        ? `?workspace_id=${encodeURIComponent(activeWorkspaceId)}&days=${days}`
        : "",
    [activeWorkspaceId, days],
  );

  const load = useCallback(async () => {
    if (!activeWorkspaceId) {
      setMetrics(null);
      return;
    }
    setLoading(true);
    try {
      setMetrics(await apiGet<Metrics>(`/api/metrics${query}`));
      setError(null);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setLoading(false);
    }
  }, [activeWorkspaceId, query]);

  useEffect(() => {
    void load();
  }, [load]);

  if (!activeWorkspaceId) {
    return (
      <PageBody>
        <PageHeader title="Observabilidade" />
        <EmptyState
          icon={Activity}
          title="Nenhum workspace ativo"
          description="Selecione um workspace para ver latência, fila e falhas."
          action={
            <Button size="sm" onClick={() => setWorkspaceDialogOpen(true)}>
              Criar workspace
            </Button>
          }
        />
      </PageBody>
    );
  }

  const daily = metrics ? fillDailySeries(metrics.daily, days) : [];
  const maxDaily = metrics ? dailyMax(daily) : 1;

  return (
    <PageBody>
      <PageHeader
        title="Observabilidade"
        description={`${workspace?.name ?? "Workspace"} · sucesso, latência, espera na fila e falhas dos últimos ${days} dias.`}
        actions={
          <>
            <div className="flex items-center gap-1 rounded-lg border border-zinc-800 bg-zinc-950/40 p-0.5">
              {RANGES.map((range) => (
                <button
                  key={range}
                  type="button"
                  onClick={() => setDays(range)}
                  aria-pressed={days === range}
                  className={cn(
                    "rounded-md px-2 py-1 text-[10px] transition-colors",
                    days === range
                      ? "workspace-tab-active"
                      : "text-zinc-500 hover:text-zinc-200",
                  )}
                >
                  {range}d
                </button>
              ))}
            </div>
            <Button variant="outline" size="sm" onClick={() => void load()} disabled={loading}>
              <RefreshCw size={13} className={loading ? "animate-spin" : undefined} />
              Atualizar
            </Button>
          </>
        }
      />

      {error ? (
        <p className="rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[11px] text-red-300">
          {error}
        </p>
      ) : null}

      {loading && !metrics ? (
        <LoadingState rows={3} label="Calculando métricas…" />
      ) : !metrics || metrics.runs_total === 0 ? (
        <EmptyState
          icon={Activity}
          title="Sem execuções no período"
          description="As métricas aparecem quando houver runs. Ajuste o período ou rode uma tarefa."
        />
      ) : (
        <>
          <div className="grid grid-cols-2 gap-2 xl:grid-cols-4">
            <MetricCard
              icon={<TrendingUp size={13} />}
              label="Taxa de sucesso"
              value={formatPercent(metrics.success_rate * 100)}
              hint={`${metrics.runs_success} de ${metrics.runs_total} ${
                metrics.runs_total === 1 ? "run" : "runs"
              }`}
              tone={successTone(metrics.success_rate)}
            />
            <MetricCard
              icon={<Timer size={13} />}
              label="Duração p95"
              value={formatMs(metrics.p95_duration_ms)}
              hint={`p50 ${formatMs(metrics.p50_duration_ms)} · média ${formatMs(metrics.avg_duration_ms)}`}
            />
            <MetricCard
              icon={<Clock3 size={13} />}
              label="Espera na fila p95"
              value={formatMs(metrics.queue_p95_wait_ms)}
              hint={`${metrics.queue_samples} jobs · média ${formatMs(metrics.queue_avg_wait_ms)}`}
            />
            <MetricCard
              icon={<Coins size={13} />}
              label="Custo"
              value={formatUsd(metrics.cost_usd)}
              hint={`${formatNumber(metrics.tokens)} tokens`}
            />
          </div>

          <div className="grid gap-3 lg:grid-cols-[minmax(0,1.6fr)_minmax(0,1fr)]">
            <Card>
              <CardHeader className="py-3">
                <CardTitle className="flex items-center gap-2 text-[13px]">
                  <Gauge size={14} className="text-zinc-400" />
                  Execuções por dia
                </CardTitle>
              </CardHeader>
              <CardContent className="p-4 pt-0">
                <div className="flex h-40 items-end gap-1">
                  {daily.map((day, index) => (
                    <div
                      key={day.date}
                      className="group flex h-full min-w-0 flex-1 flex-col items-center justify-end gap-1"
                      title={`${day.date}: ${day.runs} execuções, ${day.failed} falhas`}
                    >
                      <div className="relative flex w-full min-w-0 flex-1 items-end">
                        <div
                          className="w-full rounded-t bg-sky-500/30 transition-colors group-hover:bg-sky-500/50"
                          style={{ height: `${barPercent(day.runs, maxDaily)}%` }}
                        />
                        <div
                          className="absolute bottom-0 w-full rounded-t bg-red-400/70"
                          style={{ height: `${barPercent(day.failed, maxDaily)}%` }}
                        />
                      </div>
                      <span
                        className={cn(
                          "hidden whitespace-nowrap text-[10px] text-zinc-500 sm:block",
                          index % 5 !== 0 && index !== daily.length - 1 && "opacity-0",
                        )}
                      >
                        {day.date.slice(5)}
                      </span>
                    </div>
                  ))}
                </div>
                <div className="mt-2 flex items-center gap-3 text-[11px] text-zinc-500">
                  <span className="inline-flex items-center gap-1">
                    <span className="h-2 w-2 rounded-sm bg-sky-500/40" />
                    execuções
                  </span>
                  <span className="inline-flex items-center gap-1">
                    <span className="h-2 w-2 rounded-sm bg-red-400/70" />
                    falhas
                  </span>
                </div>
              </CardContent>
            </Card>

            <Card>
              <CardHeader className="py-3">
                <CardTitle className="flex items-center gap-2 text-[13px]">
                  <AlertTriangle size={14} className="text-amber-300" />
                  Falhas por código
                </CardTitle>
              </CardHeader>
              <CardContent className="space-y-1.5 p-4 pt-0">
                {metrics.failure_codes.length === 0 ? (
                  <p className="text-[12px] text-zinc-400">Nenhuma falha no período.</p>
                ) : (
                  metrics.failure_codes.map((failure) => (
                    <div
                      key={failure.code}
                      className="flex items-center justify-between gap-2 rounded-lg border border-zinc-800/60 bg-zinc-950/30 px-2.5 py-1.5"
                    >
                      <span className="truncate text-[12px] text-zinc-300">
                        {failureLabel(failure.code)}
                      </span>
                      <Badge variant="red">{failure.count}</Badge>
                    </div>
                  ))
                )}
              </CardContent>
            </Card>
          </div>

          <Card>
            <CardHeader className="py-3">
              <CardTitle className="flex items-center gap-2 text-[13px]">
                <Activity size={14} className="text-zinc-400" />
                Performance por agente
              </CardTitle>
            </CardHeader>
            <CardContent className="p-0">
              <div className="overflow-x-auto">
                <table className="w-full text-left text-[10px]">
                  <thead className="border-b border-zinc-800 text-[11px] uppercase tracking-[0.06em] text-zinc-400">
                    <tr>
                      <th className="px-4 py-2">Agente</th>
                      <th className="px-4 py-2">Execuções</th>
                      <th className="px-4 py-2">Sucesso</th>
                      <th className="px-4 py-2">Duração p95</th>
                      <th className="px-4 py-2">Custo</th>
                    </tr>
                  </thead>
                  <tbody>
                    {metrics.agents.map((agent) => (
                      <tr
                        key={agent.agent_id}
                        className="border-b border-zinc-800/50 last:border-0"
                      >
                        <td className="px-4 py-2 font-mono text-zinc-300">
                          @{agent.agent_id}
                        </td>
                        <td className="px-4 py-2 text-zinc-400">{agent.runs}</td>
                        <td className="px-4 py-2">
                          <Badge variant={successTone(agent.success_rate)}>
                            {formatPercent(agent.success_rate * 100)}
                          </Badge>
                        </td>
                        <td className="px-4 py-2 text-zinc-400">
                          {formatMs(agent.p95_duration_ms)}
                        </td>
                        <td className="px-4 py-2 text-zinc-400">
                          {formatUsd(agent.cost_usd)}
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </CardContent>
          </Card>
        </>
      )}
    </PageBody>
  );
}

function MetricCard({
  icon,
  label,
  value,
  hint,
  tone = "default",
}: {
  icon: ReactNode;
  label: string;
  value: string;
  hint: string;
  tone?: "default" | "green" | "yellow" | "red";
}) {
  const toneLabel = { green: "saudável", yellow: "atenção", red: "crítico" } as const;
  return (
    <Card>
      <CardContent className="p-3">
        <div className="flex items-center gap-1.5 text-zinc-500">
          {icon}
          <span className="text-[10px] uppercase tracking-[0.12em]">{label}</span>
          {tone !== "default" ? (
            <Badge variant={tone}>{toneLabel[tone]}</Badge>
          ) : null}
        </div>
        <p className="mt-1 font-mono text-lg text-zinc-100">{value}</p>
        <p className="text-[10px] text-zinc-500">{hint}</p>
      </CardContent>
    </Card>
  );
}

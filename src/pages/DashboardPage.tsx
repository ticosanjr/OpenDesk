import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import {
  Activity,
  Archive,
  Bot,
  CircleDollarSign,
  Clock3,
  Cog,
  Cpu,
  Database,
  FileText,
  FolderPlus,
  Gauge,
  RefreshCw,
  RotateCw,
  Server,
  Sparkles,
  TerminalSquare,
} from "lucide-react";
import { PageBody, PageHeader } from "@/components/layout/PageHeader";
import { InfoRow } from "@/components/ui/info-row";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { EmptyState } from "@/components/ui/empty-state";
import { StatusDot, statusTone } from "@/components/ui/status-dot";
import { WorkingIndicator } from "@/components/ui/working-indicator";
import { AttentionCard } from "@/components/dashboard/AttentionCard";
import { CostMixCard } from "@/components/dashboard/CostMixCard";
import { useAppStore } from "@/store/app-store";
import { useUiStore } from "@/store/ui-store";
import { apiGet } from "@/lib/api";
import type { AgentFile, AgentInfo, Artifact, TaskRun } from "@/lib/types";
import { ROUTER_POLICY_LABELS } from "@/lib/types";
import {
  formatNumber,
  formatPercent,
  formatUsd,
  relativeTime,
  shortPath,
} from "@/lib/format";
import type { LucideIcon } from "lucide-react";
import { cn } from "@/lib/utils";
import { links } from "@/lib/links";
import { EntityLink } from "@/components/ui/entity-link";

const FALLBACK_DAILY_LIMIT = 5;

export function DashboardPage() {
  const daemon = useAppStore((state) => state.daemon);
  const dashboard = useAppStore((state) => state.dashboard);
  const budget = useAppStore((state) => state.budget);
  const workspaces = useAppStore((state) => state.workspaces);
  const activeWorkspaceId = useAppStore((state) => state.activeWorkspaceId);
  const refresh = useAppStore((state) => state.refresh);
  const restartDaemon = useAppStore((state) => state.restartDaemon);
  const setWorkspaceDialogOpen = useUiStore(
    (state) => state.setWorkspaceDialogOpen,
  );
  const setTaskDialogOpen = useUiStore((state) => state.setTaskDialogOpen);

  const activeWorkspace = workspaces.find(
    (workspace) => workspace.id === activeWorkspaceId,
  );
  const [agents, setAgents] = useState<AgentInfo[]>([]);
  const [agentStates, setAgentStates] = useState<Record<string, string>>({});
  const [presentation, setPresentation] = useState<Record<string, AgentFile>>({});
  const [agentsError, setAgentsError] = useState<string | null>(null);
  const [recentRuns, setRecentRuns] = useState<TaskRun[]>([]);
  const [recentArtifacts, setRecentArtifacts] = useState<Artifact[]>([]);
  const [pinnedArtifacts, setPinnedArtifacts] = useState<Artifact[]>([]);
  const lastRunEvent = useAppStore((state) => state.lastRunEvent);
  const lastArtifactEvent = useAppStore((state) => state.lastArtifactEvent);
  const lastPermissionEvent = useAppStore((state) => state.lastPermissionEvent);
  const navigate = useNavigate();

  useEffect(() => {
    if (!activeWorkspaceId) {
      setAgentStates({});
      return;
    }
    void apiGet<{ agents: Array<{ agent: string; state: string }> }>(
      `/api/agents/states?workspace_id=${encodeURIComponent(activeWorkspaceId)}`,
    )
      .then((result) => {
        setAgentStates(
          Object.fromEntries(result.agents.map((item) => [item.agent, item.state])),
        );
      })
      .catch(() => setAgentStates({}));
  }, [
    activeWorkspaceId,
    lastRunEvent?.run_id,
    lastRunEvent?.status,
    lastPermissionEvent?.request_id,
  ]);

  useEffect(() => {
    if (!activeWorkspace) {
      setAgents([]);
      return;
    }

    let cancelled = false;
    setAgentsError(null);

    const directory = encodeURIComponent(activeWorkspace.root_path);
    const studioQuery = `scope=workspace&workspace_id=${encodeURIComponent(
      activeWorkspace.id,
    )}`;

    const runtimeAgents = daemon?.online
      ? apiGet<{ data: AgentInfo[] }>(`/api/agents?directory=${directory}`).catch(
          () => ({ data: [] }),
        )
      : Promise.resolve({ data: [] });

    Promise.all([
      runtimeAgents,
      apiGet<AgentFile[]>(`/api/studio/agents?${studioQuery}`).catch(() => []),
    ])
      .then(([response, studioAgents]) => {
        if (cancelled) return;
        const presentationMap = Object.fromEntries(
          studioAgents.map((agent) => [agent.id, agent]),
        );
        setPresentation(presentationMap);
        if (response.data.length > 0) {
          setAgents(response.data);
        } else {
          setAgents(
            studioAgents.map((agent) => {
              const [provider, rest] = (agent.model ?? "").split("/");
              const [id, variant] = (rest ?? "").split("#");
              return {
                id: agent.id,
                name: agent.name,
                description: agent.description,
                mode: agent.mode,
                model:
                  provider && id
                    ? { providerID: provider, id, variant: variant || null }
                    : null,
              };
            }),
          );
        }
      })
      .catch((error: unknown) => {
        if (!cancelled) {
          setAgentsError(error instanceof Error ? error.message : String(error));
        }
      });

    return () => {
      cancelled = true;
    };
  }, [activeWorkspace, daemon?.online]);

  useEffect(() => {
    if (!activeWorkspace) return;
    let cancelled = false;
    const query = encodeURIComponent(activeWorkspace.id);
    Promise.all([
      apiGet<TaskRun[]>(`/api/runs?workspace_id=${query}`).catch(() => []),
      apiGet<Artifact[]>(`/api/artifacts?workspace_id=${query}&limit=8`).catch(() => []),
      apiGet<Artifact[]>(`/api/artifacts/pinned?workspace_id=${query}`).catch(() => []),
    ]).then(([runs, artifacts, pinned]) => {
      if (!cancelled) {
        setRecentRuns(runs);
        setRecentArtifacts(artifacts);
        setPinnedArtifacts(pinned);
      }
    });
    return () => {
      cancelled = true;
    };
  }, [activeWorkspace, lastRunEvent?.run_id, lastArtifactEvent]);

  if (!activeWorkspace) {
    return (
      <PageBody>
        <PageHeader
          title="Dashboard"
          description="Registre um workspace para começar a governar agentes, runs, artefatos e custos."
          actions={
            <Button size="sm" onClick={() => setWorkspaceDialogOpen(true)}>
              <FolderPlus size={15} />
              Novo workspace
            </Button>
          }
        />
        <EmptyState
          icon={FolderPlus}
          title="Nenhum workspace registrado"
          description="O OpenDesk guarda a configuração do OpenCode nos arquivos nativos. O workspace só precisa apontar para o diretório do projeto."
          action={
            <Button size="sm" onClick={() => setWorkspaceDialogOpen(true)}>
              <FolderPlus size={15} />
              Criar primeiro workspace
            </Button>
          }
        />
      </PageBody>
    );
  }

  const runsTotal = dashboard?.runs_total ?? 0;
  const successRate =
    runsTotal > 0 ? ((dashboard?.runs_success ?? 0) / runsTotal) * 100 : 0;
  const cacheBase =
    (dashboard?.tokens_cache_read ?? 0) + (dashboard?.tokens_input ?? 0);
  const cacheHit =
    cacheBase > 0 ? ((dashboard?.tokens_cache_read ?? 0) / cacheBase) * 100 : 0;
  const spent = dashboard?.daily_cost_usd ?? 0;
  const dailyLimit = budget?.daily_limit ?? FALLBACK_DAILY_LIMIT;
  const totalSpent = dashboard?.estimated_cost_usd ?? 0;

  const modelUsage = Object.values(
    recentRuns.reduce<Record<string, { model: string; calls: number; tokens: number; cost: number; duration: number }>>(
      (accumulator, run) => {
        const key = run.model_id || "unknown";
        const entry = accumulator[key] ?? {
          model: key,
          calls: 0,
          tokens: 0,
          cost: 0,
          duration: 0,
        };
        entry.calls += 1;
        entry.tokens += run.tokens_input + run.tokens_output;
        entry.cost += run.estimated_cost_usd;
        entry.duration += run.duration_ms;
        accumulator[key] = entry;
        return accumulator;
      },
      {},
    ),
  ).sort((a, b) => b.cost - a.cost);

  const activities = [
    ...recentRuns.map((run) => ({
      kind: "run" as const,
      at: run.completed_at ?? run.started_at,
      run,
    })),
    ...recentArtifacts.map((artifact) => ({
      kind: "artifact" as const,
      at: artifact.created_at,
      artifact,
    })),
  ]
    .sort((left, right) => right.at.localeCompare(left.at))
    .slice(0, 8);

  const runningRuns = recentRuns.filter((run) => run.status === "RUNNING");

  return (
    <PageBody>
      <PageHeader
        title="Dashboard"
        description={`${activeWorkspace.name} · ${shortPath(activeWorkspace.root_path, 64)}`}
        actions={
          <>
            <Button variant="outline" size="sm" onClick={() => void refresh()}>
              <RefreshCw size={14} />
              Atualizar
            </Button>
            <Button variant="outline" size="sm" onClick={() => void restartDaemon()}>
              <RotateCw size={14} />
              Reiniciar engine
            </Button>
          </>
        }
      />

      <AttentionCard />

      {runningRuns.length > 0 ? (
        <Card className="border-blue-500/25 bg-blue-500/[0.03]">
          <CardContent className="flex flex-wrap items-center gap-3 p-3">
            <WorkingIndicator label={`${runningRuns.length} em execução`} />
            <div className="flex min-w-0 flex-1 flex-wrap gap-2">
              {runningRuns.slice(0, 4).map((run) => (
                <button
                  key={run.id}
                  type="button"
                  onClick={() => navigate(links.run(run.id))}
                  className="min-w-0 rounded-lg border border-blue-500/20 bg-zinc-950/40 px-2.5 py-1.5 text-left transition-colors hover:border-blue-500/40"
                  title={`Abrir execução de ${run.task_title ?? run.agent_name}`}
                >
                  <p className="max-w-[260px] truncate text-[13px] text-zinc-200">
                    {run.task_title ?? run.agent_name}
                  </p>
                  <p className="font-mono text-[11.5px] text-zinc-500">
                    @{run.agent_name} · {run.model_id} ·{" "}
                    {relativeTime(run.started_at)}
                  </p>
                </button>
              ))}
            </div>
            <EntityLink to="/runs" className="text-[12px] text-zinc-400">
              ver todas
            </EntityLink>
          </CardContent>
        </Card>
      ) : null}

      <Card className="overflow-hidden">
        <div className="grid gap-px bg-zinc-800/50 sm:grid-cols-2 xl:grid-cols-4">
        <KpiCard
          icon={CircleDollarSign}
          label="Gasto hoje"
          value={formatUsd(spent)}
          hint={`limite diário ${formatUsd(dailyLimit)} · ${formatPercent(
            Math.min(100, (spent / Math.max(dailyLimit, 0.01)) * 100),
          )} usado · total ${formatUsd(totalSpent)}`}
          tone={spent / Math.max(dailyLimit, 0.01) >= 0.8 ? "warning" : "default"}
        />
        <KpiCard
          icon={Activity}
          label="Execuções & sucesso"
          value={`${formatNumber(runsTotal)} execuções`}
          hint={`${formatPercent(successRate)} sucesso · ${
            dashboard?.runs_failed ?? 0
          } falhas`}
        />
        <KpiCard
          icon={Database}
          label="Cache & tokens"
          value={`${formatPercent(cacheHit)} acertos`}
          hint={`${formatNumber(dashboard?.tokens_input)} entrada · ${formatNumber(
            dashboard?.tokens_output,
          )} saída`}
        />
        <KpiCard
          icon={Clock3}
          label="Fila"
          value={`${dashboard?.queue_running ?? 0} rodando`}
          hint={`${dashboard?.queue_waiting ?? 0} aguardando · ${
            dashboard?.artifacts_total ?? 0
          } artefatos`}
          active={(dashboard?.queue_running ?? 0) > 0}
        />
        </div>
      </Card>

      <div className="flex flex-wrap items-center gap-2 px-1 text-[12px] text-zinc-500">
        <Badge
          variant={
            (dashboard?.active_routines ?? 0) + (dashboard?.enabled_recipes ?? 0) > 0
              ? "yellow"
              : "default"
          }
        >
          Automações
        </Badge>
        <EntityLink to="/routines">
          {dashboard?.active_routines ?? 0} rotinas ativas
        </EntityLink>
        <span>·</span>
        <EntityLink to="/recipes">
          {dashboard?.enabled_recipes ?? 0} receitas ativas
        </EntityLink>
        <span className="text-zinc-400">
          Revise gatilhos e custos antes de deixar rodando.
        </span>
      </div>

      <CostMixCard />

      {pinnedArtifacts.length > 0 ? (
        <Card className="border-amber-500/20 bg-amber-500/[0.02]">
          <CardHeader className="py-3">
            <CardTitle className="flex items-center gap-2 text-[14px]">
              <Archive size={15} className="text-amber-300" />
              Entregáveis oficiais
            </CardTitle>
            <Badge variant="yellow">{pinnedArtifacts.length} fixados</Badge>
          </CardHeader>
          <CardContent className="flex flex-wrap gap-2 pt-0">
            {pinnedArtifacts.slice(0, 8).map((artifact) => (
              <button
                key={artifact.id}
                type="button"
                onClick={() => navigate(links.artifact(artifact.id))}
                className="flex max-w-full items-center gap-1.5 rounded-lg border border-amber-500/20 bg-zinc-950/40 px-2.5 py-1.5 text-left text-[13px] text-zinc-300 hover:border-amber-500/40 hover:text-zinc-100"
                title={artifact.file_path}
              >
                <FileText size={12} className="shrink-0 text-amber-300" />
                <span className="max-w-56 truncate">{artifact.file_path}</span>
              </button>
            ))}
          </CardContent>
        </Card>
      ) : null}

      <div className="grid gap-3 lg:grid-cols-3">
        <Card className="lg:col-span-2">
          <CardHeader>
            <div>
              <CardTitle className="flex items-center gap-2">
                <Bot size={15} className="text-zinc-500" />
                Agentes ativos
              </CardTitle>
              <CardDescription>
                Lidos em tempo real de <span className="font-mono">/api/agent</span>{" "}
                com o diretório do workspace.
              </CardDescription>
            </div>
            <Badge variant="blue">{agents.length} encontrados</Badge>
          </CardHeader>
          <CardContent>
            {agentsError ? (
              <EmptyState
                icon={TerminalSquare}
                title="Não foi possível listar agentes"
                description={agentsError}
                compact
              />
            ) : agents.length === 0 ? (
              <EmptyState
                icon={Bot}
                title="Nenhum agente configurado"
                description="Crie um agente no OpenCode Studio ou adicione um arquivo em .opencode/agents/."
                compact
                action={
                  <Button size="sm" variant="outline" onClick={() => navigate("/studio")}>
                    Abrir Studio
                  </Button>
                }
              />
            ) : (
              <div className="grid gap-2 md:grid-cols-2">
                {agents.slice(0, 6).map((agent) => {
                  const custom = presentation[agent.id];
                  const displayName = custom?.name || agent.name || agent.id;
                  const color = custom?.color || agent.color || "#60a5fa";
                  const avatar =
                    custom?.avatar ||
                    Array.from(displayName).slice(0, 2).join("") ||
                    "🤖";
                  return (
                    <div
                      key={agent.id}
                      className="rounded-lg border border-zinc-800/70 bg-zinc-950/40 p-3"
                    >
                      <div className="flex items-start justify-between gap-2">
                        <div className="flex min-w-0 items-center gap-2.5">
                          <span
                            className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg border text-[13px]"
                            style={{
                              background: `${color}22`,
                              borderColor: `${color}44`,
                              color,
                            }}
                          >
                            {Array.from(avatar).slice(0, 2).join("")}
                          </span>
                          <div className="min-w-0">
                            <p className="truncate text-[13.5px] font-medium text-zinc-200">
                              {displayName}
                            </p>
                            <p className="truncate font-mono text-[11.5px] text-zinc-500">
                              @{agent.id}
                            </p>
                          </div>
                        </div>
                        <div className="flex flex-col items-end gap-1">
                          <Badge variant={agent.mode === "primary" ? "green" : "purple"}>
                            {agent.mode ?? "agent"}
                          </Badge>
                          <AgentStateBadge state={agentStates[agent.id]} />
                        </div>
                      </div>
                      <p className="mt-2 truncate text-[12px] text-zinc-500">
                        {custom?.description || agent.description || "sem descrição"}
                      </p>
                      <p className="mt-1 truncate font-mono text-[11.5px] text-zinc-500">
                        {agent.model
                          ? `${agent.model.providerID}/${agent.model.id}`
                          : "modelo herdado da sessão"}
                      </p>
                    </div>
                  );
                })}
              </div>
            )}
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <div>
              <CardTitle className="flex items-center gap-2">
                <Server size={15} className="text-zinc-500" />
                OpenCode engine
              </CardTitle>
              <CardDescription>Daemon gerenciado pelo OpenDesk.</CardDescription>
            </div>
            <StatusDot
              tone={statusTone(daemon?.online ?? false, daemon?.state)}
            />
          </CardHeader>
          <CardContent className="space-y-3">
            <InfoRow
              label="Estado"
              value={daemon?.online ? "online" : daemon?.state ?? "offline"}
            />
            <InfoRow label="Porta" value={daemon ? String(daemon.port) : "—"} />
            <InfoRow
              label="Versão"
              value={daemon?.version ? `v${daemon.version}` : "—"}
            />
            <InfoRow
              label="Jobs ativos"
              value={String(daemon?.active_jobs ?? 0)}
            />
            {daemon?.message ? (
              <p className="rounded-lg border border-amber-500/20 bg-amber-500/10 px-2.5 py-2 text-[12px] leading-relaxed text-amber-300">
                {daemon.message}
              </p>
            ) : null}
          </CardContent>
        </Card>
      </div>

      <div className="grid gap-3 lg:grid-cols-2">
        <Card>
          <CardHeader>
            <div>
              <CardTitle className="flex items-center gap-2">
                <Cpu size={15} className="text-zinc-500" />
                Uso por modelo
              </CardTitle>
              <CardDescription>
                Chamadas, tokens, custo e latência média por modelo nos runs recentes.
              </CardDescription>
            </div>
          </CardHeader>
          <CardContent>
            {modelUsage.length === 0 ? (
              <EmptyState
                icon={Cpu}
                title="Sem telemetria por modelo ainda"
                description="A tabela é alimentada pelos runs persistidos em task_runs."
                compact
              />
            ) : (
              <div className="scroll-area overflow-auto">
                <table className="w-full text-left">
                  <thead className="border-b border-zinc-800/70 text-[11.5px] uppercase tracking-[0.12em] text-zinc-500">
                    <tr>
                      <th className="pb-2 font-medium">Modelo</th>
                      <th className="pb-2 text-right font-medium">Chamadas</th>
                      <th className="pb-2 text-right font-medium">Tokens</th>
                      <th className="pb-2 text-right font-medium">Custo</th>
                      <th className="pb-2 text-right font-medium">Latência média</th>
                    </tr>
                  </thead>
                  <tbody>
                    {modelUsage.map((model) => (
                      <tr key={model.model} className="border-b border-zinc-800/40 text-[12px]">
                        <td className="max-w-[220px] truncate py-2 font-mono text-zinc-300">
                          {model.model}
                        </td>
                        <td className="py-2 text-right font-mono text-zinc-500">{model.calls}</td>
                        <td className="py-2 text-right font-mono text-zinc-500">
                          {formatNumber(model.tokens)}
                        </td>
                        <td className="py-2 text-right font-mono text-zinc-400">
                          {formatUsd(model.cost)}
                        </td>
                        <td className="py-2 text-right font-mono text-zinc-500">
                          {Math.round(model.duration / Math.max(1, model.calls))} ms
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <div>
              <CardTitle className="flex items-center gap-2">
                <Sparkles size={15} className="text-zinc-500" />
                Atividades recentes
              </CardTitle>
              <CardDescription>
                Clique em um run ou artefato para abrir o detalhe.
              </CardDescription>
            </div>
            <Badge variant="default">{activities.length}</Badge>
          </CardHeader>
          <CardContent>
            {activities.length === 0 ? (
              <EmptyState
                icon={Archive}
                title="Nenhuma atividade ainda"
                description="Runs e artefatos aparecerão aqui em tempo real via WebSocket."
                compact
                action={
                  <Button size="sm" onClick={() => setTaskDialogOpen(true)}>
                    Criar tarefa
                  </Button>
                }
              />
            ) : (
              <div className="space-y-2">
                {activities.map((activity) =>
                  activity.kind === "artifact" ? (
                    <button
                      type="button"
                      key={`artifact-${activity.artifact.id}`}
                      onClick={() => navigate(`/artifacts?id=${activity.artifact.id}`)}
                      className="flex w-full items-center gap-3 rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2 text-left transition hover:border-emerald-500/30 hover:bg-emerald-500/[0.04]"
                    >
                      <Archive size={13} className="shrink-0 text-emerald-400/70" />
                      <div className="min-w-0 flex-1">
                        <p className="truncate font-mono text-[12px] text-zinc-300">
                          {activity.artifact.file_path}
                        </p>
                        <p className="truncate text-[11.5px] text-zinc-500">
                          @{activity.artifact.created_by_agent} ·{" "}
                          {activity.artifact.creation_reason}
                        </p>
                      </div>
                      <span className="shrink-0 font-mono text-[11px] text-zinc-500">
                        {relativeTime(activity.at)}
                      </span>
                    </button>
                  ) : (
                    <button
                      type="button"
                      key={`run-${activity.run.id}`}
                      onClick={() => navigate(links.run(activity.run.id))}
                      className={cn(
                        "flex w-full items-center gap-3 rounded-lg border bg-zinc-950/40 px-3 py-2 text-left transition hover:bg-zinc-900/60",
                        activity.run.status === "FAILED"
                          ? "border-red-500/40"
                          : activity.run.status === "SUCCESS"
                            ? "border-emerald-500/25"
                            : "border-zinc-800/70",
                      )}
                    >
                      {activity.run.status === "RUNNING" ? (
                        <Cog size={13} className="opendesk-gear shrink-0 text-blue-400" />
                      ) : (
                        <Cpu
                          size={13}
                          className={cn(
                            "shrink-0",
                            activity.run.status === "FAILED"
                              ? "text-red-400"
                              : activity.run.status === "SUCCESS"
                                ? "text-emerald-400/80"
                                : "text-blue-400/70",
                          )}
                        />
                      )}
                      <div className="min-w-0 flex-1">
                        <p className="truncate text-[12px] text-zinc-300">
                          {activity.run.task_title ?? activity.run.agent_name} ·{" "}
                          {activity.run.phase ?? activity.run.execution_mode}
                        </p>
                        <p
                          className={cn(
                            "truncate font-mono text-[11.5px]",
                            activity.run.status === "FAILED"
                              ? "text-red-300/80"
                              : "text-zinc-500",
                          )}
                        >
                          {activity.run.status} · {formatUsd(activity.run.estimated_cost_usd)} ·{" "}
                          {formatNumber(
                            activity.run.tokens_input + activity.run.tokens_output,
                          )}{" "}
                          tokens
                          {activity.run.error_message
                            ? ` · ${activity.run.error_message}`
                            : ""}
                        </p>
                      </div>
                      <span className="shrink-0 font-mono text-[11px] text-zinc-500">
                        {relativeTime(activity.at)}
                      </span>
                    </button>
                  ),
                )}
              </div>
            )}
          </CardContent>
        </Card>
      </div>

      <div className="flex flex-wrap items-center gap-2 rounded-xl border border-zinc-800/70 bg-zinc-900/20 px-4 py-3 text-[13px] text-zinc-500">
        <Gauge size={14} className="text-zinc-500" />
        <span className="font-medium text-zinc-300">Workspace</span>
        <EntityLink to={links.file("")} className="font-mono text-[12px] text-zinc-500">
          {activeWorkspace.root_path}
        </EntityLink>
        <Badge variant="default">
          {ROUTER_POLICY_LABELS[activeWorkspace.default_router_policy] ??
            activeWorkspace.default_router_policy}
        </Badge>
        <Badge variant="default">{activeWorkspace.timezone}</Badge>
        <span className="ml-auto font-mono text-[12px] text-zinc-500">
          aberto {relativeTime(activeWorkspace.last_opened_at)}
        </span>
      </div>
    </PageBody>
  );
}

function KpiCard({
  icon: Icon,
  label,
  value,
  hint,
  tone = "default",
  active = false,
}: {
  icon: LucideIcon;
  label: string;
  value: string;
  hint: string;
  tone?: "default" | "warning";
  active?: boolean;
}) {
  return (
    <div className="bg-zinc-950/25 p-4">
      <div className="flex items-center gap-2">
        <Icon size={15} strokeWidth={1.7} className="shrink-0 text-zinc-400" />
        <p className="min-w-0 flex-1 truncate text-[13px] font-medium uppercase tracking-[0.1em] text-zinc-400">
          {label}
        </p>
        {tone === "warning" ? <Badge variant="yellow">atenção</Badge> : null}
      </div>
      <p className="mt-2 font-mono text-[28px] font-medium leading-tight tracking-[-0.03em] text-zinc-50 [font-variant-numeric:tabular-nums]">
        {value}
      </p>
      <p className="mt-1 text-[13.5px] leading-relaxed text-zinc-400">{hint}</p>
      {active ? (
        <div className="mt-2">
          <WorkingIndicator label="em execução" />
        </div>
      ) : null}
    </div>
  );
}


function AgentStateBadge({ state }: { state?: string }) {
  const label = state ?? "IDLE";
  const tone =
    label === "WORKING"
      ? "blue"
      : label === "BLOCKED"
        ? "red"
        : label === "DONE"
          ? "green"
          : "default";
  return <Badge variant={tone}>{label.toLowerCase()}</Badge>;
}

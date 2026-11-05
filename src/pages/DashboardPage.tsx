import { useEffect, useState } from "react";
import {
  Activity,
  Archive,
  Bot,
  CircleDollarSign,
  Clock3,
  Cpu,
  Database,
  FolderPlus,
  Gauge,
  RefreshCw,
  RotateCw,
  Server,
  Sparkles,
  TerminalSquare,
} from "lucide-react";
import { PageBody, PageHeader } from "@/components/layout/PageHeader";
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
import { useAppStore } from "@/store/app-store";
import { useUiStore } from "@/store/ui-store";
import { apiGet } from "@/lib/api";
import type { AgentInfo } from "@/lib/types";
import { ROUTER_POLICY_LABELS } from "@/lib/types";
import {
  formatNumber,
  formatPercent,
  formatUsd,
  relativeTime,
  shortPath,
} from "@/lib/format";
import type { LucideIcon } from "lucide-react";

const DAILY_LIMIT = 5;

export function DashboardPage() {
  const daemon = useAppStore((state) => state.daemon);
  const dashboard = useAppStore((state) => state.dashboard);
  const workspaces = useAppStore((state) => state.workspaces);
  const activeWorkspaceId = useAppStore((state) => state.activeWorkspaceId);
  const refresh = useAppStore((state) => state.refresh);
  const restartDaemon = useAppStore((state) => state.restartDaemon);
  const setWorkspaceDialogOpen = useUiStore(
    (state) => state.setWorkspaceDialogOpen,
  );

  const activeWorkspace = workspaces.find(
    (workspace) => workspace.id === activeWorkspaceId,
  );
  const [agents, setAgents] = useState<AgentInfo[]>([]);
  const [agentsError, setAgentsError] = useState<string | null>(null);

  useEffect(() => {
    if (!activeWorkspace || !daemon?.online) {
      setAgents([]);
      return;
    }

    let cancelled = false;
    setAgentsError(null);
    apiGet<{ data: AgentInfo[] }>(
      `/api/agents?directory=${encodeURIComponent(activeWorkspace.root_path)}`,
    )
      .then((response) => {
        if (!cancelled) setAgents(response.data);
      })
      .catch((error: unknown) => {
        if (!cancelled) {
          setAgentsError(error instanceof Error ? error.message : String(error));
        }
      });

    return () => {
      cancelled = true;
    };
  }, [activeWorkspace?.root_path, daemon?.online]);

  if (!activeWorkspace) {
    return (
      <PageBody>
        <PageHeader
          eyebrow="visão executiva"
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
  const spent = dashboard?.estimated_cost_usd ?? 0;

  return (
    <PageBody>
      <PageHeader
        eyebrow="visão executiva"
        title="Dashboard"
        description={`${activeWorkspace.name} · ${shortPath(activeWorkspace.root_path, 64)}`}
        actions={
          <>
            <Button variant="outline" size="sm" onClick={() => void refresh()}>
              <RefreshCw size={14} />
              Atualizar
            </Button>
            <Button size="sm" onClick={() => void restartDaemon()}>
              <RotateCw size={14} />
              Reiniciar engine
            </Button>
          </>
        }
      />

      <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
        <KpiCard
          icon={CircleDollarSign}
          label="Gasto acumulado"
          value={formatUsd(spent)}
          hint={`limite diário ${formatUsd(DAILY_LIMIT)} · ${formatPercent(
            Math.min(100, (spent / DAILY_LIMIT) * 100),
          )} usado`}
          tone={spent / DAILY_LIMIT >= 0.8 ? "warning" : "default"}
        />
        <KpiCard
          icon={Activity}
          label="Runs & sucesso"
          value={`${formatNumber(runsTotal)} runs`}
          hint={`${formatPercent(successRate)} sucesso · ${
            dashboard?.runs_failed ?? 0
          } falhas`}
        />
        <KpiCard
          icon={Database}
          label="Cache & tokens"
          value={`${formatPercent(cacheHit)} cache hit`}
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
        />
      </div>

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
            {!daemon?.online ? (
              <EmptyState
                icon={Server}
                title="OpenCode engine offline"
                description="O dashboard carrega agentes quando o daemon local estiver saudável."
              />
            ) : agentsError ? (
              <EmptyState
                icon={TerminalSquare}
                title="Não foi possível listar agentes"
                description={agentsError}
              />
            ) : agents.length === 0 ? (
              <EmptyState
                icon={Bot}
                title="Nenhum agente configurado"
                description="Crie arquivos em .opencode/agents/ ou use o OpenCode Studio na Fase 2."
              />
            ) : (
              <div className="grid gap-2 md:grid-cols-2">
                {agents.slice(0, 6).map((agent) => (
                  <div
                    key={agent.id}
                    className="rounded-lg border border-zinc-800/70 bg-zinc-950/40 p-3"
                  >
                    <div className="flex items-start justify-between gap-2">
                      <div className="min-w-0">
                        <p className="truncate text-[12px] font-medium text-zinc-200">
                          @{agent.id}
                        </p>
                        <p className="mt-0.5 truncate text-[10px] text-zinc-600">
                          {agent.description ?? agent.name ?? "sem descrição"}
                        </p>
                      </div>
                      <Badge variant={agent.mode === "primary" ? "green" : "purple"}>
                        {agent.mode ?? "agent"}
                      </Badge>
                    </div>
                    <p className="mt-2 truncate font-mono text-[9px] text-zinc-600">
                      {agent.model
                        ? `${agent.model.providerID}/${agent.model.id}`
                        : "modelo herdado da sessão"}
                    </p>
                  </div>
                ))}
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
              <p className="rounded-lg border border-amber-500/20 bg-amber-500/10 px-2.5 py-2 text-[10px] leading-relaxed text-amber-300">
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
                Chamadas, tokens e custo por modelo. Chega com a telemetria da Fase 3.
              </CardDescription>
            </div>
            <Badge variant="default">em breve</Badge>
          </CardHeader>
          <CardContent>
            <EmptyState
              icon={Cpu}
              title="Sem telemetria por modelo ainda"
              description="A tabela é alimentada pelos eventos session.usage.recorded e pelos runs persistidos."
            />
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
                Tarefas, artefatos, e-mails e rotinas em uma linha do tempo.
              </CardDescription>
            </div>
            <Badge variant="default">em breve</Badge>
          </CardHeader>
          <CardContent>
            <EmptyState
              icon={Archive}
              title="Nenhuma atividade ainda"
              description="Quando a fila entrar em operação, runs e artefatos aparecerão aqui em tempo real via WebSocket."
            />
          </CardContent>
        </Card>
      </div>

      <div className="flex flex-wrap items-center gap-2 rounded-xl border border-zinc-800/70 bg-zinc-900/20 px-4 py-3 text-[11px] text-zinc-500">
        <Gauge size={14} className="text-zinc-600" />
        <span className="font-medium text-zinc-300">Workspace</span>
        <span className="font-mono text-[10px] text-zinc-600">
          {activeWorkspace.root_path}
        </span>
        <Badge variant="default">
          {ROUTER_POLICY_LABELS[activeWorkspace.default_router_policy] ??
            activeWorkspace.default_router_policy}
        </Badge>
        <Badge variant="default">{activeWorkspace.timezone}</Badge>
        <span className="ml-auto font-mono text-[10px] text-zinc-600">
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
}: {
  icon: LucideIcon;
  label: string;
  value: string;
  hint: string;
  tone?: "default" | "warning";
}) {
  return (
    <Card>
      <CardContent className="p-4">
        <div className="flex items-start justify-between">
          <div className="flex h-8 w-8 items-center justify-center rounded-lg border border-zinc-800 bg-zinc-900/60 text-zinc-500">
            <Icon size={15} strokeWidth={1.7} />
          </div>
          {tone === "warning" ? <Badge variant="yellow">atenção</Badge> : null}
        </div>
        <p className="mt-3 text-[10px] font-medium uppercase tracking-[0.14em] text-zinc-600">
          {label}
        </p>
        <p className="mt-1 font-mono text-xl font-medium tracking-tight text-zinc-100">
          {value}
        </p>
        <p className="mt-1.5 text-[10px] leading-relaxed text-zinc-600">{hint}</p>
      </CardContent>
    </Card>
  );
}

function InfoRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-center justify-between gap-3 text-[11px]">
      <span className="text-zinc-600">{label}</span>
      <span className="font-mono text-[10px] text-zinc-300">{value}</span>
    </div>
  );
}

import { useCallback, useEffect, useMemo, useState, type ReactNode } from "react";
import {
  Activity,
  ArrowRight,
  CheckCircle2,
  HeartPulse,
  Loader2,
  Network,
  Pencil,
  Plus,
  RefreshCw,
  ShieldCheck,
  Target,
  Trash2,
  Users,
  Wallet,
  XCircle,
} from "lucide-react";
import { Link } from "react-router-dom";
import { PageBody, PageHeader } from "@/components/layout/PageHeader";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { EmptyState } from "@/components/ui/empty-state";
import { LoadingState } from "@/components/ui/loading-state";
import { Modal } from "@/components/ui/modal";
import { Switch } from "@/components/ui/switch";
import { Tooltip } from "@/components/ui/tooltip";
import { apiDelete, apiGet, apiPost, apiPut } from "@/lib/api";
import {
  budgetPercent,
  delegationSummary,
  delegationTone,
  formatUsd,
  goalProgress,
  goalTone,
  orgChildren,
  orgRoots,
  profileFor,
} from "@/lib/governance";
import { relativeTime } from "@/lib/format";
import { links } from "@/lib/links";
import type {
  AgentDelegation,
  AgentGoal,
  AgentPerformance,
  AgentProfileRow,
  GovernanceAgent,
  GovernanceOverview,
} from "@/lib/types";
import { fieldInputClass as inputClass, fieldLabelClass as labelClass } from "@/lib/ui-classes";
import { useAppStore } from "@/store/app-store";
import { useUiStore } from "@/store/ui-store";
import { confirmAction } from "@/store/confirm-store";
import { toast } from "@/store/toast-store";
import { cn } from "@/lib/utils";

export function GovernancePage() {
  const activeWorkspaceId = useAppStore((state) => state.activeWorkspaceId);
  const workspaces = useAppStore((state) => state.workspaces);
  const setWorkspaceDialogOpen = useUiStore(
    (state) => state.setWorkspaceDialogOpen,
  );
  const workspace = workspaces.find((item) => item.id === activeWorkspaceId);

  const [data, setData] = useState<GovernanceOverview | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [editingAgent, setEditingAgent] = useState<GovernanceAgent | null>(null);
  const [goalModal, setGoalModal] = useState<{ goal?: AgentGoal } | null>(null);
  const [busy, setBusy] = useState<string | null>(null);

  const query = useMemo(
    () =>
      activeWorkspaceId
        ? `?workspace_id=${encodeURIComponent(activeWorkspaceId)}`
        : "",
    [activeWorkspaceId],
  );

  const load = useCallback(async () => {
    if (!activeWorkspaceId) {
      setData(null);
      return;
    }
    setLoading(true);
    try {
      setData(await apiGet<GovernanceOverview>(`/api/governance${query}`));
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
        <PageHeader eyebrow="governança" title="Agentes & Governança" />
        <EmptyState
          icon={Network}
          title="Nenhum workspace ativo"
          description="Selecione um workspace para definir hierarquia, metas, budgets e delegações."
          action={
            <Button size="sm" onClick={() => setWorkspaceDialogOpen(true)}>
              Criar workspace
            </Button>
          }
        />
      </PageBody>
    );
  }

  const agents = data?.agents ?? [];
  const profiles = data?.profiles ?? [];
  const goals = data?.goals ?? [];
  const performance = data?.performance ?? [];
  const delegations = data?.delegations ?? [];
  const pending = delegations.filter((item) => item.status === "PENDING");
  const activeGoals = goals.filter((goal) => goal.status === "ACTIVE");
  const budgetTotal = profiles.reduce(
    (sum, row) => sum + (row.profile.monthly_budget_usd ?? 0),
    0,
  );
  const spendTotal = profiles.reduce((sum, row) => sum + row.monthly_spend_usd, 0);

  async function saveProfile(
    agent: GovernanceAgent,
    input: AgentProfileValues,
    close: () => void,
  ) {
    setBusy(`profile-${agent.id}`);
    try {
      await apiPut(`/api/governance/agents/${encodeURIComponent(agent.id)}${query}`, {
        role: input.role.trim() || null,
        team: input.team.trim() || null,
        manager_id: input.managerId || null,
        monthly_budget_usd: input.budget.trim() ? Number(input.budget) : null,
        concurrency_quota: Number(input.quota) || 1,
        heartbeat_enabled: input.heartbeat,
        heartbeat_minutes: Number(input.heartbeatMinutes) || 60,
        notes: input.notes.trim() || null,
      });
      await load();
      close();
      toast({ tone: "success", title: `Governança de @${agent.id} salva` });
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível salvar",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setBusy(null);
    }
  }

  async function saveGoal(input: GoalValues, goal: AgentGoal | undefined, close: () => void) {
    setBusy("goal");
    try {
      const payload = {
        agent_id: input.agentId,
        title: input.title.trim(),
        description: input.description.trim() || null,
        metric: input.metric.trim() || null,
        target_value: input.target.trim() ? Number(input.target) : null,
        current_value: input.current.trim() ? Number(input.current) : 0,
        unit: input.unit.trim() || null,
        due_at: input.dueAt || null,
        status: input.status,
      };
      if (goal) {
        await apiPut(`/api/governance/goals/${goal.id}${query}`, payload);
      } else {
        await apiPost(`/api/governance/goals${query}`, payload);
      }
      await load();
      close();
      toast({ tone: "success", title: goal ? "Meta atualizada" : "Meta criada" });
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível salvar a meta",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setBusy(null);
    }
  }

  async function removeGoal(goal: AgentGoal) {
    if (
      !(await confirmAction({
        title: `Excluir a meta “${goal.title}”?`,
        description: "A meta e o histórico de progresso serão removidos.",
        confirmLabel: "Excluir meta",
        tone: "destructive",
      }))
    ) {
      return;
    }
    setBusy(`goal-${goal.id}`);
    try {
      await apiDelete(`/api/governance/goals/${goal.id}${query}`);
      await load();
      toast({ tone: "success", title: "Meta excluída" });
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível excluir",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setBusy(null);
    }
  }

  async function createDelegation(input: DelegationValues) {
    if (!input.fromAgent || !input.toAgent || input.fromAgent === input.toAgent) {
      toast({
        tone: "error",
        title: "Escolha agentes diferentes",
        description: "A delegação precisa sair de um agente e ir para outro.",
      });
      return;
    }
    setBusy("delegation");
    try {
      await apiPost(`/api/governance/delegations${query}`, {
        from_agent: input.fromAgent,
        to_agent: input.toAgent,
        title: input.title.trim(),
        prompt: input.prompt.trim(),
        requested_by: "desktop",
      });
      await load();
      toast({
        tone: "success",
        title: "Delegação solicitada",
        description: "Ela precisa de aprovação antes de virar tarefa.",
      });
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível delegar",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setBusy(null);
    }
  }

  async function resolveDelegation(delegation: AgentDelegation, approved: boolean) {
    setBusy(`delegation-${delegation.id}`);
    try {
      const result = await apiPost<AgentDelegation>(
        `/api/governance/delegations/${delegation.id}/resolve${query}`,
        { approved, resolved_by: "desktop" },
      );
      await load();
      toast({
        tone: approved ? "success" : "info",
        title: approved ? "Delegação aprovada" : "Delegação rejeitada",
        description: result.error ?? undefined,
        href: result.task_id ? links.task(result.task_id) : undefined,
      });
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível resolver",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setBusy(null);
    }
  }

  async function heartbeat(agent: GovernanceAgent) {
    setBusy(`heartbeat-${agent.id}`);
    try {
      await apiPost(`/api/governance/agents/${encodeURIComponent(agent.id)}/heartbeat${query}`);
      await load();
      toast({
        tone: "success",
        title: `Check-in de @${agent.id} criado`,
        description: "A tarefa foi enfileirada ou ficou aguardando o motor.",
      });
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível iniciar o check-in",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setBusy(null);
    }
  }

  return (
    <PageBody>
      <PageHeader
        eyebrow="governança"
        title="Agentes & Governança"
        description={`${workspace?.name ?? "Workspace"} · hierarquia, metas, budgets, performance e delegações com aprovação.`}
        actions={
          <>
            <Button variant="outline" size="sm" onClick={() => void load()} disabled={loading}>
              <RefreshCw size={13} className={loading ? "animate-spin" : undefined} />
              Atualizar
            </Button>
            <Button size="sm" onClick={() => setGoalModal({})}>
              <Plus size={13} />
              Nova meta
            </Button>
          </>
        }
      />

      {error ? (
        <p className="rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[11px] text-red-300">
          {error}
        </p>
      ) : null}

      <div className="grid grid-cols-2 gap-2 lg:grid-cols-4">
        <StatCard icon={<Users size={13} />} label="Agentes" value={String(agents.length)} hint={`${profiles.length} com perfil`} />
        <StatCard icon={<Wallet size={13} />} label="Budget mensal" value={formatUsd(budgetTotal)} hint={`${formatUsd(spendTotal)} gastos`} />
        <StatCard icon={<Target size={13} />} label="Metas ativas" value={String(activeGoals.length)} hint={`${goals.length} no total`} />
        <StatCard icon={<ShieldCheck size={13} />} label="Delegações" value={String(pending.length)} hint={`${delegations.length} registradas`} />
      </div>

      {loading && !data ? (
        <LoadingState rows={3} label="Carregando governança…" />
      ) : (
        <>
          <section className="space-y-2">
            <SectionTitle icon={<Network size={13} />} title="Organograma" hint="Líderes e subordinados por workspace" />
            {agents.length === 0 ? (
              <EmptyState
                icon={Network}
                title="Nenhum agente neste workspace"
                description="Crie agentes no Studio para montar o organograma."
                action={
                  <Link to="/studio" className="text-[11px] text-blue-300 hover:text-blue-200">
                    Abrir Studio
                  </Link>
                }
              />
            ) : (
              <div className="space-y-2">
                {orgRoots(agents, profiles).length > 0
                  ? orgRoots(agents, profiles).map((agent) => (
                      <OrgNode
                        key={agent.id}
                        agent={agent}
                        agents={agents}
                        profiles={profiles}
                        performance={performance}
                        depth={0}
                        busy={busy}
                        onEdit={setEditingAgent}
                        onHeartbeat={(target) => void heartbeat(target)}
                      />
                    ))
                  : agents.map((agent) => (
                      <OrgNode
                        key={agent.id}
                        agent={agent}
                        agents={agents}
                        profiles={profiles}
                        performance={performance}
                        depth={0}
                        busy={busy}
                        onEdit={setEditingAgent}
                        onHeartbeat={(target) => void heartbeat(target)}
                      />
                    ))}
              </div>
            )}
          </section>

          <GoalsSection
            goals={goals}
            agents={agents}
            busy={busy}
            onNew={() => setGoalModal({})}
            onEdit={(goal) => setGoalModal({ goal })}
            onDelete={(goal) => void removeGoal(goal)}
          />

          <DelegationsSection
            agents={agents}
            delegations={delegations}
            busy={busy}
            onCreate={(input) => void createDelegation(input)}
            onResolve={(delegation, approved) => void resolveDelegation(delegation, approved)}
          />

          <PerformanceSection rows={performance} />
        </>
      )}

      <ProfileModal
        agent={editingAgent}
        rows={profiles}
        agents={agents}
        saving={editingAgent ? busy === `profile-${editingAgent.id}` : false}
        onClose={() => setEditingAgent(null)}
        onSave={saveProfile}
      />
      <GoalModal
        state={goalModal}
        agents={agents}
        saving={busy === "goal"}
        onClose={() => setGoalModal(null)}
        onSave={saveGoal}
      />
    </PageBody>
  );
}

type AgentProfileValues = {
  role: string;
  team: string;
  managerId: string;
  budget: string;
  quota: string;
  heartbeat: boolean;
  heartbeatMinutes: string;
  notes: string;
};

type GoalValues = {
  agentId: string;
  title: string;
  description: string;
  metric: string;
  target: string;
  current: string;
  unit: string;
  dueAt: string;
  status: string;
};

type DelegationValues = {
  fromAgent: string;
  toAgent: string;
  title: string;
  prompt: string;
};

function StatCard({
  icon,
  label,
  value,
  hint,
}: {
  icon: ReactNode;
  label: string;
  value: string;
  hint: string;
}) {
  return (
    <Card>
      <CardContent className="p-3">
        <div className="flex items-center gap-1.5 text-zinc-500">
          {icon}
          <span className="text-[9px] uppercase tracking-[0.12em]">{label}</span>
        </div>
        <p className="mt-1 font-mono text-lg text-zinc-100">{value}</p>
        <p className="text-[9px] text-zinc-600">{hint}</p>
      </CardContent>
    </Card>
  );
}

function SectionTitle({
  icon,
  title,
  hint,
}: {
  icon: ReactNode;
  title: string;
  hint?: string;
}) {
  return (
    <div className="flex flex-wrap items-center gap-2">
      <span className="text-zinc-400">{icon}</span>
      <h2 className="text-[12px] font-medium text-zinc-200">{title}</h2>
      {hint ? <span className="text-[9px] text-zinc-600">{hint}</span> : null}
    </div>
  );
}

function OrgNode({
  agent,
  agents,
  profiles,
  performance,
  depth,
  busy,
  onEdit,
  onHeartbeat,
}: {
  agent: GovernanceAgent;
  agents: GovernanceAgent[];
  profiles: AgentProfileRow[];
  performance: AgentPerformance[];
  depth: number;
  busy: string | null;
  onEdit: (agent: GovernanceAgent) => void;
  onHeartbeat: (agent: GovernanceAgent) => void;
}) {
  const row = profileFor(profiles, agent.id);
  const profile = row?.profile;
  const percent = row ? budgetPercent(row) : null;
  const perf = performance.find((item) => item.agent_id === agent.id);
  const children = orgChildren(agents, profiles, agent.id);
  const color = agent.color || "#60a5fa";
  const initials = Array.from(agent.avatar || agent.name || agent.id)
    .slice(0, 2)
    .join("");
  return (
    <div className={cn(depth > 0 && "ml-4 border-l border-zinc-800/80 pl-3")}>
      <Card className={cn(depth === 0 && "border-zinc-700/60")}>
        <CardContent className="p-3">
          <div className="flex flex-wrap items-start gap-3">
            <div
              className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl border border-zinc-700/60 text-sm"
              style={{ background: `${color}22`, color }}
            >
              {initials}
            </div>
            <div className="min-w-0 flex-1">
              <div className="flex flex-wrap items-center gap-1.5">
                <p className="text-[12px] font-medium text-zinc-100">
                  {agent.name || agent.id}
                </p>
                <span className="font-mono text-[9px] text-zinc-600">@{agent.id}</span>
                <Badge variant={agent.scope === "workspace" ? "blue" : "default"}>
                  {agent.scope}
                </Badge>
                {profile?.role ? <Badge variant="purple">{profile.role}</Badge> : null}
                {profile?.team ? <Badge variant="default">{profile.team}</Badge> : null}
                {profile?.heartbeat_enabled ? (
                  <Badge variant="green">
                    <HeartPulse size={9} />
                    heartbeat {profile.heartbeat_minutes}m
                  </Badge>
                ) : null}
                {agent.disabled ? <Badge variant="red">desabilitado</Badge> : null}
              </div>
              <p className="mt-0.5 line-clamp-1 text-[9px] text-zinc-500">
                {agent.description || "Sem descrição"}
              </p>
              <div className="mt-1.5 flex flex-wrap items-center gap-3 text-[9px] text-zinc-600">
                <span>
                  {perf
                    ? `${perf.runs} runs · ${Math.round(perf.success_rate * 100)}% sucesso`
                    : "sem runs em 30d"}
                </span>
                {row ? (
                  <span>
                    budget {formatUsd(row.monthly_spend_usd)} /{" "}
                    {formatUsd(row.profile.monthly_budget_usd ?? null)}
                  </span>
                ) : (
                  <span>sem budget definido</span>
                )}
                {profile ? <span>quota {profile.concurrency_quota}</span> : null}
                {profile?.manager_id ? <span>reporta a @{profile.manager_id}</span> : null}
              </div>
              {percent !== null ? (
                <div className="mt-1.5 h-1 overflow-hidden rounded-full bg-zinc-800">
                  <div
                    className={cn(
                      "h-full rounded-full",
                      percent >= 90 ? "bg-red-400" : percent >= 70 ? "bg-amber-400" : "bg-emerald-400",
                    )}
                    style={{ width: `${percent}%` }}
                  />
                </div>
              ) : null}
            </div>
            <div className="flex shrink-0 flex-wrap items-center gap-1.5">
              {profile ? (
                <Tooltip content="Criar uma tarefa de revisão de metas agora">
                  <Button
                    size="sm"
                    variant="outline"
                    className="h-7"
                    disabled={busy === `heartbeat-${agent.id}`}
                    onClick={() => onHeartbeat(agent)}
                  >
                    <HeartPulse size={11} />
                    Check-in
                  </Button>
                </Tooltip>
              ) : null}
              <Button
                size="sm"
                variant="outline"
                className="h-7"
                onClick={() => onEdit(agent)}
              >
                <Pencil size={11} />
                Editar
              </Button>
              <Link
                to={links.agent(agent.id, agent.scope, agent.scope === "workspace" ? profile?.workspace_id : undefined)}
                className="inline-flex h-7 items-center gap-1 rounded-lg border border-zinc-800 px-2 text-[10px] text-zinc-300 hover:bg-zinc-900"
              >
                Studio
              </Link>
            </div>
          </div>
        </CardContent>
      </Card>
      {children.length > 0 && depth < 6 ? (
        <div className="mt-1.5 space-y-1.5">
          {children.map((child) => (
            <OrgNode
              key={child.id}
              agent={child}
              agents={agents}
              profiles={profiles}
              performance={performance}
              depth={depth + 1}
              busy={busy}
              onEdit={onEdit}
              onHeartbeat={onHeartbeat}
            />
          ))}
        </div>
      ) : null}
    </div>
  );
}

function GoalsSection({
  goals,
  agents,
  busy,
  onNew,
  onEdit,
  onDelete,
}: {
  goals: AgentGoal[];
  agents: GovernanceAgent[];
  busy: string | null;
  onNew: () => void;
  onEdit: (goal: AgentGoal) => void;
  onDelete: (goal: AgentGoal) => void;
}) {
  return (
    <section className="space-y-2">
      <div className="flex items-center justify-between gap-2">
        <SectionTitle icon={<Target size={13} />} title="Metas" hint="OKRs e progresso por agente" />
        <Button variant="outline" size="sm" onClick={onNew}>
          <Plus size={11} />
          Nova meta
        </Button>
      </div>
      {goals.length === 0 ? (
        <EmptyState
          icon={Target}
          title="Nenhuma meta definida"
          description="Crie metas para orientar heartbeats, delegações e a revisão de performance."
        />
      ) : (
        <div className="grid gap-1.5 lg:grid-cols-2">
          {goals.map((goal) => {
            const progress = goalProgress(goal);
            return (
              <Card key={goal.id}>
                <CardContent className="p-3">
                  <div className="flex items-start justify-between gap-2">
                    <div className="min-w-0">
                      <div className="flex flex-wrap items-center gap-1.5">
                        <p className="text-[11px] font-medium text-zinc-200">{goal.title}</p>
                        <Badge variant={goalTone(goal.status)}>{goal.status}</Badge>
                        <span className="font-mono text-[9px] text-zinc-600">@{goal.agent_id}</span>
                      </div>
                      <p className="mt-0.5 line-clamp-1 text-[9px] text-zinc-500">
                        {goal.description || goal.metric || "Sem descrição"}
                      </p>
                    </div>
                    <div className="flex shrink-0 gap-1">
                      <Button
                        size="sm"
                        variant="ghost"
                        className="h-6 w-6 p-0"
                        aria-label={`Editar meta ${goal.title}`}
                        onClick={() => onEdit(goal)}
                      >
                        <Pencil size={11} />
                      </Button>
                      <Button
                        size="sm"
                        variant="ghost"
                        className="h-6 w-6 p-0 text-zinc-500 hover:text-red-300"
                        aria-label={`Excluir meta ${goal.title}`}
                        disabled={busy === `goal-${goal.id}`}
                        onClick={() => onDelete(goal)}
                      >
                        <Trash2 size={11} />
                      </Button>
                    </div>
                  </div>
                  <div className="mt-2 flex items-center gap-2">
                    <div className="h-1 flex-1 overflow-hidden rounded-full bg-zinc-800">
                      <div
                        className={cn(
                          "h-full rounded-full",
                          progress >= 100 ? "bg-emerald-400" : "bg-violet-400",
                        )}
                        style={{ width: `${progress}%` }}
                      />
                    </div>
                    <span className="font-mono text-[9px] text-zinc-500">
                      {goal.current_value}
                      {goal.target_value !== null && goal.target_value !== undefined
                        ? ` / ${goal.target_value}`
                        : ""}{" "}
                      {goal.unit ?? ""}
                    </span>
                  </div>
                  {goal.due_at ? (
                    <p className="mt-1 text-[9px] text-zinc-600">prazo {goal.due_at}</p>
                  ) : null}
                </CardContent>
              </Card>
            );
          })}
        </div>
      )}
      {agents.length === 0 ? (
        <p className="text-[9px] text-zinc-600">
          Crie agentes no Studio antes de cadastrar metas.
        </p>
      ) : null}
    </section>
  );
}

function DelegationsSection({
  agents,
  delegations,
  busy,
  onCreate,
  onResolve,
}: {
  agents: GovernanceAgent[];
  delegations: AgentDelegation[];
  busy: string | null;
  onCreate: (input: DelegationValues) => void;
  onResolve: (delegation: AgentDelegation, approved: boolean) => void;
}) {
  const [form, setForm] = useState<DelegationValues>({
    fromAgent: agents[0]?.id ?? "",
    toAgent: agents[1]?.id ?? agents[0]?.id ?? "",
    title: "",
    prompt: "",
  });
  useEffect(() => {
    setForm((current) => ({
      ...current,
      fromAgent: current.fromAgent || agents[0]?.id || "",
      toAgent:
        current.toAgent ||
        agents.find((agent) => agent.id !== (current.fromAgent || agents[0]?.id))?.id ||
        "",
    }));
  }, [agents]);
  const pending = delegations.filter((item) => item.status === "PENDING");
  return (
    <section className="space-y-2">
      <SectionTitle icon={<ShieldCheck size={13} />} title="Delegações" hint="agente → agente com aprovação humana" />
      <Card>
        <CardContent className="grid gap-2 p-3 md:grid-cols-2">
          <label>
            <span className={labelClass}>De</span>
            <select
              value={form.fromAgent}
              onChange={(event) => setForm({ ...form, fromAgent: event.target.value })}
              className={inputClass}
            >
              {agents.map((agent) => (
                <option key={agent.id} value={agent.id}>
                  @{agent.id}
                </option>
              ))}
            </select>
          </label>
          <label>
            <span className={labelClass}>Para</span>
            <select
              value={form.toAgent}
              onChange={(event) => setForm({ ...form, toAgent: event.target.value })}
              className={inputClass}
            >
              {agents.map((agent) => (
                <option key={agent.id} value={agent.id}>
                  @{agent.id}
                </option>
              ))}
            </select>
          </label>
          <label>
            <span className={labelClass}>Título</span>
            <input
              value={form.title}
              onChange={(event) => setForm({ ...form, title: event.target.value })}
              placeholder="Revisar o relatório trimestral"
              className={inputClass}
            />
          </label>
          <label>
            <span className={labelClass}>Prompt</span>
            <input
              value={form.prompt}
              onChange={(event) => setForm({ ...form, prompt: event.target.value })}
              placeholder="O que o agente deve fazer?"
              className={inputClass}
            />
          </label>
          <div className="md:col-span-2 flex justify-end">
            <Button
              size="sm"
              disabled={busy === "delegation" || agents.length < 2}
              onClick={() => onCreate(form)}
            >
              <ArrowRight size={12} />
              Solicitar delegação
            </Button>
          </div>
        </CardContent>
      </Card>
      {pending.length > 0 ? (
        <div className="space-y-1.5">
          {pending.map((delegation) => (
            <Card key={delegation.id} className="border-amber-500/25">
              <CardContent className="flex flex-wrap items-start gap-2 p-3">
                <div className="min-w-0 flex-1">
                  <div className="flex flex-wrap items-center gap-1.5">
                    <Badge variant="yellow">pendente</Badge>
                    <p className="text-[11px] font-medium text-zinc-200">{delegation.title}</p>
                    <span className="font-mono text-[9px] text-zinc-600">
                      {delegationSummary(delegation)}
                    </span>
                  </div>
                  <p className="mt-1 line-clamp-2 text-[10px] leading-relaxed text-zinc-500">
                    {delegation.prompt}
                  </p>
                  <p className="mt-1 text-[9px] text-zinc-600">
                    solicitada {relativeTime(delegation.requested_at)}
                  </p>
                </div>
                <div className="flex shrink-0 gap-1.5">
                  <Button
                    size="sm"
                    disabled={busy === `delegation-${delegation.id}`}
                    onClick={() => onResolve(delegation, true)}
                  >
                    <CheckCircle2 size={11} />
                    Aprovar e executar
                  </Button>
                  <Button
                    size="sm"
                    variant="outline"
                    className="border-red-500/30 text-red-300"
                    disabled={busy === `delegation-${delegation.id}`}
                    onClick={() => onResolve(delegation, false)}
                  >
                    <XCircle size={11} />
                    Rejeitar
                  </Button>
                </div>
              </CardContent>
            </Card>
          ))}
        </div>
      ) : null}
      {delegations.length > pending.length ? (
        <div className="flex flex-wrap gap-1.5">
          {delegations
            .filter((item) => item.status !== "PENDING")
            .slice(0, 6)
            .map((delegation) => (
              <div
                key={delegation.id}
                className="flex items-center gap-1.5 rounded-lg border border-zinc-800 bg-zinc-950/30 px-2 py-1"
              >
                <Badge variant={delegationTone(delegation.status)}>{delegation.status}</Badge>
                <span className="text-[9px] text-zinc-500">{delegation.title}</span>
                <span className="font-mono text-[9px] text-zinc-600">
                  {delegationSummary(delegation)}
                </span>
                {delegation.task_id ? (
                  <Link
                    to={links.task(delegation.task_id)}
                    className="text-[9px] text-blue-300 hover:text-blue-200"
                  >
                    tarefa
                  </Link>
                ) : null}
              </div>
            ))}
        </div>
      ) : null}
    </section>
  );
}

function PerformanceSection({ rows }: { rows: AgentPerformance[] }) {
  return (
    <section className="space-y-2">
      <SectionTitle icon={<Activity size={13} />} title="Performance" hint="últimos 30 dias" />
      {rows.length === 0 ? (
        <EmptyState
          icon={Activity}
          title="Sem execuções no período"
          description="A performance aparece quando os agentes começarem a rodar tarefas."
        />
      ) : (
        <Card>
          <div className="overflow-x-auto">
            <table className="w-full text-left text-[10px]">
              <thead className="border-b border-zinc-800 text-[9px] uppercase tracking-wide text-zinc-600">
                <tr>
                  <th className="px-3 py-2">Agente</th>
                  <th className="px-3 py-2">Runs</th>
                  <th className="px-3 py-2">Sucesso</th>
                  <th className="px-3 py-2">Falhas</th>
                  <th className="px-3 py-2">Custo</th>
                  <th className="px-3 py-2">Duração média</th>
                  <th className="px-3 py-2">Tokens</th>
                </tr>
              </thead>
              <tbody>
                {rows.map((row) => (
                  <tr key={row.agent_id} className="border-b border-zinc-800/50 last:border-0">
                    <td className="px-3 py-2 font-mono text-zinc-300">@{row.agent_id}</td>
                    <td className="px-3 py-2 text-zinc-400">{row.runs}</td>
                    <td className="px-3 py-2">
                      <Badge variant={row.success_rate >= 0.8 ? "green" : row.success_rate >= 0.5 ? "yellow" : "red"}>
                        {Math.round(row.success_rate * 100)}%
                      </Badge>
                    </td>
                    <td className="px-3 py-2 text-zinc-400">{row.failed + row.cancelled}</td>
                    <td className="px-3 py-2 text-zinc-400">{formatUsd(row.cost_usd)}</td>
                    <td className="px-3 py-2 text-zinc-400">
                      {(row.avg_duration_ms / 1000).toFixed(1)}s
                    </td>
                    <td className="px-3 py-2 text-zinc-400">{row.tokens}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </Card>
      )}
    </section>
  );
}

function ProfileModal({
  agent,
  rows,
  agents,
  saving,
  onClose,
  onSave,
}: {
  agent: GovernanceAgent | null;
  rows: AgentProfileRow[];
  agents: GovernanceAgent[];
  saving: boolean;
  onClose: () => void;
  onSave: (agent: GovernanceAgent, input: AgentProfileValues, close: () => void) => void;
}) {
  const existing = agent ? profileFor(rows, agent.id)?.profile : undefined;
  const [values, setValues] = useState<AgentProfileValues>(emptyProfile());
  useEffect(() => {
    if (!agent) return;
    setValues({
      role: existing?.role ?? "",
      team: existing?.team ?? "",
      managerId: existing?.manager_id ?? "",
      budget:
        existing?.monthly_budget_usd !== null && existing?.monthly_budget_usd !== undefined
          ? String(existing.monthly_budget_usd)
          : "",
      quota: String(existing?.concurrency_quota ?? 5),
      heartbeat: existing?.heartbeat_enabled ?? false,
      heartbeatMinutes: String(existing?.heartbeat_minutes ?? 60),
      notes: existing?.notes ?? "",
    });
  }, [agent, existing]);
  return (
    <Modal
      open={agent !== null}
      onClose={onClose}
      title={agent ? `Governança de @${agent.id}` : "Governança do agente"}
      description="Hierarquia, budget, quota e heartbeat são estado operacional do OpenDesk; o arquivo do agente continua sendo a fonte de verdade do OpenCode."
      className="max-w-lg"
    >
      {agent ? (
        <div className="space-y-3">
          <div className="grid gap-2 md:grid-cols-2">
            <label>
              <span className={labelClass}>Cargo / papel</span>
              <input
                value={values.role}
                onChange={(event) => setValues({ ...values, role: event.target.value })}
                placeholder="Líder de pesquisa"
                className={inputClass}
              />
            </label>
            <label>
              <span className={labelClass}>Time</span>
              <input
                value={values.team}
                onChange={(event) => setValues({ ...values, team: event.target.value })}
                placeholder="Inteligência"
                className={inputClass}
              />
            </label>
            <label>
              <span className={labelClass}>Reporta a</span>
              <select
                value={values.managerId}
                onChange={(event) => setValues({ ...values, managerId: event.target.value })}
                className={inputClass}
              >
                <option value="">sem líder</option>
                {agents
                  .filter((item) => item.id !== agent.id)
                  .map((item) => (
                    <option key={item.id} value={item.id}>
                      @{item.id}
                    </option>
                  ))}
              </select>
            </label>
            <label>
              <span className={labelClass}>Budget mensal (US$)</span>
              <input
                type="number"
                min={0}
                step="0.01"
                value={values.budget}
                onChange={(event) => setValues({ ...values, budget: event.target.value })}
                placeholder="sem limite"
                className={inputClass}
              />
            </label>
            <label>
              <span className={labelClass}>Quota de concorrência</span>
              <input
                type="number"
                min={1}
                max={32}
                value={values.quota}
                onChange={(event) => setValues({ ...values, quota: event.target.value })}
                className={inputClass}
              />
            </label>
            <label>
              <span className={labelClass}>Intervalo do heartbeat (min)</span>
              <input
                type="number"
                min={5}
                max={1440}
                value={values.heartbeatMinutes}
                disabled={!values.heartbeat}
                onChange={(event) =>
                  setValues({ ...values, heartbeatMinutes: event.target.value })
                }
                className={inputClass}
              />
            </label>
          </div>
          <div className="flex items-center justify-between gap-3 rounded-lg border border-zinc-800/70 bg-zinc-950/30 px-3 py-2">
            <div>
              <p className="text-[10px] font-medium text-zinc-300">Heartbeat autônomo</p>
              <p className="text-[9px] text-zinc-600">
                Cria uma tarefa de revisão de metas no intervalo, respeitando budget e quota.
              </p>
            </div>
            <Switch
              checked={values.heartbeat}
              onCheckedChange={(heartbeat) => setValues({ ...values, heartbeat })}
              label="Heartbeat autônomo"
            />
          </div>
          <label>
            <span className={labelClass}>Notas</span>
            <textarea
              value={values.notes}
              onChange={(event) => setValues({ ...values, notes: event.target.value })}
              rows={2}
              className={cn(inputClass, "h-auto py-2")}
            />
          </label>
          {existing?.last_heartbeat_at ? (
            <p className="text-[9px] text-zinc-600">
              Último heartbeat {relativeTime(existing.last_heartbeat_at)}
              {existing.next_heartbeat_at
                ? ` · próximo ${new Date(existing.next_heartbeat_at).toLocaleString("pt-BR")}`
                : ""}
            </p>
          ) : null}
          <div className="flex justify-end gap-2">
            <Button variant="ghost" size="sm" onClick={onClose}>
              Cancelar
            </Button>
            <Button
              size="sm"
              disabled={saving}
              onClick={() => onSave(agent, values, onClose)}
            >
              {saving ? <Loader2 size={13} className="animate-spin" /> : <Pencil size={13} />}
              Salvar governança
            </Button>
          </div>
        </div>
      ) : null}
    </Modal>
  );
}

function GoalModal({
  state,
  agents,
  saving,
  onClose,
  onSave,
}: {
  state: { goal?: AgentGoal } | null;
  agents: GovernanceAgent[];
  saving: boolean;
  onClose: () => void;
  onSave: (input: GoalValues, goal: AgentGoal | undefined, close: () => void) => void;
}) {
  const goal = state?.goal;
  const [values, setValues] = useState<GoalValues>(emptyGoal());
  useEffect(() => {
    if (!state) return;
    setValues(
      goal
        ? {
            agentId: goal.agent_id,
            title: goal.title,
            description: goal.description ?? "",
            metric: goal.metric ?? "",
            target:
              goal.target_value !== null && goal.target_value !== undefined
                ? String(goal.target_value)
                : "",
            current: String(goal.current_value),
            unit: goal.unit ?? "",
            dueAt: goal.due_at ? goal.due_at.slice(0, 10) : "",
            status: goal.status,
          }
        : { ...emptyGoal(), agentId: agents[0]?.id ?? "" },
    );
  }, [state, goal, agents]);
  return (
    <Modal
      open={state !== null}
      onClose={onClose}
      title={goal ? "Editar meta" : "Nova meta"}
      description="Metas orientam heartbeats e a revisão de performance do agente."
      className="max-w-lg"
    >
      <div className="space-y-3">
        <div className="grid gap-2 md:grid-cols-2">
          <label>
            <span className={labelClass}>Agente</span>
            <select
              value={values.agentId}
              onChange={(event) => setValues({ ...values, agentId: event.target.value })}
              className={inputClass}
            >
              {agents.map((agent) => (
                <option key={agent.id} value={agent.id}>
                  @{agent.id}
                </option>
              ))}
            </select>
          </label>
          <label>
            <span className={labelClass}>Status</span>
            <select
              value={values.status}
              onChange={(event) => setValues({ ...values, status: event.target.value })}
              className={inputClass}
            >
              <option value="ACTIVE">Ativa</option>
              <option value="PAUSED">Pausada</option>
              <option value="DONE">Concluída</option>
              <option value="CANCELLED">Cancelada</option>
            </select>
          </label>
        </div>
        <label>
          <span className={labelClass}>Título</span>
          <input
            value={values.title}
            onChange={(event) => setValues({ ...values, title: event.target.value })}
            placeholder="Publicar 4 relatórios"
            className={inputClass}
          />
        </label>
        <label>
          <span className={labelClass}>Descrição</span>
          <textarea
            value={values.description}
            onChange={(event) => setValues({ ...values, description: event.target.value })}
            rows={2}
            className={cn(inputClass, "h-auto py-2")}
          />
        </label>
        <div className="grid gap-2 md:grid-cols-4">
          <label className="md:col-span-2">
            <span className={labelClass}>Métrica</span>
            <input
              value={values.metric}
              onChange={(event) => setValues({ ...values, metric: event.target.value })}
              placeholder="relatórios"
              className={inputClass}
            />
          </label>
          <label>
            <span className={labelClass}>Atual</span>
            <input
              type="number"
              value={values.current}
              onChange={(event) => setValues({ ...values, current: event.target.value })}
              className={inputClass}
            />
          </label>
          <label>
            <span className={labelClass}>Meta</span>
            <input
              type="number"
              value={values.target}
              onChange={(event) => setValues({ ...values, target: event.target.value })}
              className={inputClass}
            />
          </label>
        </div>
        <div className="grid gap-2 md:grid-cols-2">
          <label>
            <span className={labelClass}>Unidade</span>
            <input
              value={values.unit}
              onChange={(event) => setValues({ ...values, unit: event.target.value })}
              placeholder="relatórios"
              className={inputClass}
            />
          </label>
          <label>
            <span className={labelClass}>Prazo</span>
            <input
              type="date"
              value={values.dueAt}
              onChange={(event) => setValues({ ...values, dueAt: event.target.value })}
              className={inputClass}
            />
          </label>
        </div>
        <div className="flex justify-end gap-2">
          <Button variant="ghost" size="sm" onClick={onClose}>
            Cancelar
          </Button>
          <Button
            size="sm"
            disabled={saving || !values.title.trim() || !values.agentId}
            onClick={() => onSave(values, goal, onClose)}
          >
            {saving ? <Loader2 size={13} className="animate-spin" /> : <Target size={13} />}
            {goal ? "Salvar meta" : "Criar meta"}
          </Button>
        </div>
      </div>
    </Modal>
  );
}

function emptyProfile(): AgentProfileValues {
  return {
    role: "",
    team: "",
    managerId: "",
    budget: "",
    quota: "5",
    heartbeat: false,
    heartbeatMinutes: "60",
    notes: "",
  };
}

function emptyGoal(): GoalValues {
  return {
    agentId: "",
    title: "",
    description: "",
    metric: "",
    target: "",
    current: "0",
    unit: "",
    dueAt: "",
    status: "ACTIVE",
  };
}

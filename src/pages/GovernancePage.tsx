import { useCallback, useEffect, useMemo, useState } from "react";
import {
  Network,
  Plus,
  RefreshCw,
  ShieldCheck,
  Target,
  Users,
  Wallet,
} from "lucide-react";
import { PageBody, PageHeader } from "@/components/layout/PageHeader";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { EmptyState } from "@/components/ui/empty-state";
import { LoadingState } from "@/components/ui/loading-state";
import { DelegationsSection } from "@/components/governance/DelegationsSection";
import { GoalModal } from "@/components/governance/GoalModal";
import { GoalsSection } from "@/components/governance/GoalsSection";
import { OrgChart } from "@/components/governance/OrgChart";
import { PerformanceSection } from "@/components/governance/PerformanceSection";
import { ProfileModal } from "@/components/governance/ProfileModal";
import { StatCard } from "@/components/governance/StatCard";
import type {
  AgentProfileValues,
  DelegationValues,
  GoalValues,
} from "@/components/governance/types";
import { apiDelete, apiGet, apiPost, apiPut } from "@/lib/api";
import { formatUsd } from "@/lib/governance";
import { links } from "@/lib/links";
import type {
  AgentDelegation,
  AgentGoal,
  GovernanceAgent,
  GovernanceOverview,
} from "@/lib/types";
import { useAppStore } from "@/store/app-store";
import { useUiStore } from "@/store/ui-store";
import { confirmAction } from "@/store/confirm-store";
import { toast } from "@/store/toast-store";

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
        <PageHeader title="Agentes & Governança" />
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
      await apiPut(
        `/api/governance/agents/${encodeURIComponent(agent.id)}${query}`,
        {
          role: input.role.trim() || null,
          team: input.team.trim() || null,
          manager_id: input.managerId || null,
          monthly_budget_usd: input.budget.trim() ? Number(input.budget) : null,
          concurrency_quota: Number(input.quota) || 1,
          heartbeat_enabled: input.heartbeat,
          heartbeat_minutes: Number(input.heartbeatMinutes) || 60,
          notes: input.notes.trim() || null,
        },
      );
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

  async function saveGoal(
    input: GoalValues,
    goal: AgentGoal | undefined,
    close: () => void,
  ) {
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

  async function resolveDelegation(
    delegation: AgentDelegation,
    approved: boolean,
  ) {
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
      await apiPost(
        `/api/governance/agents/${encodeURIComponent(agent.id)}/heartbeat${query}`,
      );
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
        title="Agentes & Governança"
        description={`${workspace?.name ?? "Workspace"} · hierarquia, metas, budgets, performance e delegações com aprovação.`}
        actions={
          <>
            <Button
              variant="outline"
              size="sm"
              onClick={() => void load()}
              disabled={loading}
            >
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

      <Card className="overflow-hidden">
        <div className="grid grid-cols-2 gap-px bg-zinc-800/50 lg:grid-cols-4">
        <StatCard
          icon={<Users size={13} />}
          label="Agentes"
          value={String(agents.length)}
          hint={`${profiles.length} com perfil`}
        />
        <StatCard
          icon={<Wallet size={13} />}
          label="Budget mensal"
          value={formatUsd(budgetTotal)}
          hint={`${formatUsd(spendTotal)} gastos`}
        />
        <StatCard
          icon={<Target size={13} />}
          label="Metas ativas"
          value={String(activeGoals.length)}
          hint={`${goals.length} no total`}
        />
        <StatCard
          icon={<ShieldCheck size={13} />}
          label="Delegações"
          value={String(pending.length)}
          hint={`${delegations.length} registradas`}
        />
        </div>
      </Card>

      {loading && !data ? (
        <LoadingState rows={3} label="Carregando governança…" />
      ) : (
        <>
          <OrgChart
            agents={agents}
            profiles={profiles}
            performance={performance}
            busy={busy}
            onEdit={setEditingAgent}
            onHeartbeat={(agent) => void heartbeat(agent)}
          />
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
            onResolve={(delegation, approved) =>
              void resolveDelegation(delegation, approved)
            }
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

import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { RefreshCw, RotateCw } from "lucide-react";
import { PageBody, PageHeader } from "@/components/layout/PageHeader";
import { Button } from "@/components/ui/button";
import { AttentionCard } from "@/components/dashboard/AttentionCard";
import { ActivityFeed } from "@/components/dashboard/ActivityFeed";
import { CostMixCard } from "@/components/dashboard/CostMixCard";
import { IntegrationFootprint } from "@/components/dashboard/IntegrationFootprint";
import { useAppStore } from "@/store/app-store";
import { useUiStore } from "@/store/ui-store";
import { apiGet } from "@/lib/api";
import type {
  AgentFile,
  AgentInfo,
  Artifact,
  IntegrationOverviewItem,
  TaskRun,
} from "@/lib/types";
import { shortPath } from "@/lib/format";
import { links } from "@/lib/links";
import { AgentsCard } from "@/pages/dashboard/AgentsCard";
import { AutomationsBar } from "@/pages/dashboard/AutomationsBar";
import {
  buildActivities,
  buildModelUsage,
  dashboardMetrics,
} from "@/pages/dashboard/derive";
import { EmptyWorkspace } from "@/pages/dashboard/EmptyWorkspace";
import { EngineCard } from "@/pages/dashboard/EngineCard";
import { ModelUsageCard } from "@/pages/dashboard/ModelUsageCard";
import { PinnedArtifactsCard } from "@/pages/dashboard/PinnedArtifactsCard";
import { RunningRunsCard } from "@/pages/dashboard/RunningRunsCard";
import { DashboardKpis } from "@/pages/dashboard/widgets";
import { WorkspaceFooter } from "@/pages/dashboard/WorkspaceFooter";

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
  const [refreshing, setRefreshing] = useState(false);
  const [recentRuns, setRecentRuns] = useState<TaskRun[]>([]);
  const [recentArtifacts, setRecentArtifacts] = useState<Artifact[]>([]);
  const [pinnedArtifacts, setPinnedArtifacts] = useState<Artifact[]>([]);
  const [integrations, setIntegrations] = useState<IntegrationOverviewItem[]>([]);
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
    if (!activeWorkspaceId) {
      setIntegrations([]);
      return;
    }
    let cancelled = false;
    void apiGet<IntegrationOverviewItem[]>(
      `/api/integrations/overview?workspace_id=${encodeURIComponent(activeWorkspaceId)}`,
    )
      .then((items) => {
        if (!cancelled) setIntegrations(items);
      })
      .catch(() => {
        if (!cancelled) setIntegrations([]);
      });
    return () => {
      cancelled = true;
    };
  }, [activeWorkspaceId]);

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
      <EmptyWorkspace onCreate={() => setWorkspaceDialogOpen(true)} />
    );
  }

  const metrics = dashboardMetrics(dashboard, budget, FALLBACK_DAILY_LIMIT);

  const modelUsage = buildModelUsage(recentRuns);

  const activities = buildActivities(recentRuns, recentArtifacts);

  const runningRuns = recentRuns.filter((run) => run.status === "RUNNING");

  async function refreshAll() {
    setRefreshing(true);
    try {
      await refresh();
    } finally {
      setRefreshing(false);
    }
  }

  return (
    <PageBody>
      <PageHeader
        title="Dashboard"
        description={`${activeWorkspace.name} · ${shortPath(activeWorkspace.root_path, 64)}`}
        actions={
          <>
            <Button
              variant="outline"
              size="sm"
              onClick={() => void refreshAll()}
              disabled={refreshing}
            >
              <RefreshCw
                size={14}
                className={refreshing ? "animate-spin" : undefined}
              />
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

      <RunningRunsCard
        runs={runningRuns}
        onOpenRun={(runId) => navigate(links.run(runId))}
      />

      <DashboardKpis dashboard={dashboard} metrics={metrics} />

      <AutomationsBar dashboard={dashboard} />

      <CostMixCard />

      <PinnedArtifactsCard
        artifacts={pinnedArtifacts}
        onOpenArtifact={(artifactId) => navigate(links.artifact(artifactId))}
      />

      <div className="grid gap-3 lg:grid-cols-3">
        <AgentsCard
          agents={agents}
          presentation={presentation}
          agentStates={agentStates}
          agentsError={agentsError}
          onOpenStudio={() => navigate("/studio")}
        />

        <EngineCard daemon={daemon} />
      </div>

      <div className="grid gap-3 lg:grid-cols-2">
        <ModelUsageCard modelUsage={modelUsage} />

        <ActivityFeed
          activities={activities}
          onCreateTask={() => setTaskDialogOpen(true)}
        />
      </div>

      <IntegrationFootprint items={integrations} />

      <WorkspaceFooter workspace={activeWorkspace} />
    </PageBody>
  );
}

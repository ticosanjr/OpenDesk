import {
  markNotificationOnce,
  notificationEnabled,
  sendNativeNotification,
} from "@/lib/notification-policy";
import { invalidateAgents, invalidateModels, invalidateRuntime } from "@/lib/studio-cache";
import type { Artifact, DaemonStatus, QueueJob, WsEvent } from "@/lib/types";
import type { AppStore, LiveRevisions } from "./app-store-types";

type SetState = (
  partial: Partial<AppStore> | ((state: AppStore) => Partial<AppStore>),
) => void;

export interface EventHooks {
  getState: () => AppStore;
  setState: SetState;
  scheduleLiveRefresh: () => void;
}

function bump(hooks: EventHooks, domain: keyof LiveRevisions) {
  hooks.setState((state) => ({
    revisions: {
      ...state.revisions,
      [domain]: state.revisions[domain] + 1,
    },
  }));
}

export function reduceEvent(event: WsEvent, hooks: EventHooks) {
  const state = hooks.getState();
  if (event.event_id > state.lastEventId) {
    hooks.setState({ lastEventId: event.event_id });
  }

  switch (event.event_type) {
    case "DaemonStatusChanged":
      hooks.setState({ daemon: event.payload as DaemonStatus });
      invalidateModels();
      break;
    case "QueueUpdated": {
      const payload = event.payload as {
        workspace_id?: string;
        jobs?: QueueJob[];
      };
      // Queue events are broadcast per workspace. Applying another workspace's
      // history made the sidebar badge show a number that did not belong to the
      // active board.
      if (
        !payload.workspace_id ||
        payload.workspace_id === hooks.getState().activeWorkspaceId
      ) {
        const jobs = payload.jobs ?? [];
        const running = jobs.filter((job) => job.status === "RUNNING").length;
        const waiting = jobs.filter(
          (job) => job.status === "QUEUED" || job.status === "PAUSED",
        ).length;
        hooks.setState((current) => ({
          queue: jobs,
          queueStatus: current.queueStatus
            ? {
                ...current.queueStatus,
                running,
                waiting,
                active_jobs: running,
              }
            : null,
          dashboard: current.dashboard
            ? {
                ...current.dashboard,
                queue_running: running,
                queue_waiting: waiting,
              }
            : current.dashboard,
          revisions: {
            ...current.revisions,
            queue: current.revisions.queue + 1,
          },
        }));
      }
      break;
    }
    case "FsSyncTriggered": {
      const payload = event.payload as {
        workspace_id?: string;
        entity?: string;
        path?: string;
      };
      hooks.setState({
        lastFsEvent: {
          entity: payload.entity ?? "file",
          path: payload.path ?? "",
          workspace_id: payload.workspace_id ?? "",
        },
      });
      break;
    }
    case "ResyncRequired":
      invalidateAgents();
      invalidateModels();
      invalidateRuntime();
      void state.refreshLiveState();
      break;
    case "TaskUpdated": {
      const payload = event.payload as {
        task_id?: string;
        workspace_id?: string;
        stage?: string;
        phase_status?: string | null;
      };
      if (payload.task_id && payload.workspace_id && payload.stage) {
        hooks.setState({
          lastTaskEvent: {
            task_id: payload.task_id,
            workspace_id: payload.workspace_id,
            stage: payload.stage,
            phase_status: payload.phase_status,
          },
        });
        bump(hooks, "tasks");
        hooks.scheduleLiveRefresh();
      }
      break;
    }
    case "RunStreamChunk": {
      const payload = event.payload as {
        run_id?: string;
        task_id?: string | null;
        chunk_type?: string;
      };
      if (payload.run_id) {
        hooks.setState({
          lastRunChunk: {
            run_id: payload.run_id,
            task_id: payload.task_id,
            chunk_type: payload.chunk_type ?? "event",
            seq: event.event_id,
          },
        });
      }
      break;
    }
    case "ChatUpdated": {
      const payload = event.payload as {
        thread_id?: string;
        workspace_id?: string;
      };
      if (payload.thread_id && payload.workspace_id) {
        hooks.setState({
          lastChatEvent: {
            thread_id: payload.thread_id,
            workspace_id: payload.workspace_id,
            seq: event.event_id,
          },
        });
      }
      break;
    }
    case "RecipeRunUpdated":
    case "HitlRequested":
    case "HitlResolved":
      hooks.setState((current) => ({
        lastRecipeEvent: current.lastRecipeEvent + 1,
        revisions: {
          ...current.revisions,
          recipes: current.revisions.recipes + 1,
          attention:
            event.event_type === "HitlRequested" ||
            event.event_type === "HitlResolved"
              ? current.revisions.attention + 1
              : current.revisions.attention,
        },
      }));
      break;
    case "ProposalCreated":
    case "ProposalResolved":
      hooks.setState((current) => ({
        lastProposalEvent: current.lastProposalEvent + 1,
        revisions: {
          ...current.revisions,
          attention: current.revisions.attention + 1,
        },
      }));
      break;
    case "PermissionRequested": {
      const payload = event.payload as {
        request_id?: string;
        task_id?: string | null;
        workspace_id?: string;
        action?: string;
        resources?: string[];
      };
      if (payload.request_id && payload.workspace_id) {
        hooks.setState((current) => ({
          lastPermissionEvent: {
            request_id: payload.request_id as string,
            task_id: payload.task_id,
            workspace_id: payload.workspace_id as string,
            action: payload.action ?? "permission",
            resources: payload.resources ?? [],
          },
          revisions: {
            ...current.revisions,
            attention: current.revisions.attention + 1,
          },
        }));
        if (
          notificationEnabled("interactions") &&
          markNotificationOnce(`permission:${payload.request_id}`)
        ) {
          void sendNativeNotification(
            "Agente bloqueado esperando você",
            `${payload.action ?? "permissão"}: ${(payload.resources ?? []).join(", ").slice(0, 120)}`,
          );
        }
      }
      break;
    }
    case "RunFinished": {
      const payload = event.payload as {
        run_id?: string;
        task_id?: string | null;
        workspace_id?: string;
        status?: string;
      };
      if (payload.run_id && payload.status) {
        hooks.setState({
          lastRunEvent: {
            run_id: payload.run_id,
            task_id: payload.task_id,
            workspace_id: payload.workspace_id,
            status: payload.status,
          },
        });
        if (
          !payload.workspace_id ||
          payload.workspace_id === hooks.getState().activeWorkspaceId
        ) {
          bump(hooks, "runs");
          hooks.scheduleLiveRefresh();
        }
      }
      break;
    }
    case "ArtifactCreated":
      hooks.setState((current) => ({
        lastArtifactEvent: current.lastArtifactEvent + 1,
        lastArtifact: (event.payload as { artifact?: Artifact }).artifact ?? null,
        revisions: {
          ...current.revisions,
          artifacts: current.revisions.artifacts + 1,
        },
        dashboard: current.dashboard
          ? {
              ...current.dashboard,
              artifacts_total: current.dashboard.artifacts_total + 1,
            }
          : current.dashboard,
      }));
      break;
    case "GoogleConnectionUpdated": {
      const payload = event.payload as {
        workspace_id?: string;
        connected?: boolean;
        email?: string | null;
      };
      if (payload.workspace_id) {
        hooks.setState({
          lastGoogleEvent: {
            workspace_id: payload.workspace_id,
            connected: payload.connected ?? false,
            email: payload.email,
          },
        });
      }
      break;
    }
    case "EngineUpdateProgress": {
      const payload = event.payload as {
        phase?: string;
        message?: string | null;
        ok?: boolean | null;
      };
      hooks.setState({
        lastEngineUpdate: {
          phase: payload.phase ?? "running",
          message: payload.message,
          ok: payload.ok,
        },
      });
      if (payload.phase === "done" || payload.phase === "failed") {
        invalidateModels();
      }
      break;
    }
    case "ScheduleUpdated": {
      const payload = event.payload as {
        schedule_id?: string;
        workspace_id?: string;
      };
      if (payload.schedule_id && payload.workspace_id) {
        hooks.setState((current) => ({
          lastScheduleEvent: {
            schedule_id: payload.schedule_id as string,
            workspace_id: payload.workspace_id as string,
          },
          revisions: {
            ...current.revisions,
            schedules: current.revisions.schedules + 1,
          },
        }));
      }
      break;
    }
    case "LearningUpdated":
      bump(hooks, "memory");
      invalidateAgents();
      invalidateModels();
      invalidateRuntime();
      break;
    case "HandoffUpdated":
    case "TicketUpdated":
      bump(hooks, "tasks");
      bump(hooks, "governance");
      hooks.scheduleLiveRefresh();
      break;
    case "BudgetAlert": {
      const payload = event.payload as {
        level?: string;
        message?: string;
        daily_spent?: number;
        daily_limit?: number;
      };
      hooks.setState({
        budgetAlert: {
          level: payload.level ?? "soft",
          message: payload.message ?? "orçamento próximo do limite",
          daily_spent: payload.daily_spent ?? 0,
          daily_limit: payload.daily_limit ?? 0,
        },
      });
      break;
    }
    default:
      break;
  }
}


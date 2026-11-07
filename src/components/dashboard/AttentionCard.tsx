import { useCallback, useEffect, useState } from "react";
import { AlertTriangle, Ban, CheckCircle2, Clock3, ExternalLink } from "lucide-react";
import { Link } from "react-router-dom";
import { Card, CardContent } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { apiGet, apiPost } from "@/lib/api";
import { links } from "@/lib/links";
import { useAppStore } from "@/store/app-store";
import { toast } from "@/store/toast-store";

interface PermissionRequest {
  id: string;
  request_id: string;
  task_id?: string | null;
  run_id?: string | null;
  permission_action: string;
  resources?: string | null;
  status: string;
  created_at: string;
}

interface HitlRequest {
  token: string;
  recipe_run_id: string;
  workspace_id: string;
  checkpoint_id: string;
  interaction_type: string;
  status: string;
  created_at: string;
}

interface Attention {
  requests: PermissionRequest[];
  hitl: HitlRequest[];
  awaiting_approval: Array<{ id: string; title: string; phase_status?: string | null }>;
  blocked: Array<{ id: string; title: string; reason?: string | null }>;
  failed_runs: Array<{
    id: string;
    title?: string | null;
    status: string;
    error?: string | null;
  }>;
}

export function AttentionCard() {
  const workspaceId = useAppStore((state) => state.activeWorkspaceId);
  const lastTaskEvent = useAppStore((state) => state.lastTaskEvent);
  const lastRunEvent = useAppStore((state) => state.lastRunEvent);
  const lastPermissionEvent = useAppStore((state) => state.lastPermissionEvent);
  const lastRecipeEvent = useAppStore((state) => state.lastRecipeEvent);
  const [data, setData] = useState<Attention | null>(null);

  const load = useCallback(async () => {
    if (!workspaceId) {
      setData(null);
      return;
    }
    try {
      const [attention, permissions, hitl] = await Promise.all([
        apiGet<Omit<Attention, "requests" | "hitl">>(
          `/api/attention?workspace_id=${encodeURIComponent(workspaceId)}`,
        ),
        apiGet<{ requests: PermissionRequest[] }>(
          `/api/permissions?workspace_id=${encodeURIComponent(workspaceId)}`,
        ),
        apiGet<{ requests: HitlRequest[] }>(
          `/api/recipes/hitl?workspace_id=${encodeURIComponent(workspaceId)}`,
        ).catch(() => ({ requests: [] })),
      ]);
      setData({ ...attention, requests: permissions.requests, hitl: hitl.requests });
    } catch {
      setData(null);
    }
  }, [workspaceId]);

  useEffect(() => {
    void load();
  }, [
    load,
    lastTaskEvent?.task_id,
    lastRunEvent?.run_id,
    lastRunEvent?.status,
    lastPermissionEvent?.request_id,
    lastRecipeEvent,
  ]);

  if (
    !data ||
    (data.requests.length === 0 &&
      data.hitl.length === 0 &&
      data.awaiting_approval.length === 0 &&
      data.blocked.length === 0 &&
      data.failed_runs.length === 0)
  ) {
    return null;
  }

  async function resolveHitl(token: string, resolution: string, text?: string) {
    try {
      await apiPost(`/api/recipes/hitl/${token}/resolve`, { resolution, text });
      toast({ tone: "success", title: resolution === "APPROVED" ? "Receita aprovada" : "Receita rejeitada" });
      await load();
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível resolver o checkpoint",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    }
  }

  async function decide(request: PermissionRequest, decision: string) {
    try {
      await apiPost(`/api/permissions/${request.request_id}/reply`, { decision });
      toast({
        tone: "success",
        title:
          decision === "reject"
            ? "Permissão negada"
            : decision === "always"
              ? "Permitido nesta tarefa"
              : "Permitido uma vez",
        href: request.task_id ? links.task(request.task_id) : undefined,
      });
      await load();
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível responder",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    }
  }

  async function retryOther(runId: string) {
    try {
      const result = await apiPost<{ model: string }>(
        `/api/runs/${runId}/retry-other-model`,
        {},
      );
      toast({
        tone: "success",
        title: "Reenfileirado com outro modelo",
        description: result.model,
      });
      await load();
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível reenfileirar",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    }
  }

  async function approve(taskId: string) {
    try {
      await apiPost(`/api/tasks/${taskId}/approve-plan`, {});
      toast({ tone: "success", title: "Plano aprovado", href: links.task(taskId) });
      await load();
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível aprovar",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    }
  }

  return (
    <Card className="border-amber-500/30 bg-amber-500/[0.04]">
      <CardContent className="space-y-2 p-3.5">
        <div className="flex flex-wrap items-center gap-2">
          <AlertTriangle size={15} className="text-amber-400" />
          <p className="text-[13px] font-semibold text-amber-100">
            Precisa da sua atenção
          </p>
          <Badge variant="yellow">
            {data.requests.length +
              data.hitl.length +
              data.awaiting_approval.length +
              data.blocked.length +
              data.failed_runs.length}
          </Badge>
          <Button variant="ghost" size="sm" className="ml-auto h-7" onClick={() => void load()}>
            atualizar
          </Button>
        </div>

        {data.hitl.map((request) => (
          <div
            key={`hitl-${request.token}`}
            className="flex flex-wrap items-center gap-2 rounded-lg border border-amber-500/25 bg-zinc-950/40 px-3 py-2"
          >
            <Clock3 size={13} className="text-amber-300" />
            <div className="min-w-0 flex-1">
              <p className="truncate text-[12px] text-zinc-200">
                Receita aguardando decisão ·{" "}
                <span className="font-mono text-[10px] text-amber-300">
                  {request.interaction_type}
                </span>
              </p>
              <p className="truncate font-mono text-[9px] text-zinc-600">
                token {request.token} · {request.checkpoint_id}
              </p>
            </div>
            <Button
              size="sm"
              className="h-7"
              onClick={() => void resolveHitl(request.token, "APPROVED")}
            >
              Aprovar
            </Button>
            <Button
              size="sm"
              variant="outline"
              className="h-7"
              onClick={() => {
                const feedback = window.prompt("Feedback para refazer o step:");
                if (feedback?.trim()) {
                  void resolveHitl(request.token, "FEEDBACK", feedback.trim());
                }
              }}
            >
              Feedback
            </Button>
            <Button
              size="sm"
              variant="outline"
              className="h-7 border-red-500/30 text-red-300"
              onClick={() => void resolveHitl(request.token, "REJECTED")}
            >
              Rejeitar
            </Button>
          </div>
        ))}

        {data.requests.map((request) => (
          <div
            key={`permission-${request.request_id}`}
            className="flex flex-wrap items-center gap-2 rounded-lg border border-amber-500/25 bg-zinc-950/40 px-3 py-2"
          >
            <Clock3 size={13} className="text-amber-300" />
            <div className="min-w-0 flex-1">
              <p className="truncate text-[12px] text-zinc-200">
                <span className="font-mono text-[10px] uppercase text-amber-300">
                  {request.permission_action}
                </span>{" "}
                {request.resources || "ação do agente"}
              </p>
              {request.task_id ? (
                <Link
                  to={links.task(request.task_id)}
                  className="text-[10px] text-zinc-500 underline-offset-2 hover:text-zinc-300 hover:underline"
                >
                  abrir tarefa
                </Link>
              ) : null}
            </div>
            <Button
              size="sm"
              className="h-7"
              onClick={() => void decide(request, "once")}
            >
              Permitir
            </Button>
            <Button
              size="sm"
              variant="outline"
              className="h-7"
              onClick={() => void decide(request, "always")}
            >
              Sempre nesta tarefa
            </Button>
            <Button
              size="sm"
              variant="outline"
              className="h-7 border-red-500/30 text-red-300"
              onClick={() => void decide(request, "reject")}
            >
              Negar
            </Button>
          </div>
        ))}

        {data.awaiting_approval.map((task) => (
          <div
            key={`approval-${task.id}`}
            className="flex flex-wrap items-center gap-2 rounded-lg border border-amber-500/20 bg-zinc-950/40 px-3 py-2"
          >
            <Clock3 size={13} className="text-amber-300" />
            <span className="min-w-0 flex-1 truncate text-[12px] text-zinc-200">
              {task.title}
            </span>
            <Badge variant="yellow">plano aguardando aprovação</Badge>
            <Button size="sm" className="h-7" onClick={() => void approve(task.id)}>
              Aprovar
            </Button>
            <Link
              to={links.task(task.id)}
              className="text-[11px] text-zinc-400 underline-offset-2 hover:text-zinc-200 hover:underline"
            >
              abrir
            </Link>
          </div>
        ))}

        {data.blocked.map((task) => (
          <div
            key={`blocked-${task.id}`}
            className="flex flex-wrap items-center gap-2 rounded-lg border border-red-500/20 bg-zinc-950/40 px-3 py-2"
          >
            <Ban size={13} className="text-red-300" />
            <span className="min-w-0 flex-1 truncate text-[12px] text-zinc-200">
              {task.title}
            </span>
            <Badge variant="red">bloqueada</Badge>
            {task.reason ? (
              <span className="max-w-[45%] truncate text-[10px] text-red-200/70">
                {task.reason}
              </span>
            ) : null}
            <Link
              to={links.task(task.id)}
              className="text-[11px] text-zinc-400 underline-offset-2 hover:text-zinc-200 hover:underline"
            >
              resolver
            </Link>
          </div>
        ))}

        {data.failed_runs.map((run) => (
          <div
            key={`failed-${run.id}`}
            className="flex flex-wrap items-center gap-2 rounded-lg border border-red-500/20 bg-zinc-950/40 px-3 py-2"
          >
            <AlertTriangle size={13} className="text-red-300" />
            <span className="min-w-0 flex-1 truncate text-[12px] text-zinc-200">
              {run.title ?? "Execução"}
            </span>
            <Badge variant="red">falhou 24h</Badge>
            {run.error ? (
              <span className="max-w-[45%] truncate text-[10px] text-red-200/70">
                {run.error}
              </span>
            ) : null}
            <Button
              size="sm"
              variant="outline"
              className="h-7"
              onClick={() => void retryOther(run.id)}
            >
              Retry com outro modelo
            </Button>
            <Link
              to={links.run(run.id)}
              className="inline-flex items-center gap-1 text-[11px] text-zinc-400 underline-offset-2 hover:text-zinc-200 hover:underline"
            >
              <ExternalLink size={10} />
              logs
            </Link>
          </div>
        ))}

        {data.requests.length +
          data.hitl.length +
          data.awaiting_approval.length +
          data.blocked.length +
          data.failed_runs.length ===
        0 ? (
          <p className="flex items-center gap-1 text-[11px] text-emerald-300">
            <CheckCircle2 size={12} />
            Nada pendente.
          </p>
        ) : null}
      </CardContent>
    </Card>
  );
}

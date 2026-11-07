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

interface Attention {
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
  const [data, setData] = useState<Attention | null>(null);

  const load = useCallback(async () => {
    if (!workspaceId) {
      setData(null);
      return;
    }
    try {
      setData(
        await apiGet<Attention>(
          `/api/attention?workspace_id=${encodeURIComponent(workspaceId)}`,
        ),
      );
    } catch {
      setData(null);
    }
  }, [workspaceId]);

  useEffect(() => {
    void load();
  }, [load, lastTaskEvent?.task_id, lastRunEvent?.run_id, lastRunEvent?.status]);

  if (
    !data ||
    (data.awaiting_approval.length === 0 &&
      data.blocked.length === 0 &&
      data.failed_runs.length === 0)
  ) {
    return null;
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
            {data.awaiting_approval.length + data.blocked.length + data.failed_runs.length}
          </Badge>
          <Button variant="ghost" size="sm" className="ml-auto h-7" onClick={() => void load()}>
            atualizar
          </Button>
        </div>

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
            <Link
              to={links.run(run.id)}
              className="inline-flex items-center gap-1 text-[11px] text-zinc-400 underline-offset-2 hover:text-zinc-200 hover:underline"
            >
              <ExternalLink size={10} />
              logs
            </Link>
          </div>
        ))}

        {data.awaiting_approval.length +
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

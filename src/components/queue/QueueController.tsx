import {
  AlertTriangle,
  ArrowDown,
  ArrowUp,
  Clock3,
  FolderOpen,
  ListChecks,
  Pause,
  Play,
  RotateCw,
  Square,
} from "lucide-react";
import { openPath } from "@tauri-apps/plugin-opener";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { CountPill } from "@/components/ui/count-pill";
import { IconLabel } from "@/components/ui/icon-label";
import { EmptyState } from "@/components/ui/empty-state";
import { EntityLink } from "@/components/ui/entity-link";
import { links } from "@/lib/links";
import { relativeTime } from "@/lib/format";
import type { QueueJob, QueueStatus } from "@/lib/types";

function leaseLabel(value: string): string {
  const remaining = Math.round((new Date(value).getTime() - Date.now()) / 1000);
  if (!Number.isFinite(remaining)) return "—";
  return remaining > 0 ? `${remaining}s` : "expirado";
}

const STATUS_ORDER: Record<string, number> = {
  RUNNING: 0,
  QUEUED: 1,
  PAUSED: 2,
  FAILED: 3,
  CANCELLED: 4,
  COMPLETED: 5,
};

export function QueueController({
  queue,
  status,
  loading,
  onRefresh,
  onTogglePause,
  onConcurrency,
  onJobAction,
  onPriority,
}: {
  queue: QueueJob[];
  status: QueueStatus | null;
  loading: boolean;
  onRefresh: () => void;
  onTogglePause: (pause: boolean) => void;
  onConcurrency: (value: number) => void;
  onJobAction: (id: string, action: "cancel" | "retry") => void;
  onPriority: (id: string, priority: number) => void;
}) {
  const running = queue.filter((job) => job.status === "RUNNING").length;
  const waiting = queue.filter(
    (job) => job.status === "QUEUED" || job.status === "PAUSED",
  ).length;
  const failed = queue.filter((job) => job.status === "FAILED").length;
  const sorted = [...queue].sort((left, right) => {
    const order = (STATUS_ORDER[left.status] ?? 9) - (STATUS_ORDER[right.status] ?? 9);
    if (order !== 0) return order;
    return left.enqueued_at.localeCompare(right.enqueued_at);
  });

  return (
    <Card>
      <CardHeader>
        <div>
          <CardTitle className="flex items-center gap-2">
            <ListChecks size={15} className="text-zinc-500" />
            Controlador da fila
          </CardTitle>
          <p className="mt-1 text-xs text-zinc-500">
            Jobs persistem no SQLite e recuperam após reinício. Lease de 60s com
            heartbeat de 5s.
          </p>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <label className="flex items-center gap-2 text-[10px] text-zinc-500">
            concorrência
            <select
              value={status?.concurrency ?? 1}
              onChange={(event) => onConcurrency(Number(event.target.value))}
              className="h-7 rounded-md border border-zinc-800 bg-zinc-900/60 px-2 text-[10px] text-zinc-200 outline-none focus:border-blue-500/60"
            >
              {[1, 2, 3, 4].map((value) => (
                <option key={value} value={value}>
                  {value}
                </option>
              ))}
            </select>
          </label>
          {failed > 0 ? (
            <Button
              variant="outline"
              size="sm"
              onClick={() =>
                queue
                  .filter((job) => job.status === "FAILED")
                  .forEach((job) => onJobAction(job.id, "retry"))
              }
            >
              <RotateCw size={13} />
              Retry falhas
            </Button>
          ) : null}
          <Button
            variant="outline"
            size="sm"
            onClick={() => onTogglePause(!(status?.paused ?? false))}
          >
            {status?.paused ? <Play size={13} /> : <Pause size={13} />}
            {status?.paused ? "Retomar" : "Pausar"}
          </Button>
          <Button variant="ghost" size="sm" onClick={onRefresh} disabled={loading}>
            <RotateCw size={13} className={loading ? "animate-spin" : undefined} />
            Atualizar
          </Button>
        </div>
      </CardHeader>
      <CardContent className="space-y-2">
        <div className="flex flex-wrap items-center gap-x-4 gap-y-1 rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2 text-[10px] text-zinc-500">
          <span className="inline-flex items-center gap-1.5">
            rodando <CountPill value={running} tone="blue" />
          </span>
          <span className="inline-flex items-center gap-1.5">
            aguardando <CountPill value={waiting} tone="yellow" />
          </span>
          <span className="inline-flex items-center gap-1.5">
            falhas <CountPill value={failed} tone={failed > 0 ? "red" : "default"} />
          </span>
          <span className="inline-flex items-center gap-1.5">
            total <CountPill value={queue.length} />
          </span>
          <span className="ml-auto hidden font-mono text-[9px] text-zinc-600 md:inline">
            lease 60s · heartbeat 5s
          </span>
        </div>

        {status?.pause_reason ? (
          <p className="rounded-lg border border-amber-500/20 bg-amber-500/10 px-3 py-2 text-[10px] text-amber-200">
            Pausada: {status.pause_reason}
          </p>
        ) : null}

        {sorted.length === 0 ? (
          <EmptyState
            icon={ListChecks}
            title="Fila vazia"
            description="Execute uma tarefa para enfileirar um job. A fila persiste no SQLite e recupera jobs interrompidos após reinício."
          />
        ) : (
          <div className="space-y-1.5">
            {sorted.map((job) => (
              <div
                key={job.id}
                className="rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-2.5 py-2"
              >
                <div className="flex flex-wrap items-center gap-1.5">
                  <Badge
                    variant={
                      job.status === "RUNNING"
                        ? "blue"
                        : job.status === "FAILED"
                          ? "red"
                          : job.status === "COMPLETED"
                            ? "green"
                            : job.status === "CANCELLED"
                              ? "default"
                              : "yellow"
                    }
                  >
                    {job.status}
                  </Badge>
                  {job.job_kind && job.job_kind !== "task" ? (
                    <Badge variant="purple">{job.job_kind}</Badge>
                  ) : null}
                  <div className="min-w-0 flex-1">
                    <p className="truncate text-[11px] text-zinc-200">
                      {job.title ?? job.id}
                    </p>
                    <p className="truncate font-mono text-[9px] text-zinc-500">
                      {job.resolved_agent} · {job.resolved_model}
                      {job.concurrency_group ? ` · ${job.concurrency_group}` : ""}
                      {(job.retry_count ?? 0) > 0
                        ? ` · retry ${job.retry_count}/${job.max_retries ?? "?"}`
                        : ""}
                    </p>
                  </div>
                  <span className="inline-flex items-center gap-1 font-mono text-[9px] text-zinc-600">
                    <Clock3 size={9} />
                    {relativeTime(job.started_at ?? job.enqueued_at)}
                  </span>
                  {job.status === "RUNNING" && job.lease_expires_at ? (
                    <span
                      className="font-mono text-[9px] text-zinc-600"
                      title={`heartbeat: ${job.last_heartbeat_at ?? "—"}`}
                    >
                      lease {leaseLabel(job.lease_expires_at)}
                    </span>
                  ) : null}
                </div>

                {job.error_message ? (
                  <p className="mt-1 flex items-start gap-1 text-[9px] text-red-300/80">
                    <AlertTriangle size={10} className="mt-0.5 shrink-0" />
                    <span className="min-w-0 break-words">{job.error_message}</span>
                  </p>
                ) : null}

                <div className="mt-1.5 flex flex-wrap items-center gap-1">
                  {job.task_id ? (
                    <EntityLink to={links.task(job.task_id)} chip>
                      tarefa
                    </EntityLink>
                  ) : null}
                  {job.output_dir ? (
                    <Button
                      variant="ghost"
                      size="sm"
                      className="h-6 px-1.5 text-[9px]"
                      title={job.output_dir}
                      onClick={() => void openPath(job.output_dir ?? "")}
                    >
                      <IconLabel icon={<FolderOpen size={10} />}>pasta</IconLabel>
                    </Button>
                  ) : null}
                  <span className="ml-auto flex items-center gap-1">
                    {job.status === "QUEUED" || job.status === "PAUSED" ? (
                      <>
                        <Button
                          variant="ghost"
                          size="sm"
                          className="h-6 px-1.5"
                          onClick={() => onPriority(job.id, Math.min(4, job.priority + 1))}
                          title="Aumentar prioridade"
                        >
                          <ArrowUp size={10} />
                        </Button>
                        <Button
                          variant="ghost"
                          size="sm"
                          className="h-6 px-1.5"
                          onClick={() => onPriority(job.id, Math.max(1, job.priority - 1))}
                          title="Diminuir prioridade"
                        >
                          <ArrowDown size={10} />
                        </Button>
                      </>
                    ) : null}
                    {job.status === "RUNNING" || job.status === "QUEUED" ? (
                      <Button
                        variant="destructive"
                        size="sm"
                        className="h-6 px-1.5 text-[9px]"
                        onClick={() => onJobAction(job.id, "cancel")}
                      >
                        <IconLabel icon={<Square size={10} />}>Cancelar</IconLabel>
                      </Button>
                    ) : null}
                    {job.status === "FAILED" || job.status === "CANCELLED" ? (
                      <Button
                        variant="outline"
                        size="sm"
                        className="h-6 px-1.5 text-[9px]"
                        onClick={() => onJobAction(job.id, "retry")}
                      >
                        <IconLabel icon={<RotateCw size={10} />}>Retry</IconLabel>
                      </Button>
                    ) : null}
                  </span>
                </div>
              </div>
            ))}
          </div>
        )}
      </CardContent>
    </Card>
  );
}

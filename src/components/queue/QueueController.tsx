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
      <CardContent className="space-y-3">
        <div className="grid grid-cols-2 gap-2 sm:grid-cols-4">
          {[
            ["rodando", running, "agora"],
            ["aguardando", waiting, "na fila"],
            ["falhas", failed, "para revisar"],
            ["total", queue.length, "no workspace"],
          ].map(([label, value, hint]) => (
            <div
              key={label}
              className="rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2"
            >
              <p className="text-[9px] uppercase tracking-wide text-zinc-600">{label}</p>
              <p className="mt-0.5 font-mono text-[16px] text-zinc-200">{value}</p>
              <p className="text-[9px] text-zinc-600">{hint}</p>
            </div>
          ))}
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
          <div className="space-y-2">
            {sorted.map((job) => (
              <div
                key={job.id}
                className="rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2.5"
              >
                <div className="flex flex-wrap items-center gap-2">
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
                  {job.concurrency_group ? (
                    <Badge variant="default">{job.concurrency_group}</Badge>
                  ) : null}
                  <div className="min-w-0 flex-1">
                    <p className="truncate text-[11px] text-zinc-200">
                      {job.title ?? job.id}
                    </p>
                    <p className="truncate font-mono text-[9px] text-zinc-600">
                      {job.resolved_mode} · {job.resolved_agent} · {job.resolved_model} ·{" "}
                      {job.resolved_effort}
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
                      <FolderOpen size={10} />
                      pasta
                    </Button>
                  ) : null}
                </div>

                {job.error_message ? (
                  <p className="mt-1 flex items-start gap-1 text-[9px] text-red-300/80">
                    <AlertTriangle size={10} className="mt-0.5 shrink-0" />
                    <span className="min-w-0 break-words">{job.error_message}</span>
                  </p>
                ) : null}

                <div className="mt-2 flex flex-wrap items-center gap-1 border-t border-zinc-800/60 pt-2">
                  {job.status === "QUEUED" || job.status === "PAUSED" ? (
                    <>
                      <Button
                        variant="ghost"
                        size="sm"
                        className="h-6 px-1.5 text-[9px]"
                        onClick={() => onPriority(job.id, Math.min(4, job.priority + 1))}
                        title="Aumentar prioridade"
                      >
                        <ArrowUp size={10} />
                      </Button>
                      <Button
                        variant="ghost"
                        size="sm"
                        className="h-6 px-1.5 text-[9px]"
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
                      <Square size={10} />
                      Cancelar
                    </Button>
                  ) : null}
                  {job.status === "FAILED" || job.status === "CANCELLED" ? (
                    <Button
                      variant="outline"
                      size="sm"
                      className="h-6 px-1.5 text-[9px]"
                      onClick={() => onJobAction(job.id, "retry")}
                    >
                      <RotateCw size={10} />
                      Retry
                    </Button>
                  ) : null}
                </div>
              </div>
            ))}
          </div>
        )}
      </CardContent>
    </Card>
  );
}

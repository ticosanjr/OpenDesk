import type { LucideIcon } from "lucide-react";
import {
  CheckCircle2,
  FileText,
  GripVertical,
  Loader2,
  Play,
  Square,
  Star,
  XCircle,
} from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EntityLink } from "@/components/ui/entity-link";
import { Tooltip } from "@/components/ui/tooltip";
import { WorkingIndicator } from "@/components/ui/working-indicator";
import { links } from "@/lib/links";
import { relativeTime } from "@/lib/format";
import {
  EFFORT_TONE,
  ORIGIN_LABEL,
  PRIORITY_LABEL,
  PRIORITY_TONE,
} from "@/lib/task-ui";
import {
  isTaskAwaitingApproval,
  isTaskFailed,
  taskState,
} from "@/lib/task-state";
import type { Task } from "@/lib/types";
import { cn } from "@/lib/utils";

export function TaskCard({
  task,
  statusLabel,
  statusTone,
  StatusIcon,
  pinned,
  daemonOnline,
  onOpen,
  onDispatch,
  onCancel,
  onApprove,
  onLogs,
  onDragStart,
  onDragEnd,
}: {
  task: Task;
  statusLabel: string;
  statusTone: "default" | "blue" | "green" | "yellow" | "red" | "purple";
  StatusIcon: LucideIcon;
  pinned: number;
  daemonOnline: boolean;
  onOpen: () => void;
  onDispatch: () => void;
  onCancel: () => void;
  onApprove: () => void;
  onLogs: () => void;
  onDragStart: (event: React.DragEvent<HTMLElement>) => void;
  onDragEnd: () => void;
}) {
  const state = taskState(task);
  const running = state === "running";

  const meta = [
    task.assigned_agent ? (
      <EntityLink
        key="agent"
        to={links.agent(task.assigned_agent, "workspace", task.workspace_id)}
        chip
        className="max-w-[88px]"
      >
        @{task.assigned_agent}
      </EntityLink>
    ) : null,
    <Badge key="priority" variant={PRIORITY_TONE[task.priority]}>
      {PRIORITY_LABEL[task.priority] ?? task.priority}
    </Badge>,
    <Badge key="effort" variant={EFFORT_TONE[task.effort_level] ?? "default"}>
      {task.effort_level}
    </Badge>,
    task.origin && task.origin !== "MANUAL" ? (
      <Badge key="origin" variant="purple">
        {ORIGIN_LABEL[task.origin] ?? task.origin}
      </Badge>
    ) : null,
    pinned > 0 ? (
      <Badge key="pinned" variant="yellow">
        <Star size={9} fill="currentColor" />
        {pinned}
      </Badge>
    ) : null,
    task.phase_status ? (
      <Badge
        key="phase"
        variant={
          isTaskFailed(task)
            ? "red"
            : isTaskAwaitingApproval(task) || task.phase_status === "waiting"
              ? "yellow"
              : "default"
        }
        title={task.blocked_reason ?? "Estado detalhado da execução"}
        className="max-w-[96px]"
      >
        <span className="truncate">{task.phase_status}</span>
      </Badge>
    ) : null,
  ].filter(Boolean) as React.ReactNode[];

  return (
    <article
      data-card="task"
      role="button"
      tabIndex={0}
      draggable
      onDragStart={onDragStart}
      onDragEnd={onDragEnd}
      aria-label={`Abrir tarefa ${task.title}`}
      onClick={onOpen}
      onKeyDown={(event) => {
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault();
          onOpen();
        }
      }}
      className={cn(
        "flex h-[152px] flex-col rounded-lg border bg-zinc-950/50 p-2.5 text-left outline-none transition-colors hover:border-zinc-700 focus-visible:ring-2 focus-visible:ring-blue-500/60",
        task.status === "BLOCKED"
          ? "border-red-500/40"
          : state === "running"
            ? "border-blue-500/40"
            : "border-zinc-800/80",
      )}
    >
      <div className="flex min-h-[30px] items-start justify-between gap-2">
        <div className="flex min-w-0 items-start gap-1">
          <span
            title="Arrastar tarefa"
            onClick={(event) => event.stopPropagation()}
            className="mt-0.5 cursor-grab text-zinc-600 active:cursor-grabbing"
          >
            <GripVertical size={12} />
          </span>
          <p className="line-clamp-2 text-[12px] font-semibold leading-snug text-zinc-100">
            {task.title}
          </p>
        </div>
        <Tooltip content={`Status: ${statusLabel} (${task.status})`}>
          <Badge variant={statusTone} className="max-w-[96px] shrink-0">
            <StatusIcon size={9} />
            <span className="truncate">{statusLabel}</span>
          </Badge>
        </Tooltip>
      </div>

      <p className="mt-0.5 line-clamp-1 min-h-[14px] text-[10px] leading-snug text-zinc-500">
        {task.description || "—"}
      </p>

      <div className="mt-1 flex h-5 items-center gap-1 overflow-hidden">
        {meta.slice(0, 3)}
        {meta.length > 3 ? (
          <Badge variant="default" title={meta.length - 3 + " metadados ocultos"}>
            +{meta.length - 3}
          </Badge>
        ) : null}
      </div>

      <div className="mt-auto flex h-7 items-center gap-1 border-t border-zinc-800/60 pt-1.5">
        <div className="flex min-w-0 flex-1 items-center gap-1.5">
          {running ? <WorkingIndicator /> : null}
          {task.last_run_status ? (
            <span
              className={cn(
                "inline-flex min-w-0 items-center gap-1 rounded-md border px-1.5 py-0.5 font-mono text-[9px] font-semibold",
                task.last_run_status === "SUCCESS"
                  ? "border-emerald-500/25 bg-emerald-500/10 text-emerald-300"
                  : task.last_run_status === "FAILED"
                    ? "border-red-500/25 bg-red-500/10 text-red-300"
                    : "border-zinc-700 bg-zinc-900 text-zinc-400",
              )}
              title={`Última execução: ${task.last_run_status} · ${relativeTime(task.last_run_at)}`}
            >
              {task.last_run_status === "SUCCESS" ? (
                <CheckCircle2 size={10} />
              ) : task.last_run_status === "FAILED" ? (
                <XCircle size={10} />
              ) : (
                <Loader2 size={10} />
              )}
              <span className="truncate">{task.last_run_status}</span>
            </span>
          ) : (
            <span className="truncate font-mono text-[9px] text-zinc-600">
              sem execução
            </span>
          )}
        </div>
        <div className="flex shrink-0 items-center gap-1">
          {isTaskAwaitingApproval(task) ? (
            <Button
              size="sm"
              className="h-6 px-1.5 text-[10px]"
              title="Aprovar o plano e iniciar o Build"
              onClick={(event) => {
                event.stopPropagation();
                onApprove();
              }}
            >
              <CheckCircle2 size={11} />
              Aprovar
            </Button>
          ) : running ? (
            <Button
              variant="destructive"
              size="sm"
              className="h-6 px-1.5 text-[10px]"
              title="Cancelar a execução agora"
              onClick={(event) => {
                event.stopPropagation();
                onCancel();
              }}
            >
              <Square size={10} />
              Cancelar
            </Button>
          ) : task.status !== "DONE" && task.status !== "CANCELLED" ? (
            <Button
              variant="outline"
              size="sm"
              className="h-6 px-1.5 text-[10px]"
              title={
                daemonOnline
                  ? "Criar uma nova tentativa de execução"
                  : "OpenCode engine offline: inicie o motor para executar"
              }
              disabled={!daemonOnline}
              onClick={(event) => {
                event.stopPropagation();
                onDispatch();
              }}
            >
              <Play size={11} />
              {task.status === "BLOCKED" ? "Tentar" : "Executar"}
            </Button>
          ) : null}
          <Button
            variant="ghost"
            size="icon"
            className="h-6 w-6"
            aria-label="Abrir logs da tarefa"
            title="Abrir a execução e o log completo"
            onClick={(event) => {
              event.stopPropagation();
              onLogs();
            }}
          >
            <FileText size={11} />
          </Button>
        </div>
      </div>
    </article>
  );
}

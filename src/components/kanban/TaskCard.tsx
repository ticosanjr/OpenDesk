import {
  CheckCircle2,
  FileText,
  FolderOpen,
  Globe,
  GripVertical,
  Play,
  Square,
  Star,
} from "lucide-react";
import { Link } from "react-router-dom";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EntityLink } from "@/components/ui/entity-link";
import { WorkflowStatusBadge } from "@/components/ui/workflow-status";
import { links } from "@/lib/links";
import {
  EFFORT_TONE,
  ORIGIN_LABEL,
  PRIORITY_LABEL,
  PRIORITY_TONE,
  STATUS_LABEL,
} from "@/lib/task-ui";
import { isTaskAwaitingApproval, taskState } from "@/lib/task-state";
import type { Task } from "@/lib/types";
import { cn } from "@/lib/utils";

export function TaskCard({
  task,
  pinned,
  daemonOnline,
  onOpen,
  onDispatch,
  onResume,
  onCancel,
  onApprove,
  onLogs,
  onDragStart,
  onDragEnd,
}: {
  task: Task;
  pinned: number;
  daemonOnline: boolean;
  onOpen: () => void;
  onDispatch: () => void;
  onResume: () => void;
  onCancel: () => void;
  onApprove: () => void;
  onLogs: () => void;
  onDragStart: (event: React.DragEvent<HTMLElement>) => void;
  onDragEnd: () => void;
}) {
  const state = taskState(task);
  const running = state === "running";
  const attention =
    state === "blocked" || state === "needs_approval" || state === "waiting_input";
  const queued = state === "queued";
  const held = state === "held";
  const waitingInput = state === "waiting_input";
  const hasOutput = Boolean(task.output_dir_path || task.last_run_id);
  const webTask = task.tags_json.includes('"needs-browser"');

  const meta = [
    task.assigned_agent ? (
      <EntityLink
        key="agent"
        to={links.agent(task.assigned_agent, "workspace", task.workspace_id)}
        chip
        className="max-w-[88px]"
        title={`Agente responsável: @${task.assigned_agent}`}
      >
        @{task.assigned_agent}
      </EntityLink>
    ) : null,
    <Badge
      key="priority"
      variant={PRIORITY_TONE[task.priority]}
      title={`Prioridade: ${PRIORITY_LABEL[task.priority] ?? task.priority} (${task.priority} de 4)`}
    >
      {PRIORITY_LABEL[task.priority] ?? task.priority}
    </Badge>,
    <Badge
      key="effort"
      variant={EFFORT_TONE[task.effort_level] ?? "default"}
      title={`Effort: ${task.effort_level}. Controla quanto esforço de raciocínio o modelo deve usar.`}
    >
      {task.effort_level}
    </Badge>,
    task.origin && task.origin !== "MANUAL" ? (
      <Badge
        key="origin"
        variant="purple"
        title={`Origem da tarefa: ${ORIGIN_LABEL[task.origin] ?? task.origin}`}
      >
        {ORIGIN_LABEL[task.origin] ?? task.origin}
      </Badge>
    ) : null,
    webTask ? (
      <Badge key="web" variant="blue">
        <Globe size={9} />
        web
      </Badge>
    ) : null,
    pinned > 0 ? (
      <Badge key="pinned" variant="yellow">
        <Star size={9} fill="currentColor" />
        {pinned}
      </Badge>
    ) : null,
  ].filter(Boolean) as React.ReactNode[];

  return (
    <article
      data-card="task"
      data-workflow-state={task.workflow_state ?? "UNKNOWN"}
      data-workflow-reason={task.workflow_reason ?? ""}
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
        "flex min-h-[176px] flex-col gap-1.5 rounded-lg border bg-zinc-950/50 p-3 text-left outline-none transition-colors hover:border-zinc-700 hover:bg-zinc-900/40 focus-visible:ring-2 focus-visible:ring-blue-500/60",
        attention
          ? "border-amber-500/30"
          : state === "running"
            ? "border-blue-500/35"
            : queued
              ? "border-amber-500/20"
              : "border-zinc-800/80",
      )}
    >
      <div className="flex items-start gap-1.5">
        <span
          title="Arrastar tarefa"
          onClick={(event) => event.stopPropagation()}
          className="mt-0.5 cursor-grab text-zinc-600 transition-colors hover:text-zinc-400 active:cursor-grabbing"
        >
          <GripVertical size={13} />
        </span>
        <p className="line-clamp-2 min-w-0 flex-1 text-[13.5px] font-semibold leading-snug text-zinc-100">
          {task.title}
        </p>
      </div>

      <div className="flex flex-wrap items-center gap-1.5">
        <WorkflowStatusBadge task={task} />
        {meta.slice(0, 2)}
        {meta.length > 2 ? (
          <Badge variant="default" title={`${meta.length - 2} metadados ocultos`}>
            +{meta.length - 2}
          </Badge>
        ) : null}
      </div>

      <p className="line-clamp-3 text-[12px] leading-relaxed text-zinc-400">
        {task.description || "—"}
      </p>

      <div className="mt-auto flex min-h-8 items-center gap-1 border-t border-zinc-800/60 pt-2">
        <div className="flex min-w-0 flex-1 items-center gap-1.5">
          {hasOutput ? (
            <Link
              to={`/artifacts?task=${encodeURIComponent(task.id)}`}
              onClick={(event) => event.stopPropagation()}
              title="Ver arquivos gerados por esta tarefa"
              className="inline-flex items-center gap-1.5 rounded-md border border-zinc-800/80 bg-zinc-900/50 px-2 py-1 text-[11px] text-zinc-400 transition-colors hover:border-blue-500/40 hover:text-blue-200"
            >
              <FolderOpen size={12} />
              arquivos
            </Link>
          ) : attention ? (
            <span className="truncate text-[11px] text-amber-300/90">
              {task.workflow_reason === "in_progress_without_run"
                ? "sem execução ativa"
                : "precisa de atenção"}
            </span>
          ) : task.status === "DONE" || task.status === "CANCELLED" ? (
            <span className="truncate text-[11px] text-zinc-500">
              {STATUS_LABEL[task.status] ?? task.status}
            </span>
          ) : null}
        </div>
        <div className="flex shrink-0 items-center gap-1">
          {isTaskAwaitingApproval(task) ? (
            <Button
              size="xs"
              className="px-2"
              title="Aprovar o plano e iniciar o Build"
              onClick={(event) => {
                event.stopPropagation();
                onApprove();
              }}
            >
              <CheckCircle2 size={12} />
              Aprovar
            </Button>
          ) : held ? (
            <Button
              variant="outline"
              size="xs"
              className="px-2"
              title="Retomar e entrar na fila automaticamente"
              onClick={(event) => {
                event.stopPropagation();
                onResume();
              }}
            >
              <Play size={12} />
              Retomar
            </Button>
          ) : running || queued ? (
            <Button
              variant="destructive"
              size="xs"
              className="px-2"
              title={queued ? "Remover da fila" : "Cancelar a execução agora"}
              onClick={(event) => {
                event.stopPropagation();
                onCancel();
              }}
            >
              <Square size={11} />
              Cancelar
            </Button>
          ) : waitingInput ? (
            <Button
              variant="outline"
              size="xs"
              className="px-2"
              title="Abrir a tarefa para responder ao agente"
              onClick={(event) => {
                event.stopPropagation();
                onOpen();
              }}
            >
              Responder
            </Button>
          ) : task.status !== "DONE" && task.status !== "CANCELLED" ? (
            <Button
              variant="outline"
              size="xs"
              className="px-2"
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
              <Play size={12} />
              {task.status === "BLOCKED" ? "Tentar" : "Executar"}
            </Button>
          ) : null}
          <Button
            variant="ghost"
            size="xs"
            className="w-7 px-0"
            aria-label="Abrir logs da tarefa"
            title="Abrir a execução e o log completo"
            onClick={(event) => {
              event.stopPropagation();
              onLogs();
            }}
          >
            <FileText size={12} />
          </Button>
        </div>
      </div>
    </article>
  );
}

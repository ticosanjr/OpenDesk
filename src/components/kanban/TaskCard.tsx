import { useState } from "react";
import {
  AlertTriangle,
  ArrowUpRight,
  ClipboardCheck,
  FileText,
  FolderOpen,
  Globe,
  GripVertical,
  MessageCircleQuestion,
  Play,
  ShieldAlert,
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
import { workflowMeta } from "@/lib/workflow";
import type { Task, TaskInteraction } from "@/lib/types";
import { cn } from "@/lib/utils";
import {
  TaskAttentionModal,
  type TaskPermissionRequest,
} from "@/components/kanban/task-attention-modal";

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
  interactions,
  permissions,
  onAttentionResolved,
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
  interactions?: TaskInteraction[];
  permissions?: TaskPermissionRequest[];
  onAttentionResolved?: () => void;
}) {
  const [attentionOpen, setAttentionOpen] = useState(false);
  const interaction = interactions?.[0];
  const permission = permissions?.[0];
  const state = taskState(task);
  const workflow = workflowMeta(task);
  const running = state === "running";
  const attention =
    state === "blocked" ||
    state === "needs_approval" ||
    state === "waiting_input" ||
    Boolean(interaction) ||
    Boolean(permission);
  const badgeTask =
    interaction || permission
      ? {
          ...task,
          workflow_state: "WAITING_INPUT",
          workflow_reason: permission ? "permission" : "hitl",
        }
      : task;
  const queued = state === "queued";
  const held = state === "held";
  const waitingInput = state === "waiting_input";
  const hasOutput = Boolean(task.output_dir_path || task.last_run_id);
  const webTask = task.tags_json.includes('"needs-browser"');

  const attentionInfo = interaction
    ? {
        icon: MessageCircleQuestion,
        title: "O agente perguntou",
        detail: interaction.question,
        action: "Responder",
        run: () => setAttentionOpen(true),
      }
    : permission
      ? {
          icon: ShieldAlert,
          title: `Permissão para ${permission.permission_action}`,
          detail:
            permission.resources ||
            "O agente precisa de acesso para continuar a execução.",
          action: "Responder",
          run: () => setAttentionOpen(true),
        }
      : isTaskAwaitingApproval(task)
        ? {
            icon: ClipboardCheck,
            title: "Plano aguardando aprovação",
            detail: workflow.description,
            action: "Aprovar plano",
            run: onApprove,
          }
        : task.workflow_reason === "failed"
          ? {
              icon: AlertTriangle,
              title: "A última execução falhou",
              detail: task.blocked_reason || workflow.description,
              action: "Abrir diagnóstico",
              run: onOpen,
            }
          : state === "blocked"
            ? {
                icon: AlertTriangle,
                title: "Tarefa bloqueada",
                detail: task.blocked_reason || workflow.description,
                action: "Abrir tarefa",
                run: onOpen,
              }
            : waitingInput
              ? {
                  icon: MessageCircleQuestion,
                  title: "Aguardando sua resposta",
                  detail: workflow.description,
                  action: "Abrir tarefa",
                  run: onOpen,
                }
              : null;

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
    <>
    <article
      data-card="task"
      data-workflow-state={task.workflow_state ?? "UNKNOWN"}
      data-workflow-reason={task.workflow_reason ?? ""}
      draggable
      onDragStart={onDragStart}
      onDragEnd={onDragEnd}
      onClick={onOpen}
      className={cn(
        "flex min-h-[176px] flex-col gap-1.5 rounded-lg border bg-zinc-950/50 p-3 text-left transition-colors hover:border-zinc-700 hover:bg-zinc-900/40",
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
        <button
          type="button"
          onClick={(event) => {
            event.stopPropagation();
            onOpen();
          }}
          className="line-clamp-2 min-w-0 flex-1 rounded text-left text-[13.5px] font-semibold leading-snug text-zinc-100 outline-none focus-visible:ring-2 focus-visible:ring-blue-500/60"
        >
          {task.title}
        </button>
      </div>

      <div className="flex flex-wrap items-center gap-1.5">
        <WorkflowStatusBadge task={badgeTask} />
        {meta.slice(0, 2)}
        {meta.length > 2 ? (
          <Badge variant="default" title={`${meta.length - 2} metadados ocultos`}>
            +{meta.length - 2}
          </Badge>
        ) : null}
      </div>

      <p className="line-clamp-3 text-[13.5px] leading-relaxed text-zinc-400">
        {task.description || "—"}
      </p>

      {attentionInfo ? (
        <div
          data-task-attention
          className="rounded-lg border border-amber-500/25 bg-amber-500/[0.055] p-2.5"
        >
          <div className="flex items-start gap-2">
            <attentionInfo.icon
              size={14}
              className="mt-0.5 shrink-0 text-amber-300"
            />
            <div className="min-w-0 flex-1">
              <p className="text-[11.5px] font-semibold text-amber-100">
                {attentionInfo.title}
              </p>
              <p className="mt-0.5 line-clamp-2 text-[13px] leading-snug text-amber-100/70">
                {attentionInfo.detail}
              </p>
            </div>
          </div>
          <div className="mt-2 flex justify-end">
            <Button
              size="xs"
              className="px-2"
              onClick={(event) => {
                event.stopPropagation();
                attentionInfo.run();
              }}
            >
              {attentionInfo.action}
            </Button>
          </div>
        </div>
      ) : null}

      <div className="mt-auto flex min-h-8 items-center gap-1 border-t border-zinc-800/60 pt-2">
        <div className="flex min-w-0 flex-1 items-center gap-1.5">
          {hasOutput ? (
            <Link
              to={`/artifacts?task=${encodeURIComponent(task.id)}`}
              onClick={(event) => event.stopPropagation()}
              title="Ver arquivos gerados por esta tarefa"
              aria-label="Arquivos"
              className="inline-flex h-7 w-7 items-center justify-center rounded-md border border-zinc-800/80 bg-zinc-900/50 text-zinc-400 transition-colors hover:border-blue-500/40 hover:text-blue-200"
            >
              <FolderOpen size={13} />
            </Link>
          ) : task.status === "DONE" || task.status === "CANCELLED" ? (
            <span className="truncate text-[13px] text-zinc-500">
              {STATUS_LABEL[task.status] ?? task.status}
            </span>
          ) : null}
        </div>
        <div className="flex shrink-0 items-center gap-1">
          {held ? (
            <Button
              variant="outline"
              size="xs"
              className="h-7 w-7 px-0"
              title="Retomar e entrar na fila automaticamente"
              aria-label="Retomar"
              onClick={(event) => {
                event.stopPropagation();
                onResume();
              }}
            >
              <Play size={12} />
            </Button>
          ) : running || queued ? (
            <Button
              variant="destructive"
              size="xs"
              className="h-7 w-7 px-0"
              title={queued ? "Remover da fila" : "Cancelar a execução agora"}
              aria-label="Cancelar"
              onClick={(event) => {
                event.stopPropagation();
                onCancel();
              }}
            >
              <Square size={11} />
            </Button>
          ) : task.status !== "DONE" && task.status !== "CANCELLED" ? (
            <Button
              variant="outline"
              size="xs"
              className="h-7 w-7 px-0"
              title={
                daemonOnline
                  ? "Criar uma nova tentativa de execução"
                  : "OpenCode engine offline: inicie o motor para executar"
              }
              aria-label={task.status === "BLOCKED" ? "Tentar" : "Executar"}
              disabled={!daemonOnline}
              onClick={(event) => {
                event.stopPropagation();
                onDispatch();
              }}
            >
              <Play size={12} />
            </Button>
          ) : null}
          <Button
            variant="ghost"
            size="xs"
            className="w-7 px-0"
            aria-label={`Abrir tarefa ${task.title}`}
            title="Abrir tarefa"
            onClick={(event) => {
              event.stopPropagation();
              onOpen();
            }}
          >
            <ArrowUpRight size={13} />
          </Button>
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
    <TaskAttentionModal
      open={attentionOpen}
      task={task}
      interactions={interactions ?? []}
      permissions={permissions ?? []}
      onClose={() => setAttentionOpen(false)}
      onResolved={() => onAttentionResolved?.()}
      onOpenTask={() => {
        setAttentionOpen(false);
        onOpen();
      }}
    />
    </>
  );
}

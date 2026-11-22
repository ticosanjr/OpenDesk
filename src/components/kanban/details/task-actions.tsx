import {
  Ban,
  CheckCircle2,
  Columns3,
  FileText,
  FolderOpen,
  GitFork,
  Pause,
  Pencil,
  Play,
  RotateCw,
  Square,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { isTaskAwaitingApproval } from "@/lib/task-state";
import type { Task } from "@/lib/types";

export function TaskDetailActions({
  task,
  running,
  queued,
  held,
  daemonOnline,
  runCount,
  onOpenArtifacts,
  onFork,
  onSwarm,
  onEdit,
  onLogs,
  onStatus,
  onWait,
  onApprove,
  onDispatch,
}: {
  task: Task;
  running: boolean;
  queued: boolean;
  held: boolean;
  daemonOnline: boolean;
  runCount: number;
  onOpenArtifacts: () => void;
  onFork: (task: Task) => void;
  onSwarm: (task: Task) => void;
  onEdit: (task: Task) => void;
  onLogs: (task: Task) => void;
  onStatus: (task: Task, status: string) => void;
  onWait: (task: Task) => void;
  onApprove: (task: Task) => void;
  onDispatch: (task: Task) => void;
}) {
  return (
    <div className="sticky bottom-0 z-10 -mx-6 -mb-6 mt-4 flex flex-wrap items-center justify-center gap-2 border-t border-zinc-800/70 bg-zinc-950/95 px-6 py-3 backdrop-blur">
      {task.output_dir_path || runCount ? (
        <Button
          variant="outline"
          size="sm"
          onClick={onOpenArtifacts}
          title="Abrir os arquivos gerados por esta tarefa"
        >
          <FolderOpen size={13} />
          Ver arquivos
        </Button>
      ) : null}
      <Button
        variant="outline"
        size="sm"
        onClick={() => onFork(task)}
        title="Revisar e criar uma cópia independente desta tarefa"
      >
        <GitFork size={13} />
        Fork
      </Button>
      <Button
        variant="outline"
        size="sm"
        onClick={() => onSwarm(task)}
        title="Rodar a mesma tarefa em 2 modelos e comparar os entregáveis"
      >
        <Columns3 size={13} />
        A/B
      </Button>
      <Button variant="outline" size="sm" onClick={() => onEdit(task)}>
        <Pencil size={13} />
        Editar
      </Button>
      <Button variant="outline" size="sm" onClick={() => onLogs(task)}>
        <FileText size={13} />
        Execução
      </Button>
      {task.status !== "CANCELLED" && task.status !== "DONE" ? (
        <Button
          variant="destructive"
          size="sm"
          onClick={() => onStatus(task, "CANCELLED")}
          title="Cancelar imediatamente e interromper a execução"
        >
          <Square size={12} />
          Cancelar
        </Button>
      ) : null}
      {task.status !== "BLOCKED" && task.status !== "DONE" ? (
        <Button
          variant="outline"
          size="sm"
          onClick={() => onStatus(task, "BLOCKED")}
          title="Bloquear a tarefa e interromper a execução"
        >
          <Ban size={12} />
          Bloquear
        </Button>
      ) : null}
      {!held && (task.status === "IN_PROGRESS" || task.status === "TODO") ? (
        <Button
          variant="outline"
          size="sm"
          onClick={() => onWait(task)}
          title="Colocar em espera e impedir o início automático"
        >
          <Pause size={12} />
          Em espera
        </Button>
      ) : null}
      {held ? (
        <Button
          variant="outline"
          size="sm"
          onClick={() => onStatus(task, "TODO")}
          title="Retomar e entrar na fila automaticamente"
        >
          <Play size={12} />
          Retomar
        </Button>
      ) : null}
      {task.status === "BLOCKED" || task.status === "CANCELLED" || task.status === "DONE" ? (
        <Button
          variant="outline"
          size="sm"
          onClick={() => onStatus(task, "TODO")}
          title="Reabrir a tarefa como To do"
        >
          <RotateCw size={12} />
          Reabrir
        </Button>
      ) : null}
      {task.status !== "DONE" ? (
        <Button
          variant="outline"
          size="sm"
          onClick={() => onStatus(task, "DONE")}
          title="Marcar como concluída sem executar"
        >
          <CheckCircle2 size={12} />
          Concluir
        </Button>
      ) : null}
      {isTaskAwaitingApproval(task) ? (
        <Button size="sm" onClick={() => onApprove(task)}>
          <CheckCircle2 size={13} />
          Aprovar plano e iniciar Build
        </Button>
      ) : !running && !queued && !held ? (
        <Button
          size="sm"
          onClick={() => onDispatch(task)}
          disabled={!daemonOnline}
          title={
            daemonOnline
              ? "Executar agora"
              : "OpenCode engine offline: inicie o motor para executar"
          }
        >
          <Play size={13} />
          {task.status === "BLOCKED" ? "Nova tentativa" : "Executar"}
        </Button>
      ) : null}
    </div>
  );
}

import { AlertTriangle, ArrowRight } from "lucide-react";
import { InteractionCard, PermissionRequestCard } from "@/components/interactions/InteractionCard";
import { Button } from "@/components/ui/button";
import { Modal } from "@/components/ui/modal";
import type { Task, TaskInteraction } from "@/lib/types";

export interface TaskPermissionRequest {
  request_id: string;
  permission_action: string;
  resources?: string | null;
  task_id?: string | null;
  run_id?: string | null;
}

/**
 * Focused HITL surface for a Kanban card. The card shows what is pending; this
 * dialog carries the full question, options and permission decisions.
 */
export function TaskAttentionModal({
  open,
  task,
  interactions,
  permissions,
  onClose,
  onResolved,
  onOpenTask,
}: {
  open: boolean;
  task: Task;
  interactions: TaskInteraction[];
  permissions: TaskPermissionRequest[];
  onClose: () => void;
  onResolved: () => void;
  onOpenTask: () => void;
}) {
  if (!open) return null;
  const empty = interactions.length === 0 && permissions.length === 0;
  const resolved = () => {
    onResolved();
    onClose();
  };

  return (
    <Modal
      open={open}
      onClose={onClose}
      title="Precisa da sua atenção"
      description={task.title}
      size="md"
    >
      <div className="space-y-3">
        {empty ? (
          <div className="flex items-start gap-2.5 rounded-lg border border-amber-500/20 bg-amber-500/[0.04] px-3 py-2.5 text-[13.5px] text-amber-100">
            <AlertTriangle size={15} className="mt-0.5 shrink-0 text-amber-300" />
            <p>
              A pendência ainda não chegou ao OpenDesk. Abra a tarefa para
              atualizar o estado e ver o diagnóstico completo.
            </p>
          </div>
        ) : null}
        {interactions.map((interaction) => (
          <InteractionCard
            key={interaction.id}
            interaction={interaction}
            onResolved={resolved}
          />
        ))}
        {permissions.map((request) => (
          <PermissionRequestCard
            key={request.request_id}
            request={request}
            onDecided={resolved}
          />
        ))}
        {empty ? (
          <div className="flex justify-end">
            <Button size="sm" onClick={onOpenTask}>
              Abrir tarefa
              <ArrowRight size={13} />
            </Button>
          </div>
        ) : null}
      </div>
    </Modal>
  );
}

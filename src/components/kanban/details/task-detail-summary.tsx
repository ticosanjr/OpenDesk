import { Badge } from "@/components/ui/badge";
import { EntityLink } from "@/components/ui/entity-link";
import { WorkflowStatusBadge } from "@/components/ui/workflow-status";
import {
  InteractionCard,
  PermissionRequestCard,
} from "@/components/interactions/InteractionCard";
import { links } from "@/lib/links";
import type { Task, TaskInteraction } from "@/lib/types";

export interface TaskPermissionEntry {
  request_id: string;
  permission_action: string;
  resources?: string | null;
  task_id?: string | null;
  run_id?: string | null;
  created_at?: string;
}

export function TaskDetailSummary({
  task,
  interactions,
  permissions,
  onReload,
}: {
  task: Task;
  interactions: TaskInteraction[];
  permissions: TaskPermissionEntry[];
  onReload: () => void;
}) {
  return (
    <>
      <div className="flex flex-wrap items-center gap-2">
        <WorkflowStatusBadge task={task} showReason />
        <Badge
          variant="default"
          title={`Prioridade: ${task.priority} de 4 (1 é a mais alta).`}
        >
          prioridade {task.priority}
        </Badge>
        {task.assigned_agent ? (
          <EntityLink
            to={links.agent(task.assigned_agent, "workspace", task.workspace_id)}
            chip
            title={`Agente responsável: @${task.assigned_agent}`}
          >
            @{task.assigned_agent}
          </EntityLink>
        ) : (
          <Badge variant="default" title="O roteador escolhe o agente em tempo de execução.">
            agente automático
          </Badge>
        )}
        {task.preferred_model ? (
          <Badge
            variant="default"
            title="Modelo fixado nesta tarefa. O roteador não troca este modelo."
          >
            {task.preferred_model}
          </Badge>
        ) : (
          <Badge variant="default" title="O roteador escolhe o modelo conforme a política.">
            modelo automático
          </Badge>
        )}
      </div>

      {interactions.map((interaction) => (
        <InteractionCard
          key={interaction.id}
          interaction={interaction}
          onResolved={() => onReload()}
        />
      ))}
      {permissions.map((request) => (
        <PermissionRequestCard
          key={request.request_id}
          request={request}
          onDecided={() => onReload()}
        />
      ))}
    </>
  );
}

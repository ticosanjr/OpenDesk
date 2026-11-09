import { useEffect, useState } from "react";
import { Loader2, Target } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Modal } from "@/components/ui/modal";
import type { AgentGoal, GovernanceAgent } from "@/lib/types";
import { cn } from "@/lib/utils";
import {
  fieldInputClass as inputClass,
  fieldLabelClass as labelClass,
} from "@/lib/ui-classes";
import { emptyGoal, type GoalValues } from "./types";

export function GoalModal({
  state,
  agents,
  saving,
  onClose,
  onSave,
}: {
  state: { goal?: AgentGoal } | null;
  agents: GovernanceAgent[];
  saving: boolean;
  onClose: () => void;
  onSave: (
    input: GoalValues,
    goal: AgentGoal | undefined,
    close: () => void,
  ) => void;
}) {
  const goal = state?.goal;
  const [values, setValues] = useState<GoalValues>(emptyGoal());
  useEffect(() => {
    if (!state) return;
    setValues(
      goal
        ? {
            agentId: goal.agent_id,
            title: goal.title,
            description: goal.description ?? "",
            metric: goal.metric ?? "",
            target:
              goal.target_value !== null && goal.target_value !== undefined
                ? String(goal.target_value)
                : "",
            current: String(goal.current_value),
            unit: goal.unit ?? "",
            dueAt: goal.due_at ? goal.due_at.slice(0, 10) : "",
            status: goal.status,
          }
        : { ...emptyGoal(), agentId: agents[0]?.id ?? "" },
    );
  }, [state, goal, agents]);
  return (
    <Modal
      open={state !== null}
      onClose={onClose}
      title={goal ? "Editar meta" : "Nova meta"}
      description="Metas orientam heartbeats e a revisão de performance do agente."
      className="max-w-lg"
    >
      <div className="space-y-3">
        <div className="grid gap-2 md:grid-cols-2">
          <label>
            <span className={labelClass}>Agente</span>
            <select
              value={values.agentId}
              onChange={(event) =>
                setValues({ ...values, agentId: event.target.value })
              }
              className={inputClass}
            >
              {agents.map((agent) => (
                <option key={agent.id} value={agent.id}>
                  @{agent.id}
                </option>
              ))}
            </select>
          </label>
          <label>
            <span className={labelClass}>Status</span>
            <select
              value={values.status}
              onChange={(event) =>
                setValues({ ...values, status: event.target.value })
              }
              className={inputClass}
            >
              <option value="ACTIVE">Ativa</option>
              <option value="PAUSED">Pausada</option>
              <option value="DONE">Concluída</option>
              <option value="CANCELLED">Cancelada</option>
            </select>
          </label>
        </div>
        <label>
          <span className={labelClass}>Título</span>
          <input
            value={values.title}
            onChange={(event) => setValues({ ...values, title: event.target.value })}
            placeholder="Publicar 4 relatórios"
            className={inputClass}
          />
        </label>
        <label>
          <span className={labelClass}>Descrição</span>
          <textarea
            value={values.description}
            onChange={(event) =>
              setValues({ ...values, description: event.target.value })
            }
            rows={2}
            className={cn(inputClass, "h-auto py-2")}
          />
        </label>
        <div className="grid gap-2 md:grid-cols-4">
          <label className="md:col-span-2">
            <span className={labelClass}>Métrica</span>
            <input
              value={values.metric}
              onChange={(event) =>
                setValues({ ...values, metric: event.target.value })
              }
              placeholder="relatórios"
              className={inputClass}
            />
          </label>
          <label>
            <span className={labelClass}>Atual</span>
            <input
              type="number"
              value={values.current}
              onChange={(event) =>
                setValues({ ...values, current: event.target.value })
              }
              className={inputClass}
            />
          </label>
          <label>
            <span className={labelClass}>Meta</span>
            <input
              type="number"
              value={values.target}
              onChange={(event) =>
                setValues({ ...values, target: event.target.value })
              }
              className={inputClass}
            />
          </label>
        </div>
        <div className="grid gap-2 md:grid-cols-2">
          <label>
            <span className={labelClass}>Unidade</span>
            <input
              value={values.unit}
              onChange={(event) => setValues({ ...values, unit: event.target.value })}
              placeholder="relatórios"
              className={inputClass}
            />
          </label>
          <label>
            <span className={labelClass}>Prazo</span>
            <input
              type="date"
              value={values.dueAt}
              onChange={(event) => setValues({ ...values, dueAt: event.target.value })}
              className={inputClass}
            />
          </label>
        </div>
        <div className="flex justify-end gap-2">
          <Button variant="ghost" size="sm" onClick={onClose}>
            Cancelar
          </Button>
          <Button
            size="sm"
            disabled={saving || !values.title.trim() || !values.agentId}
            onClick={() => onSave(values, goal, onClose)}
          >
            {saving ? (
              <Loader2 size={13} className="animate-spin" />
            ) : (
              <Target size={13} />
            )}
            {goal ? "Salvar meta" : "Criar meta"}
          </Button>
        </div>
      </div>
    </Modal>
  );
}

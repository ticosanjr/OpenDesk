import { useEffect, useState } from "react";
import { Loader2, Target } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Modal } from "@/components/ui/modal";
import type { AgentGoal, GovernanceAgent } from "@/lib/types";
import { Field, Input, Select, Textarea } from "@/components/ui/field";
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
      size="md"
    >
      <div className="space-y-3">
        <div className="grid gap-2 md:grid-cols-2">
          <Field label="Agente" htmlFor="goal-agent">
            <Select
              id="goal-agent"
              value={values.agentId}
              onChange={(event) =>
                setValues({ ...values, agentId: event.target.value })
              }
            >
              {agents.map((agent) => (
                <option key={agent.id} value={agent.id}>
                  @{agent.id}
                </option>
              ))}
            </Select>
          </Field>
          <Field label="Status" htmlFor="goal-status">
            <Select
              id="goal-status"
              value={values.status}
              onChange={(event) =>
                setValues({ ...values, status: event.target.value })
              }
            >
              <option value="ACTIVE">Ativa</option>
              <option value="PAUSED">Pausada</option>
              <option value="DONE">Concluída</option>
              <option value="CANCELLED">Cancelada</option>
            </Select>
          </Field>
        </div>
        <Field label="Título" htmlFor="goal-title">
          <Input
            id="goal-title"
            value={values.title}
            onChange={(event) => setValues({ ...values, title: event.target.value })}
            placeholder="Publicar 4 relatórios"
          />
        </Field>
        <Field label="Descrição" htmlFor="goal-description">
          <Textarea
            id="goal-description"
            value={values.description}
            onChange={(event) =>
              setValues({ ...values, description: event.target.value })
            }
            rows={2}
            className="min-h-0"
          />
        </Field>
        <div className="grid gap-2 md:grid-cols-4">
          <Field label="Métrica" htmlFor="goal-metric" className="md:col-span-2">
            <Input
              id="goal-metric"
              value={values.metric}
              onChange={(event) =>
                setValues({ ...values, metric: event.target.value })
              }
              placeholder="relatórios"
            />
          </Field>
          <Field label="Atual" htmlFor="goal-current">
            <Input
              id="goal-current"
              type="number"
              value={values.current}
              onChange={(event) =>
                setValues({ ...values, current: event.target.value })
              }
            />
          </Field>
          <Field label="Meta" htmlFor="goal-target">
            <Input
              id="goal-target"
              type="number"
              value={values.target}
              onChange={(event) =>
                setValues({ ...values, target: event.target.value })
              }
            />
          </Field>
        </div>
        <div className="grid gap-2 md:grid-cols-2">
          <Field label="Unidade" htmlFor="goal-unit">
            <Input
              id="goal-unit"
              value={values.unit}
              onChange={(event) => setValues({ ...values, unit: event.target.value })}
              placeholder="relatórios"
            />
          </Field>
          <Field label="Prazo" htmlFor="goal-due">
            <Input
              id="goal-due"
              type="date"
              value={values.dueAt}
              onChange={(event) => setValues({ ...values, dueAt: event.target.value })}
            />
          </Field>
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

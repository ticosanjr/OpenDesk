import { useEffect, useState } from "react";
import { Loader2, Pencil } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Modal } from "@/components/ui/modal";
import { Switch } from "@/components/ui/switch";
import { relativeTime } from "@/lib/format";
import { profileFor } from "@/lib/governance";
import type { AgentProfileRow, GovernanceAgent } from "@/lib/types";
import { cn } from "@/lib/utils";
import {
  fieldInputClass as inputClass,
  fieldLabelClass as labelClass,
} from "@/lib/ui-classes";
import {
  emptyProfile,
  type AgentProfileValues,
} from "./types";

export function ProfileModal({
  agent,
  rows,
  agents,
  saving,
  onClose,
  onSave,
}: {
  agent: GovernanceAgent | null;
  rows: AgentProfileRow[];
  agents: GovernanceAgent[];
  saving: boolean;
  onClose: () => void;
  onSave: (
    agent: GovernanceAgent,
    input: AgentProfileValues,
    close: () => void,
  ) => void;
}) {
  const existing = agent ? profileFor(rows, agent.id)?.profile : undefined;
  const [values, setValues] = useState<AgentProfileValues>(emptyProfile());
  useEffect(() => {
    if (!agent) return;
    setValues({
      role: existing?.role ?? "",
      team: existing?.team ?? "",
      managerId: existing?.manager_id ?? "",
      budget:
        existing?.monthly_budget_usd !== null &&
        existing?.monthly_budget_usd !== undefined
          ? String(existing.monthly_budget_usd)
          : "",
      quota: String(existing?.concurrency_quota ?? 5),
      heartbeat: existing?.heartbeat_enabled ?? false,
      heartbeatMinutes: String(existing?.heartbeat_minutes ?? 60),
      notes: existing?.notes ?? "",
    });
  }, [agent, existing]);
  return (
    <Modal
      open={agent !== null}
      onClose={onClose}
      title={agent ? `Governança de @${agent.id}` : "Governança do agente"}
      description="Hierarquia, budget, quota e heartbeat são estado operacional do OpenDesk; o arquivo do agente continua sendo a fonte de verdade do OpenCode."
      size="md"
    >
      {agent ? (
        <div className="space-y-3">
          <div className="grid gap-2 md:grid-cols-2">
            <label>
              <span className={labelClass}>Cargo / papel</span>
              <input
                value={values.role}
                onChange={(event) =>
                  setValues({ ...values, role: event.target.value })
                }
                placeholder="Líder de pesquisa"
                className={inputClass}
              />
            </label>
            <label>
              <span className={labelClass}>Time</span>
              <input
                value={values.team}
                onChange={(event) =>
                  setValues({ ...values, team: event.target.value })
                }
                placeholder="Inteligência"
                className={inputClass}
              />
            </label>
            <label>
              <span className={labelClass}>Reporta a</span>
              <select
                value={values.managerId}
                onChange={(event) =>
                  setValues({ ...values, managerId: event.target.value })
                }
                className={inputClass}
              >
                <option value="">sem líder</option>
                {agents
                  .filter((item) => item.id !== agent.id)
                  .map((item) => (
                    <option key={item.id} value={item.id}>
                      @{item.id}
                    </option>
                  ))}
              </select>
            </label>
            <label>
              <span className={labelClass}>Budget mensal (US$)</span>
              <input
                type="number"
                min={0}
                step="0.01"
                value={values.budget}
                onChange={(event) =>
                  setValues({ ...values, budget: event.target.value })
                }
                placeholder="sem limite"
                className={inputClass}
              />
            </label>
            <label>
              <span className={labelClass}>Quota de concorrência</span>
              <input
                type="number"
                min={1}
                max={32}
                value={values.quota}
                onChange={(event) =>
                  setValues({ ...values, quota: event.target.value })
                }
                className={inputClass}
              />
            </label>
            <label>
              <span className={labelClass}>Intervalo do heartbeat (min)</span>
              <input
                type="number"
                min={5}
                max={1440}
                value={values.heartbeatMinutes}
                disabled={!values.heartbeat}
                onChange={(event) =>
                  setValues({ ...values, heartbeatMinutes: event.target.value })
                }
                className={inputClass}
              />
            </label>
          </div>
          <div className="flex items-center justify-between gap-3 rounded-lg border border-zinc-800/70 bg-zinc-950/30 px-3 py-2">
            <div>
              <p className="text-[10px] font-medium text-zinc-300">
                Heartbeat autônomo
              </p>
              <p className="text-[9px] text-zinc-500">
                Cria uma tarefa de revisão de metas no intervalo, respeitando
                budget e quota.
              </p>
            </div>
            <Switch
              checked={values.heartbeat}
              onCheckedChange={(heartbeat) => setValues({ ...values, heartbeat })}
              label="Heartbeat autônomo"
            />
          </div>
          <label>
            <span className={labelClass}>Notas</span>
            <textarea
              value={values.notes}
              onChange={(event) =>
                setValues({ ...values, notes: event.target.value })
              }
              rows={2}
              className={cn(inputClass, "h-auto py-2")}
            />
          </label>
          {existing?.last_heartbeat_at ? (
            <p className="text-[9px] text-zinc-500">
              Último heartbeat {relativeTime(existing.last_heartbeat_at)}
              {existing.next_heartbeat_at
                ? ` · próximo ${new Date(existing.next_heartbeat_at).toLocaleString("pt-BR")}`
                : ""}
            </p>
          ) : null}
          <div className="flex justify-end gap-2">
            <Button variant="ghost" size="sm" onClick={onClose}>
              Cancelar
            </Button>
            <Button
              size="sm"
              disabled={saving}
              onClick={() => onSave(agent, values, onClose)}
            >
              {saving ? (
                <Loader2 size={13} className="animate-spin" />
              ) : (
                <Pencil size={13} />
              )}
              Salvar governança
            </Button>
          </div>
        </div>
      ) : null}
    </Modal>
  );
}

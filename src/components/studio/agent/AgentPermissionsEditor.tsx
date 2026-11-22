import { Trash2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import type { PermissionRule } from "@/lib/types";
import { ACTIONS, EFFECTS, inputClass } from "./agent-presets";

export function AgentPermissionsEditor({
  permissions,
  onAdd,
  onUpdate,
  onRemove,
}: {
  permissions: PermissionRule[];
  onAdd: () => void;
  onUpdate: (index: number, patch: Partial<PermissionRule>) => void;
  onRemove: (index: number) => void;
}) {
  return (
    <div className="rounded-xl border border-zinc-800/80 bg-zinc-900/25">
      <div className="flex items-center justify-between border-b border-zinc-800/60 px-3 py-2">
        <span className="text-[12px] font-medium uppercase tracking-[0.12em] text-zinc-500">
          Permissões V2
        </span>
        <Button variant="outline" size="sm" className="h-7" onClick={onAdd}>
          Adicionar regra
        </Button>
      </div>
      <div className="space-y-2 p-3">
        {permissions.length === 0 ? (
          <p className="text-[12px] text-zinc-500">
            Sem regras próprias. O agente herda as permissões globais do OpenCode.
          </p>
        ) : (
          permissions.map((permission, index) => (
            <div key={index} className="grid grid-cols-[130px_1fr_100px_28px] gap-2">
              <select
                value={permission.action}
                onChange={(event) =>
                  onUpdate(index, { action: event.target.value })
                }
                className={cn(inputClass, "font-mono text-[12px]")}
              >
                {ACTIONS.map((action) => (
                  <option key={action} value={action}>
                    {action}
                  </option>
                ))}
              </select>
              <input
                value={permission.resource}
                onChange={(event) =>
                  onUpdate(index, { resource: event.target.value })
                }
                placeholder="*"
                className={cn(inputClass, "font-mono text-[12px]")}
              />
              <select
                value={permission.effect}
                onChange={(event) =>
                  onUpdate(index, { effect: event.target.value })
                }
                className={cn(inputClass, "font-mono text-[12px]")}
              >
                {EFFECTS.map((effect) => (
                  <option key={effect} value={effect}>
                    {effect}
                  </option>
                ))}
              </select>
              <button
                type="button"
                onClick={() => onRemove(index)}
                className="rounded-lg text-zinc-500 hover:bg-zinc-800 hover:text-red-300"
                title="Remover"
              >
                <Trash2 size={13} className="mx-auto" />
              </button>
            </div>
          ))
        )}
      </div>
    </div>
  );
}

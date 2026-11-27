import { Loader2, Save, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import type { MemoryUsageScope } from "@/lib/types";
import { cn } from "@/lib/utils";
import { smallInputClass as inputClass, smallLabelClass as labelClass } from "@/lib/ui-classes";

export interface PersonalContextDraft {
  instructions: string;
  scope: "workspace" | "global";
  usageScope: MemoryUsageScope;
}

export function PersonalContextCreateForm({
  draft,
  onChange,
  busy,
  onCreate,
  onCancel,
}: {
  draft: PersonalContextDraft;
  onChange: (draft: PersonalContextDraft) => void;
  busy: boolean;
  onCreate: () => void;
  onCancel: () => void;
}) {
  return (
<div className="space-y-2 rounded-lg border border-violet-500/20 bg-violet-500/[0.03] p-3">
        <label>
          <span className={labelClass}>Escopo</span>
          <select
            value={draft.scope}
            onChange={(event) =>
              onChange({
                ...draft,
                scope: event.target.value as "workspace" | "global",
              })
            }
            className={inputClass}
          >
            <option value="workspace">workspace</option>
            <option value="global">global</option>
          </select>
        </label>
        <label>
          <span className={labelClass}>Onde usar</span>
          <select
            aria-label="Escopo de memória da nova regra"
            value={draft.usageScope}
            onChange={(event) =>
              onChange({
                ...draft,
                usageScope: event.target.value as MemoryUsageScope,
              })
            }
            className={inputClass}
          >
            <option value="normal">Normal (tarefas e rotinas)</option>
            <option value="chat">Chat (conversas e bots)</option>
            <option value="both">Ambos</option>
          </select>
        </label>
        <label>
          <span className={labelClass}>Regra</span>
          <textarea
            value={draft.instructions}
            onChange={(event) =>
              onChange({
                ...draft,
                instructions: event.target.value,
              })
            }
            rows={3}
            placeholder="Ex.: responda em português, seja direto e cite fontes."
            className={cn(inputClass, "h-auto resize-y py-2")}
          />
        </label>
        <div className="flex justify-end gap-1.5">
          <Button size="sm" variant="ghost" onClick={onCancel}>
            <X size={12} />
            Cancelar
          </Button>
          <Button
            size="sm"
            onClick={onCreate}
            disabled={busy}
          >
            {busy ? (
              <Loader2 size={12} className="animate-spin" />
            ) : (
              <Save size={12} />
            )}
            Adicionar
          </Button>
        </div>
</div>
  );
}

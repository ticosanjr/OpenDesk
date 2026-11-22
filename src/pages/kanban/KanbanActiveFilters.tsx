import { X } from "lucide-react";
import { PRIORITY_LABEL } from "@/lib/task-ui";

export function KanbanActiveFilters({
  advancedFilterCount,
  agentFilter,
  priorityFilter,
  attentionOnly,
  onClearAgent,
  onClearPriority,
  onClearAttention,
}: {
  advancedFilterCount: number;
  agentFilter: string;
  priorityFilter: string;
  attentionOnly: boolean;
  onClearAgent: () => void;
  onClearPriority: () => void;
  onClearAttention: () => void;
}) {
  if (advancedFilterCount === 0) return null;
  return (
    <div className="flex flex-wrap items-center gap-1.5 px-1">
      <span className="text-[13px] text-zinc-500">Filtros ativos:</span>
      {agentFilter ? (
        <button
          type="button"
          onClick={onClearAgent}
          aria-label={`Remover filtro do agente @${agentFilter}`}
          className="inline-flex items-center gap-1 rounded-full border border-zinc-700/70 bg-zinc-900/60 px-2 py-0.5 text-[13px] text-zinc-300 hover:border-zinc-600 hover:text-zinc-100"
        >
          @{agentFilter}
          <X size={11} />
        </button>
      ) : null}
      {priorityFilter ? (
        <button
          type="button"
          onClick={onClearPriority}
          aria-label={`Remover filtro de prioridade ${PRIORITY_LABEL[Number(priorityFilter)] ?? priorityFilter}`}
          className="inline-flex items-center gap-1 rounded-full border border-zinc-700/70 bg-zinc-900/60 px-2 py-0.5 text-[13px] text-zinc-300 hover:border-zinc-600 hover:text-zinc-100"
        >
          prioridade {PRIORITY_LABEL[Number(priorityFilter)] ?? priorityFilter}
          <X size={11} />
        </button>
      ) : null}
      {attentionOnly ? (
        <button
          type="button"
          onClick={onClearAttention}
          aria-label="Remover filtro de atenção"
          className="inline-flex items-center gap-1 rounded-full border border-amber-500/30 bg-amber-500/10 px-2 py-0.5 text-[13px] text-amber-200 hover:border-amber-500/50"
        >
          atenção
          <X size={11} />
        </button>
      ) : null}
    </div>
  );
}

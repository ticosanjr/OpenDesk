import type { MemoryUsageScope } from "@/lib/types";
import { cn } from "@/lib/utils";

export type MemoryScopeFilter = "all" | MemoryUsageScope;

export const MEMORY_SCOPE_LABEL: Record<MemoryUsageScope, string> = {
  normal: "Normal",
  chat: "Chat",
  both: "Ambos",
};

const FILTERS: Array<{ id: MemoryScopeFilter; label: string }> = [
  { id: "all", label: "Todos" },
  { id: "normal", label: "Normal" },
  { id: "chat", label: "Chat" },
  { id: "both", label: "Ambos" },
];

export function MemoryScopeFilterBar({
  value,
  onChange,
}: {
  value: MemoryScopeFilter;
  onChange: (value: MemoryScopeFilter) => void;
}) {
  return (
    <div
      role="group"
      aria-label="Filtrar por escopo de memória"
      className="flex flex-wrap items-center gap-1"
    >
      {FILTERS.map((filter) => (
        <button
          key={filter.id}
          type="button"
          aria-pressed={value === filter.id}
          onClick={() => onChange(filter.id)}
          className={cn(
            "rounded-md border px-2 py-1 text-[11.5px] font-medium transition-colors",
            value === filter.id
              ? "border-blue-500/40 bg-blue-500/10 text-blue-200"
              : "border-zinc-800 bg-zinc-950/40 text-zinc-500 hover:border-zinc-700 hover:text-zinc-300",
          )}
        >
          {filter.label}
        </button>
      ))}
    </div>
  );
}

export function MemoryScopeSelect({
  value,
  onChange,
  label,
  disabled = false,
}: {
  value: MemoryUsageScope;
  onChange: (value: MemoryUsageScope) => void;
  label: string;
  disabled?: boolean;
}) {
  return (
    <select
      aria-label={`Escopo de memória de ${label}`}
      value={value}
      disabled={disabled}
      onChange={(event) => onChange(event.target.value as MemoryUsageScope)}
      className="h-6 rounded border border-zinc-700 bg-zinc-900 px-1 text-[11px] text-zinc-300 outline-none disabled:opacity-50"
      title="Onde esta memória pode ser injetada"
    >
      <option value="normal">Normal</option>
      <option value="chat">Chat</option>
      <option value="both">Ambos</option>
    </select>
  );
}

export function matchesScopeFilter(
  scope: MemoryUsageScope | undefined,
  filter: MemoryScopeFilter,
): boolean {
  return filter === "all" || (scope ?? "both") === filter;
}

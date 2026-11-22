import { AlertTriangle, CheckCircle2, Clock3, Loader2, XCircle } from "lucide-react";
import {
  CALENDAR_CATEGORIES,
  CALENDAR_VIEWS,
  type CalendarCategory,
  type CalendarView,
} from "@/lib/calendar";
import { cn } from "@/lib/utils";

export function CalendarToolbar({
  view,
  filters,
  visibleCount,
  onViewChange,
  onToggleFilter,
}: {
  view: CalendarView;
  filters: Record<CalendarCategory, boolean>;
  visibleCount: number;
  onViewChange: (view: CalendarView) => void;
  onToggleFilter: (id: CalendarCategory) => void;
}) {
  return (
    <>
      <div className="flex flex-wrap items-center gap-1.5">
        <div className="flex rounded-lg border border-zinc-800 bg-zinc-950/40 p-0.5">
          {CALENDAR_VIEWS.map((item) => (
            <button
              key={item.id}
              type="button"
              aria-pressed={view === item.id}
              onClick={() => onViewChange(item.id)}
              className={cn(
                "rounded-md px-2.5 py-1 text-[12px] transition-colors",
                view === item.id
                  ? "bg-zinc-800 text-zinc-100"
                  : "text-zinc-500 hover:text-zinc-300",
              )}
            >
              {item.label}
            </button>
          ))}
        </div>
        <span className="mx-1 h-4 w-px bg-zinc-800" />
        {CALENDAR_CATEGORIES.map((category) => (
          <button
            key={category.id}
            type="button"
            aria-pressed={filters[category.id]}
            onClick={() => onToggleFilter(category.id)}
            className={cn(
              "rounded-full border px-2.5 py-1 text-[12px] transition-colors",
              filters[category.id]
                ? "workspace-selected workspace-selected-text"
                : "border-zinc-800 bg-zinc-950/40 text-zinc-500 hover:text-zinc-300",
            )}
          >
            {category.label}
          </button>
        ))}
        <span className="ml-auto font-mono text-[12px] text-zinc-500">
          {visibleCount}{" "}
          {visibleCount === 1 ? "item visível" : "itens visíveis"}
        </span>
      </div>

      <div className="flex flex-wrap items-center gap-3 px-1 text-[11.5px] text-zinc-500">
        <span className="inline-flex items-center gap-1">
          <CheckCircle2 size={10} className="text-emerald-400" />
          executado
        </span>
        <span className="inline-flex items-center gap-1">
          <XCircle size={10} className="text-red-400" />
          falhou/cancelado
        </span>
        <span className="inline-flex items-center gap-1">
          <Loader2 size={10} className="text-blue-400" />
          executando
        </span>
        <span className="inline-flex items-center gap-1">
          <Clock3 size={10} className="text-amber-400" />
          na fila/agendada
        </span>
        <span className="inline-flex items-center gap-1">
          <AlertTriangle size={10} className="text-amber-400" />
          perdida
        </span>
        <span>Dias passados mostram apenas o que rodou ou precisa de decisão.</span>
      </div>
    </>
  );
}

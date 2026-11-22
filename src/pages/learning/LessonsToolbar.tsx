import { Brain } from "lucide-react";
import { cn } from "@/lib/utils";

export type LessonFilter = "OPEN" | "PROMOTED" | "MEMORIZED" | "DISMISSED" | "ALL";

const FILTERS: Array<{ id: LessonFilter; label: string }> = [
  { id: "OPEN", label: "Abertas" },
  { id: "PROMOTED", label: "Promovidas" },
  { id: "MEMORIZED", label: "Memorizadas" },
  { id: "DISMISSED", label: "Descartadas" },
  { id: "ALL", label: "Todas" },
];

export function LessonsToolbar({
  filter,
  onFilterChange,
}: {
  filter: LessonFilter;
  onFilterChange: (filter: LessonFilter) => void;
}) {
  return (
    <div className="flex flex-wrap items-center justify-between gap-2">
      <div className="flex items-center gap-2">
        <Brain size={13} className="text-zinc-400" />
        <h2 className="text-[13.5px] font-medium text-zinc-200">Lições</h2>
      </div>
      <div className="flex flex-wrap gap-1">
        {FILTERS.map((option) => (
          <button
            key={option.id}
            type="button"
            aria-pressed={filter === option.id}
            onClick={() => onFilterChange(option.id)}
            className={cn(
              "rounded-md px-2 py-1 text-[12px] transition-colors",
              filter === option.id
                ? "bg-zinc-800 text-zinc-100"
                : "text-zinc-500 hover:text-zinc-200",
            )}
          >
            {option.label}
          </button>
        ))}
      </div>
    </div>
  );
}

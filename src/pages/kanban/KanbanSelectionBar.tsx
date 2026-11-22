import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { KANBAN_COLUMNS } from "@/components/kanban/kanban-columns";

export function KanbanSelectionBar({
  selectedCount,
  onMove,
  onClear,
}: {
  selectedCount: number;
  onMove: (dropStatus: string) => void;
  onClear: () => void;
}) {
  return (
    <div className="flex flex-wrap items-center gap-2 rounded-xl border border-blue-500/25 bg-blue-500/[0.06] px-3 py-2">
      <Badge variant="blue">{selectedCount} selecionada(s)</Badge>
      <span className="text-[13px] text-zinc-400">mover para:</span>
      {KANBAN_COLUMNS.map((column) => (
        <Button
          key={column.id}
          variant="outline"
          size="sm"
          className="h-7 text-[12px]"
          title={column.description}
          onClick={() => onMove(column.dropStatus)}
        >
          {column.label}
        </Button>
      ))}
      <button
        type="button"
        className="ml-auto text-[12px] text-zinc-500 hover:text-zinc-200"
        onClick={onClear}
      >
        limpar seleção
      </button>
    </div>
  );
}

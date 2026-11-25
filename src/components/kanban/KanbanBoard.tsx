import { useCallback, useEffect, useRef, useState } from "react";
import { ChevronLeft, ChevronRight, GripVertical } from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { CountPill } from "@/components/ui/count-pill";
import { TaskCard } from "@/components/kanban/TaskCard";
import type { TaskPermissionRequest } from "@/components/kanban/task-attention-modal";
import { workflowMeta } from "@/lib/workflow";
import { cn } from "@/lib/utils";
import type { Task, TaskInteraction } from "@/lib/types";
import { KANBAN_COLUMNS } from "./kanban-columns";

export type KanbanAttentionMap = Map<
  string,
  { interactions: TaskInteraction[]; permissions: TaskPermissionRequest[] }
>;

export function KanbanBoard({
  tasks,
  attentionByTask,
  pinned,
  daemonOnline,
  onOpenTask,
  onDispatch,
  onResume,
  onCancel,
  onApprove,
  onLogs,
  onTransition,
  onBlock,
  onAttentionResolved,
}: {
  tasks: Task[];
  attentionByTask: KanbanAttentionMap;
  pinned: Record<string, number>;
  daemonOnline: boolean;
  onOpenTask: (task: Task) => void;
  onDispatch: (task: Task) => void;
  onResume: (task: Task) => void;
  onCancel: (task: Task) => void;
  onApprove: (task: Task) => void;
  onLogs: (task: Task) => void;
  onTransition: (task: Task, status: string) => void;
  onBlock: (task: Task) => void;
  onAttentionResolved: () => void;
}) {
  const boardRef = useRef<HTMLDivElement | null>(null);
  const draggedRef = useRef<string | null>(null);
  const [dragOverColumn, setDragOverColumn] = useState<string | null>(null);
  const [boardScroll, setBoardScroll] = useState({ left: false, right: false });

  const updateBoardScroll = useCallback(() => {
    const element = boardRef.current;
    if (!element) {
      setBoardScroll({ left: false, right: false });
      return;
    }
    setBoardScroll({
      left: element.scrollLeft > 4,
      right:
        element.scrollLeft + element.clientWidth < element.scrollWidth - 4,
    });
  }, []);

  useEffect(() => {
    updateBoardScroll();
    const element = boardRef.current;
    element?.addEventListener("scroll", updateBoardScroll, { passive: true });
    window.addEventListener("resize", updateBoardScroll);
    return () => {
      element?.removeEventListener("scroll", updateBoardScroll);
      window.removeEventListener("resize", updateBoardScroll);
    };
  }, [updateBoardScroll, tasks.length]);

  return (
    <div className="relative flex min-h-0 flex-1 flex-col">
      <div
        ref={boardRef}
        role="region"
        aria-label="Quadro Kanban"
        tabIndex={0}
        className="-mx-1 flex min-h-0 flex-1 items-stretch gap-3 overflow-x-auto px-1 pb-2 outline-none focus-visible:ring-2 focus-visible:ring-blue-500/50"
        onDragOver={(event) => {
          event.preventDefault();
          const element = boardRef.current;
          if (!element) return;
          const rect = element.getBoundingClientRect();
          if (event.clientX < rect.left + 72) element.scrollLeft -= 18;
          else if (event.clientX > rect.right - 72) element.scrollLeft += 18;
        }}
      >
        {KANBAN_COLUMNS.map((column) => {
          const columnTasks = tasks
            .filter((task) => {
            const pending = attentionByTask.get(task.id);
            const effective =
              pending &&
              (pending.interactions.length > 0 || pending.permissions.length > 0)
                ? "WAITING_INPUT"
                : workflowMeta(task).state;
              return column.states.includes(effective);
            });
          return (
            <Card
              key={column.id}
              data-column={column.id}
              className={cn(
                "flex h-full min-h-[440px] w-[280px] shrink-0 flex-col transition-colors xl:w-auto xl:min-w-[220px] xl:flex-1",
                dragOverColumn === column.id &&
                  "border-blue-500/50 bg-blue-500/[0.04]",
              )}
              onDragEnter={() => setDragOverColumn(column.id)}
              onDragLeave={(event) => {
                if (
                  !event.currentTarget.contains(
                    event.relatedTarget as Node | null,
                  )
                ) {
                  setDragOverColumn((current) =>
                    current === column.id ? null : current,
                  );
                }
              }}
              onDragOver={(event) => event.preventDefault()}
              onDrop={(event) => {
                const taskId =
                  event.dataTransfer.getData("text/task-id") ||
                  draggedRef.current;
                draggedRef.current = null;
                setDragOverColumn(null);
                const task = tasks.find((item) => item.id === taskId);
                if (!task) return;
                if (column.dropStatus === "BLOCKED") onBlock(task);
                else onTransition(task, column.dropStatus);
              }}
            >
              <CardHeader className="px-3.5 py-3">
                <CardTitle
                  className="flex items-center gap-2 text-[13.5px]"
                  title={column.description}
                >
                  <span className="h-1.5 w-1.5 rounded-full bg-zinc-600" />
                  {column.label}
                </CardTitle>
                <CountPill value={columnTasks.length} tone={column.tone} />
              </CardHeader>
              <CardContent className="min-h-0 flex-1 space-y-2 overflow-y-auto p-2.5">
                {columnTasks.length === 0 ? (
                  <div className="rounded-lg border border-dashed border-zinc-800/80 bg-zinc-950/30 px-3 py-10 text-center">
                    <GripVertical size={16} className="mx-auto text-zinc-600" />
                    <p className="mt-2 text-[12px] font-medium text-zinc-400">
                      Nenhuma tarefa nesta etapa
                    </p>
                    <p className="mt-0.5 text-[11px] text-zinc-500">
                      Arraste um card para mover.
                    </p>
                  </div>
                ) : (
                  columnTasks.map((task) => (
                    <TaskCard
                      key={task.id}
                      task={task}
                      pinned={pinned[task.id] ?? 0}
                      daemonOnline={daemonOnline}
                      onOpen={() => onOpenTask(task)}
                      onDispatch={() => onDispatch(task)}
                      onResume={() => onResume(task)}
                      onCancel={() => onCancel(task)}
                      onApprove={() => onApprove(task)}
                      onLogs={() => onLogs(task)}
                      interactions={attentionByTask.get(task.id)?.interactions}
                      permissions={attentionByTask.get(task.id)?.permissions}
                      onAttentionResolved={onAttentionResolved}
                      onDragStart={(event) => {
                        draggedRef.current = task.id;
                        event.dataTransfer.setData("text/task-id", task.id);
                        event.dataTransfer.effectAllowed = "move";
                      }}
                      onDragEnd={() => {
                        // Keep the ref until the drop handler runs.
                      }}
                    />
                  ))
                )}
              </CardContent>
            </Card>
          );
        })}
      </div>
      {boardScroll.left ? (
        <button
          type="button"
          aria-label="Rolar o quadro para a esquerda"
          title="Rolar para a esquerda"
          onClick={() =>
            boardRef.current?.scrollBy({ left: -320, behavior: "smooth" })
          }
          className="absolute left-1 top-1/2 z-20 hidden h-8 w-8 -translate-y-1/2 items-center justify-center rounded-full border border-zinc-700 bg-zinc-950/95 text-zinc-300 shadow-lg md:flex"
        >
          <ChevronLeft size={17} />
        </button>
      ) : null}
      {boardScroll.right ? (
        <button
          type="button"
          aria-label="Rolar o quadro para a direita"
          title="Rolar para a direita"
          onClick={() =>
            boardRef.current?.scrollBy({ left: 320, behavior: "smooth" })
          }
          className="absolute right-1 top-1/2 z-20 hidden h-8 w-8 -translate-y-1/2 items-center justify-center rounded-full border border-zinc-700 bg-zinc-950/95 text-zinc-300 shadow-lg md:flex"
        >
          <ChevronRight size={17} />
        </button>
      ) : null}
    </div>
  );
}

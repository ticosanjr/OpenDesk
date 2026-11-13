import { Card, CardContent } from "@/components/ui/card";
import { WorkflowStatusBadge } from "@/components/ui/workflow-status";
import { PRIORITY_LABEL } from "@/lib/task-ui";
import { relativeTime } from "@/lib/format";
import { cn } from "@/lib/utils";
import type { Task } from "@/lib/types";

export function KanbanListView({
  tasks,
  selected,
  onSelectAll,
  onToggleTask,
  onOpenTask,
}: {
  tasks: Task[];
  selected: Set<string>;
  onSelectAll: (checked: boolean) => void;
  onToggleTask: (id: string, checked: boolean) => void;
  onOpenTask: (task: Task) => void;
}) {
  return (
    <Card>
      <CardContent className="p-0">
        <div className="scroll-area max-h-[calc(100dvh-18rem)]">
          <table className="w-full text-left">
            <thead className="sticky top-0 z-10 border-b border-zinc-800/70 bg-zinc-950/95 text-[12px] uppercase tracking-wide text-zinc-500">
              <tr>
                <th className="w-8 px-3 py-2">
                  <input
                    type="checkbox"
                    aria-label="Selecionar todas as tarefas"
                    checked={tasks.length > 0 && selected.size === tasks.length}
                    onChange={(event) => onSelectAll(event.target.checked)}
                    className="accent-blue-500"
                  />
                </th>
                <th className="px-3 py-2">Tarefa</th>
                <th className="px-3 py-2">Status</th>
                <th className="px-3 py-2">Agente</th>
                <th className="px-3 py-2">Último run</th>
                <th className="px-3 py-2">Prioridade</th>
              </tr>
            </thead>
            <tbody>
              {tasks.map((task) => (
                <tr
                  key={task.id}
                  className="cursor-pointer border-b border-zinc-800/50 text-[13.5px] hover:bg-zinc-900/40"
                  onClick={() => onOpenTask(task)}
                >
                  <td
                    className="px-3 py-2"
                    onClick={(event) => event.stopPropagation()}
                  >
                    <input
                      type="checkbox"
                      aria-label={`Selecionar ${task.title}`}
                      checked={selected.has(task.id)}
                      onChange={(event) =>
                        onToggleTask(task.id, event.target.checked)
                      }
                      className="accent-blue-500"
                    />
                  </td>
                  <td className="max-w-[360px] truncate px-3 py-2 font-medium text-zinc-200">
                    {task.title}
                  </td>
                  <td className="px-3 py-2">
                    <WorkflowStatusBadge task={task} />
                  </td>
                  <td className="px-3 py-2 font-mono text-[13px] text-zinc-400">
                    {task.assigned_agent ? `@${task.assigned_agent}` : "router"}
                  </td>
                  <td className="px-3 py-2">
                    {task.last_run_status ? (
                      <span
                        className={cn(
                          "font-mono text-[13px]",
                          task.last_run_status === "SUCCESS"
                            ? "text-emerald-300"
                            : task.last_run_status === "FAILED"
                              ? "text-red-300"
                              : "text-zinc-400",
                        )}
                      >
                        {task.last_run_status} · {relativeTime(task.last_run_at)}
                      </span>
                    ) : (
                      <span className="text-[13px] text-zinc-500">
                        sem execução
                      </span>
                    )}
                  </td>
                  <td className="px-3 py-2 text-zinc-400">
                    {PRIORITY_LABEL[task.priority] ?? task.priority}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </CardContent>
    </Card>
  );
}

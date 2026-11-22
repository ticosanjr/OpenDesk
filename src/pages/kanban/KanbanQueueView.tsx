import { QueueController } from "@/components/queue/QueueController";
import type { QueueJob, QueueStatus } from "@/lib/types";
import { cn } from "@/lib/utils";

export function KanbanQueueView({
  queueScope,
  setQueueScope,
  workQueueCount,
  automationQueueCount,
  queueLength,
  displayedQueue,
  queueStatus,
  loading,
  onRefresh,
  onTogglePause,
  onConcurrency,
  onJobAction,
  onPriority,
}: {
  queueScope: "work" | "automations" | "all";
  setQueueScope: (scope: "work" | "automations" | "all") => void;
  workQueueCount: number;
  automationQueueCount: number;
  queueLength: number;
  displayedQueue: QueueJob[];
  queueStatus: QueueStatus | null;
  loading: boolean;
  onRefresh: () => void;
  onTogglePause: (pause: boolean) => void;
  onConcurrency: (value: number) => void;
  onJobAction: (id: string, action: "cancel" | "retry") => void;
  onPriority: (id: string, priority: number) => void;
}) {
  return (
    <>
      <div className="flex flex-wrap items-center gap-1 rounded-lg border border-zinc-800 bg-zinc-950/40 p-0.5">
        {([
          ["work", `Trabalho · ${workQueueCount}`],
          ["automations", `Automações · ${automationQueueCount}`],
          ["all", `Tudo · ${queueLength}`],
        ] as const).map(([value, label]) => (
          <button
            key={value}
            type="button"
            onClick={() => setQueueScope(value)}
            aria-pressed={queueScope === value}
            className={cn(
              "rounded-md px-2.5 py-1 text-[12.5px] transition-colors",
              queueScope === value
                ? "workspace-tab-active"
                : "text-zinc-500 hover:text-zinc-200",
            )}
          >
            {label}
          </button>
        ))}
      </div>
      <QueueController
        queue={displayedQueue}
        status={queueStatus}
        loading={loading}
        onRefresh={onRefresh}
        onTogglePause={onTogglePause}
        onConcurrency={onConcurrency}
        onJobAction={onJobAction}
        onPriority={onPriority}
      />
    </>
  );
}

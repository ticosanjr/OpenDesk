import { Card, CardContent } from "@/components/ui/card";
import { EntityLink } from "@/components/ui/entity-link";
import { WorkingIndicator } from "@/components/ui/working-indicator";
import { relativeTime } from "@/lib/format";
import type { TaskRun } from "@/lib/types";

export function RunningRunsCard({
  runs,
  onOpenRun,
}: {
  runs: TaskRun[];
  onOpenRun: (runId: string) => void;
}) {
  if (runs.length === 0) return null;
  return (
    <Card className="border-blue-500/25 bg-blue-500/[0.03]">
      <CardContent className="flex flex-wrap items-center gap-3 p-3">
        <WorkingIndicator label={`${runs.length} em execução`} />
        <div className="flex min-w-0 flex-1 flex-wrap gap-2">
          {runs.slice(0, 4).map((run) => (
            <button
              key={run.id}
              type="button"
              onClick={() => onOpenRun(run.id)}
              className="min-w-0 rounded-lg border border-blue-500/20 bg-zinc-950/40 px-2.5 py-1.5 text-left transition-colors hover:border-blue-500/40"
              title={`Abrir execução de ${run.task_title ?? run.agent_name}`}
            >
              <p className="max-w-[260px] truncate text-[13px] text-zinc-200">
                {run.task_title ?? run.agent_name}
              </p>
              <p className="font-mono text-[11.5px] text-zinc-500">
                @{run.agent_name} · {run.model_id} ·{" "}
                {relativeTime(run.started_at)}
              </p>
            </button>
          ))}
        </div>
        <EntityLink to="/runs" className="text-[12px] text-zinc-400">
          ver todas
        </EntityLink>
      </CardContent>
    </Card>
  );
}

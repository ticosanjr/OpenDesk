import { useMemo, useState } from "react";
import { useNavigate } from "react-router-dom";
import { Archive, Cog, Cpu, Sparkles } from "lucide-react";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { EmptyState } from "@/components/ui/empty-state";
import { Button } from "@/components/ui/button";
import { dayLabel, formatNumber, formatUsd, relativeTime } from "@/lib/format";
import { links } from "@/lib/links";
import { cn } from "@/lib/utils";
import type { Artifact, TaskRun } from "@/lib/types";

export type Activity =
  | { kind: "run"; at: string; run: TaskRun }
  | { kind: "artifact"; at: string; artifact: Artifact };

export type ActivityKind = "all" | "run" | "artifact";

const FILTERS: Array<{ id: ActivityKind; label: string }> = [
  { id: "all", label: "Tudo" },
  { id: "run", label: "Runs" },
  { id: "artifact", label: "Artefatos" },
];

export function ActivityFeed({
  activities,
  onCreateTask,
}: {
  activities: Activity[];
  onCreateTask: () => void;
}) {
  const navigate = useNavigate();
  const [kind, setKind] = useState<ActivityKind>("all");

  const visible = useMemo(
    () =>
      kind === "all"
        ? activities
        : activities.filter((activity) => activity.kind === kind),
    [activities, kind],
  );

  const groups = useMemo(() => {
    const map = new Map<string, Activity[]>();
    for (const activity of visible) {
      const label = dayLabel(activity.at);
      map.set(label, [...(map.get(label) ?? []), activity]);
    }
    return Array.from(map.entries());
  }, [visible]);

  return (
    <Card>
      <CardHeader>
        <div>
          <CardTitle className="flex items-center gap-2">
            <Sparkles size={15} className="text-zinc-500" />
            Atividades recentes
          </CardTitle>
          <CardDescription>
            Clique em um run ou artefato para abrir o detalhe.
          </CardDescription>
        </div>
        <div className="flex items-center gap-1">
          <div
            className="flex rounded-lg border border-zinc-800 bg-zinc-950/40 p-0.5"
            role="group"
            aria-label="Filtrar atividades"
          >
            {FILTERS.map((filter) => (
              <button
                key={filter.id}
                type="button"
                aria-pressed={kind === filter.id}
                onClick={() => setKind(filter.id)}
                className={cn(
                  "inline-flex h-6 items-center rounded-md px-2 text-[12px] transition-colors",
                  kind === filter.id
                    ? "bg-zinc-800 text-zinc-100"
                    : "text-zinc-500 hover:text-zinc-200",
                )}
              >
                {filter.label}
              </button>
            ))}
          </div>
          <Badge variant="default">{visible.length}</Badge>
        </div>
      </CardHeader>
      <CardContent>
        {visible.length === 0 ? (
          <EmptyState
            icon={Archive}
            title={
              activities.length === 0
                ? "Nenhuma atividade ainda"
                : "Nada neste filtro"
            }
            description={
              activities.length === 0
                ? "Runs e artefatos aparecerão aqui em tempo real via WebSocket."
                : "Troque o filtro para ver runs ou artefatos."
            }
            compact
            action={
              activities.length === 0 ? (
                <Button size="sm" onClick={onCreateTask}>
                  Criar tarefa
                </Button>
              ) : undefined
            }
          />
        ) : (
          <div className="space-y-3">
            {groups.map(([label, items]) => (
              <section key={label} className="space-y-2">
                <p className="px-0.5 font-mono text-[11.5px] uppercase tracking-wide text-zinc-500">
                  {label}
                </p>
                {items.map((activity) =>
                  activity.kind === "artifact" ? (
                    <button
                      type="button"
                      key={`artifact-${activity.artifact.id}`}
                      onClick={() =>
                        navigate(links.artifact(activity.artifact.id))
                      }
                      className="flex w-full items-center gap-3 rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2 text-left transition hover:border-emerald-500/30 hover:bg-emerald-500/[0.04]"
                    >
                      <Archive size={13} className="shrink-0 text-emerald-400/70" />
                      <div className="min-w-0 flex-1">
                        <p className="truncate font-mono text-[12px] text-zinc-300">
                          {activity.artifact.file_path}
                        </p>
                        <p className="truncate text-[11.5px] text-zinc-500">
                          @{activity.artifact.created_by_agent} ·{" "}
                          {activity.artifact.creation_reason}
                        </p>
                      </div>
                      <span className="shrink-0 font-mono text-[11px] text-zinc-500">
                        {relativeTime(activity.at)}
                      </span>
                    </button>
                  ) : (
                    <button
                      type="button"
                      key={`run-${activity.run.id}`}
                      onClick={() => navigate(links.run(activity.run.id))}
                      className={cn(
                        "flex w-full items-center gap-3 rounded-lg border bg-zinc-950/40 px-3 py-2 text-left transition hover:bg-zinc-900/60",
                        activity.run.status === "FAILED"
                          ? "border-red-500/40"
                          : activity.run.status === "SUCCESS"
                            ? "border-emerald-500/25"
                            : "border-zinc-800/70",
                      )}
                    >
                      {activity.run.status === "RUNNING" ? (
                        <Cog size={13} className="opendesk-gear shrink-0 text-blue-400" />
                      ) : (
                        <Cpu
                          size={13}
                          className={cn(
                            "shrink-0",
                            activity.run.status === "FAILED"
                              ? "text-red-400"
                              : activity.run.status === "SUCCESS"
                                ? "text-emerald-400/80"
                                : "text-blue-400/70",
                          )}
                        />
                      )}
                      <div className="min-w-0 flex-1">
                        <p className="truncate text-[12px] text-zinc-300">
                          {activity.run.task_title ?? activity.run.agent_name} ·{" "}
                          {activity.run.phase ?? activity.run.execution_mode}
                        </p>
                        <p
                          className={cn(
                            "truncate font-mono text-[11.5px]",
                            activity.run.status === "FAILED"
                              ? "text-red-300/80"
                              : "text-zinc-500",
                          )}
                        >
                          {activity.run.status} ·{" "}
                          {formatUsd(activity.run.estimated_cost_usd)} ·{" "}
                          {formatNumber(
                            activity.run.tokens_input + activity.run.tokens_output,
                          )}{" "}
                          tokens
                          {activity.run.error_message
                            ? ` · ${activity.run.error_message}`
                            : ""}
                        </p>
                      </div>
                      <span className="shrink-0 font-mono text-[11px] text-zinc-500">
                        {relativeTime(activity.at)}
                      </span>
                    </button>
                  ),
                )}
              </section>
            ))}
          </div>
        )}
      </CardContent>
    </Card>
  );
}

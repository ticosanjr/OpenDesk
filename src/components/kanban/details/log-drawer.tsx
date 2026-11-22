import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { FileText, XCircle } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { RunLogView } from "@/components/runs/RunLogView";
import { EmptyState } from "@/components/ui/empty-state";
import { apiGet } from "@/lib/api";
import { formatUsd, relativeTime } from "@/lib/format";
import { links } from "@/lib/links";
import { STATUS_LABEL } from "@/lib/task-ui";
import type { Task, TaskRun } from "@/lib/types";
import { cn } from "@/lib/utils";
import { runVariant } from "./ui";

export function LogDrawer({
  task,
  initialRunId,
  onClose,
  lastRunEvent,
}: {
  task: Task | null;
  initialRunId?: string | null;
  onClose: () => void;
  lastRunEvent: { run_id: string; status: string } | null;
}) {
  const [runs, setRuns] = useState<TaskRun[]>([]);
  const [selectedRun, setSelectedRun] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [onClose]);

  useEffect(() => {
    if (!task) return;
    let cancelled = false;
    setLoading(true);
    apiGet<TaskRun[]>(
      `/api/runs?task_id=${encodeURIComponent(task.id)}&workspace_id=${encodeURIComponent(task.workspace_id)}`,
    )
      .then((items) => {
        if (cancelled) return;
        setRuns(items);
        setSelectedRun((current) => {
          if (initialRunId && items.some((item) => item.id === initialRunId)) {
            return initialRunId;
          }
          return current ?? items[0]?.id ?? null;
        });
      })
      .catch((cause) =>
        setError(cause instanceof Error ? cause.message : String(cause)),
      )
      .finally(() => setLoading(false));
    return () => {
      cancelled = true;
    };
  }, [task, initialRunId, lastRunEvent?.run_id, lastRunEvent?.status]);

  if (!task) return null;

  return (
    <div className="fixed inset-0 z-40 flex items-center justify-center p-3 sm:p-5">
      <button
        type="button"
        aria-label="Fechar logs"
        className="absolute inset-0 bg-black/70 backdrop-blur-[2px]"
        onClick={onClose}
      />
      <aside className="relative flex h-[min(88dvh,64rem)] w-[min(94vw,72rem)] flex-col overflow-hidden rounded-2xl border border-zinc-800 bg-zinc-950/95 shadow-[0_24px_80px_-20px_rgba(0,0,0,0.85)] backdrop-blur-xl">
        <div className="flex items-center justify-between border-b border-zinc-800/70 px-4 py-3">
          <div className="min-w-0">
            <p className="truncate text-[13.5px] font-medium text-zinc-200">
              {task.title}
            </p>
            <p className="font-mono text-[11.5px] text-zinc-500">
              {STATUS_LABEL[task.status] ?? task.status} · {task.phase_status ?? "sem fase"}
            </p>
          </div>
          <div className="flex shrink-0 items-center gap-2">
            {selectedRun ? (
              <Link
                to={links.run(selectedRun)}
                className="rounded-lg border border-zinc-800 px-2.5 py-1 text-[12px] text-zinc-400 hover:border-zinc-700 hover:text-zinc-200"
              >
                abrir página
              </Link>
            ) : null}
            <Button variant="ghost" size="sm" onClick={onClose}>
              <XCircle size={14} />
              Fechar
            </Button>
          </div>
        </div>
        <div className="flex min-h-0 flex-1">
          <div className="w-56 shrink-0 overflow-y-auto border-r border-zinc-800/70 p-2">
            {loading && runs.length === 0 ? (
              <p className="p-2 text-[12px] text-zinc-500">Carregando…</p>
            ) : error ? (
              <p className="p-2 text-[12px] text-red-300">{error}</p>
            ) : runs.length === 0 ? (
              <p className="p-2 text-[12px] text-zinc-500">Nenhum run.</p>
            ) : (
              runs.map((run) => (
                <button
                  key={run.id}
                  type="button"
                  onClick={() => setSelectedRun(run.id)}
                  className={cn(
                    "mb-1 w-full rounded-lg px-2 py-1.5 text-left",
                    selectedRun === run.id
                      ? "bg-zinc-800 text-zinc-100"
                      : "text-zinc-500 hover:bg-zinc-900",
                  )}
                >
                  <div className="flex items-center justify-between gap-1">
                    <p className="truncate text-[12px]">
                      tentativa {run.attempt_number ?? 1} · {run.phase ?? "run"}
                    </p>
                    <Badge variant={runVariant(run.status)}>{run.status}</Badge>
                  </div>
                  <p className="font-mono text-[11px] text-zinc-500">
                    {formatUsd(run.estimated_cost_usd)} · {relativeTime(run.started_at)}
                  </p>
                  {run.error_message ? (
                    <p className="truncate text-[11px] text-red-400/80">
                      {run.error_message}
                    </p>
                  ) : null}
                </button>
              ))
            )}
          </div>
          <div className="flex min-w-0 flex-1 flex-col overflow-hidden bg-zinc-950/60">
            {selectedRun ? (
              <RunLogView runId={selectedRun} compact />
            ) : (
              <EmptyState
                icon={FileText}
                title="Nenhum run selecionado"
                description="Execute a tarefa ou escolha um run na lista à esquerda."
                className="border-0 bg-transparent"
              />
            )}
          </div>
        </div>
      </aside>
    </div>
  );
}

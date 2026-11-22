import { AlertTriangle, ExternalLink, RefreshCw, Trash2 } from "lucide-react";
import { Link } from "react-router-dom";
import { Button } from "@/components/ui/button";
import { IconLabel } from "@/components/ui/icon-label";
import { Tooltip } from "@/components/ui/tooltip";
import { DismissButton, Row } from "@/components/dashboard/attention/Row";
import { Section } from "@/components/dashboard/attention/Section";
import type { Attention } from "@/components/dashboard/attention/types";
import { links } from "@/lib/links";

export function FailedRunsSection({
  runs,
  busy,
  onRetry,
  onRemoveTask,
  onDismiss,
}: {
  runs: Attention["failed_runs"];
  busy: string | null;
  onRetry: (runId: string) => void;
  onRemoveTask: (taskId: string, title: string) => void;
  onDismiss: (runId: string, version?: string | null) => void;
}) {
  if (runs.length === 0) return null;
  return (
    <Section title="Falhas nas últimas 24h" count={runs.length} tone="red">
      {runs.map((run) => (
        <Row
          key={`failed-${run.id}`}
          tone="red"
          icon={<AlertTriangle size={13} />}
          title={run.title ?? "Execução"}
          meta="falhou"
          detail={run.error}
          actions={
            <>
              <Button
                size="xs"
                variant="outline"
                disabled={busy === `retry-${run.id}`}
                onClick={() => onRetry(run.id)}
              >
                <IconLabel
                  icon={
                    <RefreshCw
                      size={11}
                      className={
                        busy === `retry-${run.id}`
                          ? "animate-spin"
                          : undefined
                      }
                    />
                  }
                >
                  Retry com outro modelo
                </IconLabel>
              </Button>
              <Link
                to={links.run(run.id)}
                className="inline-flex items-center gap-1 text-[13px] text-zinc-400 hover:text-zinc-200"
              >
                <ExternalLink size={10} />
                logs
              </Link>
              {run.task_id ? (
                <Tooltip content="Remover tarefa">
                  <Button
                    size="sm"
                    variant="outline"
                    className="h-7 border-red-500/30 px-2 text-red-300"
                    aria-label="Remover tarefa"
                    disabled={busy === `remove-${run.task_id}`}
                    onClick={() =>
                      onRemoveTask(run.task_id as string, run.title ?? "Execução")
                    }
                  >
                    <Trash2 size={12} />
                  </Button>
                </Tooltip>
              ) : null}
              <DismissButton
                label="Dispensar alerta de falha"
                onClick={() => onDismiss(run.id, run.version)}
              />
            </>
          }
        />
      ))}
    </Section>
  );
}

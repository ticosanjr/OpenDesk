import { CheckCircle2, Loader2, Plus } from "lucide-react";
import { PageHeader } from "@/components/layout/PageHeader";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import type { DispatchDecision, QueueStatus } from "@/lib/types";

export function KanbanPageHeader({
  workspaceName,
  queueStatus,
  workRunning,
  onCreateTask,
  error,
  notice,
  onDismissNotice,
  decision,
  onDismissDecision,
}: {
  workspaceName: string;
  queueStatus: QueueStatus | null;
  workRunning: number;
  onCreateTask: () => void;
  error: string | null;
  notice: string | null;
  onDismissNotice: () => void;
  decision: DispatchDecision | null;
  onDismissDecision: () => void;
}) {
  return (
    <>
      <PageHeader
        title="Kanban & Fila"
        description={`${workspaceName} · tarefas passam por Plan, Build e Audit; a fila executa no OpenCode com budget guard.`}
        actions={
          <>
            {queueStatus ? (
              <Badge variant={queueStatus.paused ? "yellow" : "green"}>
                {queueStatus.paused ? "fila pausada" : "fila ativa"}
              </Badge>
            ) : null}
            {workRunning > 0 ? (
              <Badge variant="blue" title="Execuções de trabalho em andamento">
                <Loader2 size={11} className="animate-spin" />
                {workRunning} executando
              </Badge>
            ) : null}
            <Button size="sm" onClick={onCreateTask}>
              <Plus size={15} />
              Nova tarefa
            </Button>
          </>
        }
      />

      {error ? (
        <p className="rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[13px] text-red-300">
          {error}
        </p>
      ) : null}
      {notice ? (
        <p className="flex items-center gap-2 rounded-lg border border-emerald-500/20 bg-emerald-500/10 px-3 py-2 text-[13px] text-emerald-300">
          <CheckCircle2 size={13} />
          {notice}
          <button
            type="button"
            className="ml-auto text-[12px] text-emerald-300/70 hover:text-emerald-200"
            onClick={onDismissNotice}
          >
            fechar
          </button>
        </p>
      ) : null}

      {decision ? (
        <div className="flex flex-wrap items-center gap-2 rounded-xl border border-blue-500/20 bg-blue-500/[0.06] px-4 py-3 text-[13px] text-blue-200">
          <Badge variant="blue">{decision.phase}</Badge>
          <span className="font-medium">
            {decision.selected_agent} · {decision.selected_model} · {decision.effort}
          </span>
          <span className="text-blue-300/70">{decision.rationale}</span>
          <button
            type="button"
            className="ml-auto text-[12px] text-blue-300/60 hover:text-blue-200"
            onClick={onDismissDecision}
          >
            fechar
          </button>
        </div>
      ) : null}
    </>
  );
}

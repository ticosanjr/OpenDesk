import {
  GripVertical,
  KanbanSquare,
  ListChecks,
  Pause,
  Play,
  Plus,
  Square,
} from "lucide-react";
import { PageBody, PageHeader } from "@/components/layout/PageHeader";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { EmptyState } from "@/components/ui/empty-state";
import { useAppStore } from "@/store/app-store";
import { relativeTime } from "@/lib/format";

const COLUMNS = [
  { id: "BACKLOG", label: "Backlog", tone: "default" as const },
  { id: "PLAN", label: "Plan", tone: "blue" as const },
  { id: "BUILD", label: "Build", tone: "purple" as const },
  { id: "REVIEW", label: "Review / Audit", tone: "yellow" as const },
  { id: "DONE", label: "Done", tone: "green" as const },
];

export function KanbanPage() {
  const queue = useAppStore((state) => state.queue);
  const dashboard = useAppStore((state) => state.dashboard);

  return (
    <PageBody>
      <PageHeader
        eyebrow="gestão de tarefas"
        title="Kanban & Fila"
        description="Cards passam por Plan, Build e Audit. A fila controla concorrência, pausa, cancelamento e retry."
        actions={
          <Button size="sm" disabled title="Disponível na Fase 3">
            <Plus size={15} />
            Nova tarefa
            <Badge variant="default" className="ml-1">
              fase 3
            </Badge>
          </Button>
        }
      />

      <div className="grid gap-3 xl:grid-cols-5">
        {COLUMNS.map((column) => (
          <Card key={column.id} className="flex min-h-[360px] flex-col">
            <CardHeader className="px-3.5 py-3">
              <CardTitle className="flex items-center gap-2 text-[12px]">
                <span className="h-1.5 w-1.5 rounded-full bg-zinc-600" />
                {column.label}
              </CardTitle>
              <Badge variant={column.tone}>0</Badge>
            </CardHeader>
            <CardContent className="flex flex-1 items-center justify-center p-3">
              <div className="w-full rounded-lg border border-dashed border-zinc-800/80 bg-zinc-950/30 px-3 py-8 text-center">
                <GripVertical size={16} className="mx-auto text-zinc-700" />
                <p className="mt-2 text-[10px] text-zinc-600">Sem tarefas</p>
              </div>
            </CardContent>
          </Card>
        ))}
      </div>

      <Card>
        <CardHeader>
          <div>
            <CardTitle className="flex items-center gap-2">
              <ListChecks size={15} className="text-zinc-500" />
              Controlador da fila
            </CardTitle>
            <p className="mt-1 text-xs text-zinc-500">
              {dashboard?.queue_running ?? 0} rodando ·{" "}
              {dashboard?.queue_waiting ?? 0} aguardando
            </p>
          </div>
          <div className="flex items-center gap-2">
            <Button variant="outline" size="sm" disabled>
              <Pause size={13} />
              Pausar
            </Button>
            <Button variant="outline" size="sm" disabled>
              <Play size={13} />
              Retomar
            </Button>
            <Button variant="outline" size="sm" disabled>
              <Square size={13} />
              Cancelar
            </Button>
          </div>
        </CardHeader>
        <CardContent>
          {queue.length === 0 ? (
            <EmptyState
              icon={KanbanSquare}
              title="Fila vazia"
              description="Jobs enfileirados aparecem aqui em tempo real. A execução e o controle de concorrência entram na Fase 3."
            />
          ) : (
            <div className="space-y-2">
              {queue.map((job) => (
                <div
                  key={job.id}
                  className="flex items-center gap-3 rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2.5"
                >
                  <Badge
                    variant={
                      job.status === "RUNNING"
                        ? "blue"
                        : job.status === "FAILED"
                          ? "red"
                          : "default"
                    }
                  >
                    {job.status}
                  </Badge>
                  <div className="min-w-0 flex-1">
                    <p className="truncate text-[12px] text-zinc-200">
                      {job.title ?? job.id}
                    </p>
                    <p className="truncate font-mono text-[9px] text-zinc-600">
                      {job.resolved_agent} · {job.resolved_model} ·{" "}
                      {job.resolved_effort}
                    </p>
                  </div>
                  <span className="font-mono text-[9px] text-zinc-600">
                    {relativeTime(job.enqueued_at)}
                  </span>
                </div>
              ))}
            </div>
          )}
        </CardContent>
      </Card>
    </PageBody>
  );
}

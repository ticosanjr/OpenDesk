import { Pencil, Plus, Target, Trash2 } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { EmptyState } from "@/components/ui/empty-state";
import { goalProgress, goalTone } from "@/lib/governance";
import type { AgentGoal, GovernanceAgent } from "@/lib/types";
import { cn } from "@/lib/utils";
import { SectionTitle } from "./SectionTitle";

export function GoalsSection({
  goals,
  agents,
  busy,
  onNew,
  onEdit,
  onDelete,
}: {
  goals: AgentGoal[];
  agents: GovernanceAgent[];
  busy: string | null;
  onNew: () => void;
  onEdit: (goal: AgentGoal) => void;
  onDelete: (goal: AgentGoal) => void;
}) {
  return (
    <section className="space-y-2">
      <div className="flex items-center justify-between gap-2">
        <SectionTitle
          icon={<Target size={13} />}
          title="Metas"
          hint="OKRs e progresso por agente"
        />
        <Button variant="outline" size="sm" onClick={onNew}>
          <Plus size={11} />
          Nova meta
        </Button>
      </div>
      {goals.length === 0 ? (
        <EmptyState
          icon={Target}
          title="Nenhuma meta definida"
          description="Crie metas para orientar heartbeats, delegações e a revisão de performance."
        />
      ) : (
        <div className="grid gap-1.5 lg:grid-cols-2">
          {goals.map((goal) => {
            const progress = goalProgress(goal);
            return (
              <Card key={goal.id}>
                <CardContent className="p-3">
                  <div className="flex items-start justify-between gap-2">
                    <div className="min-w-0">
                      <div className="flex flex-wrap items-center gap-1.5">
                        <p className="text-[13px] font-medium text-zinc-200">
                          {goal.title}
                        </p>
                        <Badge variant={goalTone(goal.status)}>{goal.status}</Badge>
                        <span className="font-mono text-[11.5px] text-zinc-500">
                          @{goal.agent_id}
                        </span>
                      </div>
                      <p className="mt-0.5 line-clamp-1 text-[11.5px] text-zinc-500">
                        {goal.description || goal.metric || "Sem descrição"}
                      </p>
                    </div>
                    <div className="flex shrink-0 gap-1">
                      <Button
                        size="sm"
                        variant="ghost"
                        className="h-6 w-6 p-0"
                        aria-label={`Editar meta ${goal.title}`}
                        onClick={() => onEdit(goal)}
                      >
                        <Pencil size={11} />
                      </Button>
                      <Button
                        size="sm"
                        variant="ghost"
                        className="h-6 w-6 p-0 text-zinc-500 hover:text-red-300"
                        aria-label={`Excluir meta ${goal.title}`}
                        disabled={busy === `goal-${goal.id}`}
                        onClick={() => onDelete(goal)}
                      >
                        <Trash2 size={11} />
                      </Button>
                    </div>
                  </div>
                  <div className="mt-2 flex items-center gap-2">
                    <div className="h-1 flex-1 overflow-hidden rounded-full bg-zinc-800">
                      <div
                        className={cn(
                          "h-full rounded-full",
                          progress >= 100 ? "bg-emerald-400" : "bg-violet-400",
                        )}
                        style={{ width: `${progress}%` }}
                      />
                    </div>
                    <span className="font-mono text-[11.5px] text-zinc-500">
                      {goal.current_value}
                      {goal.target_value !== null && goal.target_value !== undefined
                        ? ` / ${goal.target_value}`
                        : ""}{" "}
                      {goal.unit ?? ""}
                    </span>
                  </div>
                  {goal.due_at ? (
                    <p className="mt-1 text-[11.5px] text-zinc-500">
                      prazo {goal.due_at}
                    </p>
                  ) : null}
                </CardContent>
              </Card>
            );
          })}
        </div>
      )}
      {agents.length === 0 ? (
        <p className="text-[11.5px] text-zinc-500">
          Crie agentes no Studio antes de cadastrar metas.
        </p>
      ) : null}
    </section>
  );
}

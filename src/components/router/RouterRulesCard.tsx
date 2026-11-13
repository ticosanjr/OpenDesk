import { GitBranch, Pencil, Plus, Route, Trash2 } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { EmptyState } from "@/components/ui/empty-state";
import type { RouterRule } from "@/lib/types";
import type { RouterConfig } from "./router-types";

export function RouterRulesCard({
  config,
  onNew,
  onEdit,
  onRemove,
}: {
  config: RouterConfig | null;
  onNew: () => void;
  onEdit: (rule: RouterRule) => void;
  onRemove: (id: string) => void;
}) {
  return (
    <Card>
      <CardHeader>
        <div>
          <CardTitle className="flex items-center gap-2">
            <GitBranch size={15} className="text-zinc-500" />
            Regras IF-THEN
          </CardTitle>
          <p className="mt-1 text-xs text-zinc-500">
            Avaliadas em ordem de prioridade; a primeira que casar define fase,
            agente, modelo e effort.
          </p>
        </div>
        <Button size="sm" onClick={onNew}>
          <Plus size={14} />
          Nova regra
        </Button>
      </CardHeader>
      <CardContent>
        {!config || config.rules.length === 0 ? (
          <EmptyState
            icon={Route}
            title="Nenhuma regra customizada"
            description='Exemplo: {"field":"tags","op":"contains","value":"roteiro"} → build/@roteirista/glm-5.3/high.'
          />
        ) : (
          <div className="space-y-2">
            {config.rules.map((rule) => (
              <div
                key={rule.id}
                className="flex flex-wrap items-center gap-2 rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2.5"
              >
                <Badge variant={rule.is_enabled ? "green" : "default"}>
                  {rule.is_enabled ? "on" : "off"}
                </Badge>
                <div className="min-w-0 flex-1">
                  <p className="text-[13px] font-medium text-zinc-200">{rule.name}</p>
                  <p className="truncate font-mono text-[11.5px] text-zinc-500">
                    {rule.condition_json}
                  </p>
                </div>
                <Badge variant="purple">{rule.target_mode}</Badge>
                <Badge variant="default">@{rule.target_agent ?? "build"}</Badge>
                <span className="font-mono text-[11.5px] text-zinc-500">
                  {rule.target_model} · {rule.target_effort}
                </span>
                <Button
                  variant="outline"
                  size="sm"
                  className="h-6 px-1.5 text-[11.5px]"
                  onClick={() => onEdit(rule)}
                  title="Editar regra"
                >
                  <Pencil size={10} />
                </Button>
                <Button
                  variant="destructive"
                  size="sm"
                  className="h-6 px-1.5 text-[11.5px]"
                  onClick={() => onRemove(rule.id)}
                >
                  <Trash2 size={10} />
                </Button>
              </div>
            ))}
          </div>
        )}
      </CardContent>
    </Card>
  );
}

import { Workflow } from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { CountPill } from "@/components/ui/count-pill";
import { EmptyState } from "@/components/ui/empty-state";
import { LoadingState } from "@/components/ui/loading-state";
import { Switch } from "@/components/ui/switch";
import { cn } from "@/lib/utils";
import type { RecipeIndex } from "./recipe-shared";

export function RecipeListCard({
  recipes,
  selected,
  loading,
  toggling,
  onSelect,
  onToggle,
}: {
  recipes: RecipeIndex[];
  selected: string | null;
  loading: boolean;
  toggling: string | null;
  onSelect: (id: string) => void;
  onToggle: (item: RecipeIndex) => void;
}) {
  return (
    <Card className="min-h-0">
      <CardHeader className="py-3">
        <CardTitle className="flex items-center gap-2">
          <Workflow size={15} className="text-zinc-400" />
          Receitas
        </CardTitle>
        <CountPill value={recipes.length} />
      </CardHeader>
      <CardContent className="max-h-[70dvh] space-y-1 overflow-y-auto p-2">
        {loading && recipes.length === 0 ? (
          <LoadingState rows={4} compact />
        ) : recipes.length === 0 ? (
          <EmptyState
            icon={Workflow}
            title="Nenhuma receita"
            description="Crie a primeira receita YAML."
            compact
            className="border-0 bg-transparent"
          />
        ) : (
          recipes.map((item) => (
            <div
              key={item.id}
              className={cn(
                "flex items-start gap-2 rounded-lg px-3 py-2.5 transition-colors",
                selected === item.id
                  ? "workspace-nav-active"
                  : "hover:bg-zinc-900",
                item.is_enabled ? undefined : "bg-zinc-950/20",
              )}
            >
              <button
                type="button"
                onClick={() => onSelect(item.id)}
                className={cn(
                  "min-w-0 flex-1 text-left",
                  selected === item.id ? "text-zinc-100" : "text-zinc-400",
                )}
              >
                <p className="truncate text-[13.5px] font-medium">{item.name}</p>
                <p className="truncate font-mono text-[13px] text-zinc-400">
                  {item.trigger_type} · {item.total_runs} execuções
                </p>
                {item.last_error ? (
                  <p className="truncate text-[13px] text-red-300">{item.last_error}</p>
                ) : null}
              </button>
              <Switch
                checked={item.is_enabled}
                onCheckedChange={() => onToggle(item)}
                disabled={toggling === item.id}
                label={
                  item.is_enabled
                    ? `Desativar receita ${item.name}`
                    : `Ativar receita ${item.name}`
                }
                className="mt-0.5"
              />
            </div>
          ))
        )}
      </CardContent>
    </Card>
  );
}

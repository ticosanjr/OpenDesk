import { GitBranch, Play, Sparkles, Trash2 } from "lucide-react";
import type { Dictionary } from "@/components/recipes/recipe-editors";
import { RecipeStepper } from "@/components/recipes/recipe-editors";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { IconLabel } from "@/components/ui/icon-label";
import { LoadingState } from "@/components/ui/loading-state";
import { cn } from "@/lib/utils";

export function RecipeFlowCard({
  recipe,
  workspaceId,
  editorLoading,
  deleteConfirm,
  selected,
  canDelete,
  onChange,
  onDelete,
  onDryRun,
  onRun,
}: {
  recipe: Dictionary | null;
  workspaceId: string | null;
  editorLoading: boolean;
  deleteConfirm: string | null;
  selected: string | null;
  canDelete: boolean;
  onChange: (next: Dictionary) => void;
  onDelete: () => void;
  onDryRun: () => void;
  onRun: () => void;
}) {
  return (
    <Card className="min-h-0 min-w-0 flex-1">
      <CardHeader className="py-3">
        <CardTitle className="flex items-center gap-2">
          <GitBranch size={15} className="text-zinc-400" />
          Fluxo visual
        </CardTitle>
        <div className="flex flex-wrap gap-1">
          <Button
            variant="ghost"
            size="sm"
            className={cn(
              "text-zinc-400",
              deleteConfirm === selected &&
                "border border-red-500/40 bg-red-500/10 text-red-300",
            )}
            onClick={onDelete}
            disabled={!canDelete}
          >
            <Trash2 size={12} />
            {deleteConfirm === selected ? "Tem certeza?" : "Excluir"}
          </Button>
          <Button
            variant="outline"
            size="sm"
            onClick={onDryRun}
            disabled={!selected}
          >
            <IconLabel icon={<Sparkles size={12} />}>Dry-run</IconLabel>
          </Button>
          <Button
            size="sm"
            onClick={onRun}
            disabled={!selected}
          >
            <IconLabel icon={<Play size={12} />}>Executar</IconLabel>
          </Button>
        </div>
      </CardHeader>
      <CardContent className="max-h-[70dvh] space-y-2 overflow-y-auto">
        {editorLoading ? (
          <LoadingState label="Carregando receita…" rows={5} />
        ) : recipe ? (
          <RecipeStepper
            recipe={recipe}
            workspaceId={workspaceId}
            onChange={onChange}
          />
        ) : (
          <p className="rounded-lg border border-red-500/20 bg-red-500/10 p-3 text-[13px] text-red-300">
            YAML inválido. Corrija no editor ao lado.
          </p>
        )}
      </CardContent>
    </Card>
  );
}

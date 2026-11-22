import { ChevronRight } from "lucide-react";
import { Link } from "react-router-dom";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { CountPill } from "@/components/ui/count-pill";
import { relativeTime } from "@/lib/format";
import { links } from "@/lib/links";
import {
  recipeRunStatusLabel,
  recipeRunStatusTone,
} from "@/lib/status";
import type { RecipeRun } from "./recipe-shared";

export function RecipeRunsCard({ runs }: { runs: RecipeRun[] }) {
  return (
    <Card>
      <CardHeader className="py-3">
        <CardTitle>Execuções de receitas</CardTitle>
        <CountPill value={runs.length} />
      </CardHeader>
      <CardContent className="space-y-1">
        {runs.length === 0 ? (
          <p className="p-2 text-[13px] text-zinc-500">Nenhuma execução ainda.</p>
        ) : (
          runs.map((run) => (
            <Link
              key={run.id}
              to={links.recipeRun(run.id)}
              className="flex flex-wrap items-center gap-2 rounded-lg border border-zinc-800/70 bg-zinc-950/30 px-3 py-2 transition-colors hover:border-blue-500/30 hover:bg-zinc-900/40"
            >
              <Badge variant={recipeRunStatusTone(run.status)}>
                {recipeRunStatusLabel(run.status)}
              </Badge>
              <span className="font-mono text-[13px] text-zinc-400">{run.recipe_id}</span>
              <span className="min-w-0 flex-1 truncate font-mono text-[13px] text-zinc-500">{run.current_step_id ?? "—"}</span>
              <span className="text-[13px] text-zinc-500">{relativeTime(run.started_at)}</span>
              <ChevronRight size={13} className="text-zinc-600" />
            </Link>
          ))
        )}
      </CardContent>
    </Card>
  );
}

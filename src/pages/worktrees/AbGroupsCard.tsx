import { ArrowLeftRight, Loader2 } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { EmptyState } from "@/components/ui/empty-state";
import type { AbGroup } from "@/lib/types";
import { abTone } from "./worktrees-shared";

export function AbGroupsCard({
  groups,
  busy,
  onOpenDetail,
}: {
  groups: AbGroup[];
  busy: string | null;
  onOpenDetail: (id: string) => void;
}) {
  return (
    <Card>
      <CardHeader>
        <CardTitle>Experimentos</CardTitle>
      </CardHeader>
      <CardContent className="p-0">
        {groups.length === 0 ? (
          <EmptyState
            icon={ArrowLeftRight}
            title="Nenhum A/B ainda"
            description="Crie um experimento para comparar dois runs isolados."
          />
        ) : (
          <div className="divide-y divide-zinc-800/60">
            {groups.map((group) => (
              <button
                key={group.id}
                type="button"
                onClick={() => onOpenDetail(group.id)}
                className="flex w-full items-center gap-3 px-4 py-3 text-left transition-colors hover:bg-zinc-900/50"
              >
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-2">
                    <span className="truncate text-[13.5px] font-medium text-zinc-200">
                      {group.title}
                    </span>
                    <Badge variant={abTone(group.status)}>{group.status}</Badge>
                    {group.winner_slot ? (
                      <Badge variant="green">vencedor {group.winner_slot.toUpperCase()}</Badge>
                    ) : null}
                  </div>
                  <p className="mt-0.5 truncate text-[12.5px] text-zinc-500">
                    {group.prompt}
                  </p>
                </div>
                {busy === `detail:${group.id}` ? (
                  <Loader2 size={14} className="animate-spin text-zinc-500" />
                ) : null}
              </button>
            ))}
          </div>
        )}
      </CardContent>
    </Card>
  );
}

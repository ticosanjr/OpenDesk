import { Badge } from "@/components/ui/badge";
import { EntityLink } from "@/components/ui/entity-link";
import type { DashboardStats } from "@/lib/types";

export function AutomationsBar({ dashboard }: { dashboard: DashboardStats | null }) {
  return (
    <div className="flex flex-wrap items-center gap-2 px-1 text-[12px] text-zinc-500">
      <Badge
        variant={
          (dashboard?.active_routines ?? 0) + (dashboard?.enabled_recipes ?? 0) > 0
            ? "yellow"
            : "default"
        }
      >
        Automações
      </Badge>
      <EntityLink to="/routines">
        {dashboard?.active_routines ?? 0} rotinas ativas
      </EntityLink>
      <span>·</span>
      <EntityLink to="/recipes">
        {dashboard?.enabled_recipes ?? 0} receitas ativas
      </EntityLink>
      <span className="text-zinc-400">
        Revise gatilhos e custos antes de deixar rodando.
      </span>
    </div>
  );
}

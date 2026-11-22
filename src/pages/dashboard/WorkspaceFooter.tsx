import { Gauge } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { EntityLink } from "@/components/ui/entity-link";
import { relativeTime } from "@/lib/format";
import { links } from "@/lib/links";
import { ROUTER_POLICY_LABELS } from "@/lib/types";
import type { Workspace } from "@/lib/types";

export function WorkspaceFooter({ workspace }: { workspace: Workspace }) {
  return (
    <div className="flex flex-wrap items-center gap-2 rounded-xl border border-zinc-800/70 bg-zinc-900/20 px-4 py-3 text-[13px] text-zinc-500">
      <Gauge size={14} className="text-zinc-500" />
      <span className="font-medium text-zinc-300">Workspace</span>
      <EntityLink to={links.file("")} className="font-mono text-[12px] text-zinc-500">
        {workspace.root_path}
      </EntityLink>
      <Badge variant="default">
        {ROUTER_POLICY_LABELS[workspace.default_router_policy] ??
          workspace.default_router_policy}
      </Badge>
      <Badge variant="default">{workspace.timezone}</Badge>
      <span className="ml-auto font-mono text-[12px] text-zinc-500">
        aberto {relativeTime(workspace.last_opened_at)}
      </span>
    </div>
  );
}

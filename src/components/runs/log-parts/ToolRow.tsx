import { useMemo, useState } from "react";

import {
  ChevronRight,
  Clock3,
  ExternalLink,
  Wrench,
  XCircle,
} from "lucide-react";
import type { TimelineTool } from "@/lib/types";
import { Badge } from "@/components/ui/badge";
import { EntityLink } from "@/components/ui/entity-link";
import { links } from "@/lib/links";
import { formatClock, formatDuration } from "@/lib/format";
import { cn } from "@/lib/utils";
import { Expandable } from "@/components/runs/log-parts/Expandable";
import {
  pretty,
  relativeToolPath,
  toolPath,
} from "@/components/runs/log-parts/helpers";

export function ToolRow({
  tool,
  workspaceRoot,
}: {
  tool: TimelineTool;
  workspaceRoot?: string | null;
}) {
  const [open, setOpen] = useState(false);
  const path = useMemo(
    () => relativeToolPath(toolPath(tool.input), workspaceRoot),
    [tool.input, workspaceRoot],
  );
  const failed = tool.status === "failed";
  return (
    <Expandable
      open={open}
      onToggle={() => setOpen((current) => !current)}
      className="border-zinc-800/70 bg-zinc-950/40"
      headerClassName="px-2.5 py-1.5"
      contentClassName="space-y-2 px-2.5 py-2"
      header={
        <>
          {failed ? (
            <XCircle size={12} className="shrink-0 text-red-400" />
          ) : (
            <Wrench size={12} className="shrink-0 text-zinc-500" />
          )}
          <span className="font-mono text-[12px] text-zinc-200">
            {tool.summary}
          </span>
          <Badge variant={failed ? "red" : "green"}>{tool.status}</Badge>
          <span className="ml-auto flex shrink-0 items-center gap-2 font-mono text-[11.5px] text-zinc-500">
            <Clock3 size={9} />
            {formatClock(tool.started_at)}
            <span>{formatDuration(tool.duration_ms)}</span>
          </span>
          <ChevronRight
            size={13}
            className={cn(
              "shrink-0 text-zinc-500 transition-transform duration-200",
              open && "rotate-90",
            )}
          />
        </>
      }
    >
      <div className="flex flex-wrap items-center gap-2">
        {path ? (
          <EntityLink to={links.file(path)} chip>
            <ExternalLink size={9} />
            abrir {path.split("/").pop()}
          </EntityLink>
        ) : null}
        <span className="font-mono text-[11.5px] text-zinc-500">
          {formatClock(tool.started_at)} → {formatClock(tool.ended_at)}
        </span>
      </div>
      <div>
        <p className="mb-1 font-mono text-[11px] uppercase tracking-wide text-zinc-500">
          entrada
        </p>
        <pre className="max-h-40 overflow-auto whitespace-pre-wrap break-words rounded border border-zinc-800/60 bg-zinc-950 p-2 font-mono text-[11.5px] text-zinc-400">
          {pretty(tool.input)}
        </pre>
      </div>
      <div>
        <p className="mb-1 font-mono text-[11px] uppercase tracking-wide text-zinc-500">
          saída
        </p>
        <pre className="max-h-56 overflow-auto whitespace-pre-wrap break-words rounded border border-zinc-800/60 bg-zinc-950 p-2 font-mono text-[11.5px] text-zinc-400">
          {tool.output || "sem saída"}
        </pre>
        {tool.output_truncated ? (
          <p className="mt-1 text-[11.5px] text-amber-300">Saída truncada.</p>
        ) : null}
      </div>
    </Expandable>
  );
}

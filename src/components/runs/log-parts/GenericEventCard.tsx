import { useState } from "react";

import { Braces, ChevronRight } from "lucide-react";
import type { TimelineEvent } from "@/lib/types";
import { Badge } from "@/components/ui/badge";
import { formatClock } from "@/lib/format";
import { cn } from "@/lib/utils";
import { Expandable } from "@/components/runs/log-parts/Expandable";
import { pretty } from "@/components/runs/log-parts/helpers";

export function GenericEventCard({ event }: { event: TimelineEvent }) {
  const [open, setOpen] = useState(false);
  return (
    <Expandable
      open={open}
      onToggle={() => setOpen((current) => !current)}
      className="border-zinc-800/70 bg-zinc-950/40"
      header={
        <>
          <Braces size={12} className="shrink-0 text-zinc-400" />
          <span className="font-mono text-[12px] text-zinc-300">
            {event.event_type || event.text}
          </span>
          <Badge variant="default">evento</Badge>
          <span className="ml-auto font-mono text-[11.5px] text-zinc-500">
            {formatClock(event.started_at)}
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
      <pre className="max-h-96 overflow-auto whitespace-pre-wrap break-words rounded-lg border border-zinc-800/60 bg-zinc-950 p-2 font-mono text-[12px] leading-relaxed text-zinc-400">
        {event.data ? pretty(event.data) : "Sem dados."}
      </pre>
    </Expandable>
  );
}

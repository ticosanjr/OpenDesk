import { useState } from "react";

import { ChevronRight, MessageSquare } from "lucide-react";
import type { TimelineEvent } from "@/lib/types";
import { formatClock, formatDuration } from "@/lib/format";
import { cn } from "@/lib/utils";
import { Expandable } from "@/components/runs/log-parts/Expandable";

export function CommentCard({ event }: { event: TimelineEvent }) {
  const [open, setOpen] = useState(true);
  return (
    <Expandable
      open={open}
      onToggle={() => setOpen((current) => !current)}
      className="border-zinc-800/70 bg-zinc-900/40"
      header={
        <>
          <MessageSquare size={12} className="shrink-0 text-blue-300" />
          <span className="text-[13px] font-semibold text-zinc-200">Resposta</span>
          <span className="font-mono text-[11.5px] text-zinc-500">
            {formatClock(event.started_at)} → {formatClock(event.ended_at)}
          </span>
          <span className="font-mono text-[11.5px] text-zinc-500">
            {formatDuration(event.duration_ms)}
          </span>
          <ChevronRight
            size={13}
            className={cn(
              "ml-auto shrink-0 text-zinc-500 transition-transform duration-200",
              open && "rotate-90",
            )}
          />
        </>
      }
    >
      <p className="whitespace-pre-wrap break-words text-[13.5px] leading-relaxed text-zinc-200">
        {event.text || "Sem texto."}
      </p>
    </Expandable>
  );
}

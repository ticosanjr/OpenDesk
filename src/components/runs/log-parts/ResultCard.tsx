import { useState } from "react";

import { CheckCircle2, ChevronRight, Copy, XCircle } from "lucide-react";
import { Button } from "@/components/ui/button";
import { formatClock, formatDuration } from "@/lib/format";
import { cn } from "@/lib/utils";
import { Expandable } from "@/components/runs/log-parts/Expandable";

export function ResultCard({
  text,
  summary,
  failed,
  durationMs,
  startedAt,
  endedAt,
  truncated,
  copy,
}: {
  text: string;
  summary?: string;
  failed: boolean;
  durationMs: number;
  startedAt?: string | null;
  endedAt?: string | null;
  truncated: boolean;
  copy: (text: string) => Promise<void>;
}) {
  const [open, setOpen] = useState(true);
  return (
    <Expandable
      open={open}
      onToggle={() => setOpen((current) => !current)}
      className={cn(
        failed
          ? "border-red-500/20 bg-red-500/[0.035]"
          : "border-emerald-500/20 bg-emerald-500/[0.035]",
      )}
      actions={
        <Button
          variant="ghost"
          size="sm"
          className="h-6"
          onClick={() => void copy(text || summary || "")}
        >
          <Copy size={11} />
          copiar
        </Button>
      }
      header={
        <>
          {failed ? (
            <XCircle size={14} className="text-red-400" />
          ) : (
            <CheckCircle2 size={14} className="text-emerald-400" />
          )}
          <span
            className={cn(
              "text-[13.5px] font-semibold",
              failed ? "text-red-100" : "text-emerald-100",
            )}
          >
            Resultado
          </span>
          <span className="font-mono text-[12px] text-zinc-500">
            {formatDuration(durationMs)}
          </span>
          <span className="hidden font-mono text-[12px] text-zinc-500 sm:inline">
            {formatClock(startedAt)} → {formatClock(endedAt)}
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
      <pre className="whitespace-pre-wrap break-words font-sans text-[13.5px] leading-relaxed text-zinc-100">
        {text || summary || "Sem texto final. Veja as ferramentas executadas e o modo bruto."}
      </pre>
      {truncated ? (
        <p className="mt-2 text-[12px] text-amber-300">
          Resultado truncado. Use o modo bruto para ver o log completo.
        </p>
      ) : null}
    </Expandable>
  );
}

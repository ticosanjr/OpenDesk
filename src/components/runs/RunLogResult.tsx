import { CheckCircle2, Copy, XCircle } from "lucide-react";
import { Button } from "@/components/ui/button";
import { formatClock, formatDuration } from "@/lib/format";
import { cn } from "@/lib/utils";
import type { RunTimeline } from "@/lib/types";

export function RunLogResult({
  timeline,
  summary,
  failedRun,
  onCopy,
}: {
  timeline: RunTimeline | null;
  summary: string | null | undefined;
  failedRun: boolean;
  onCopy: (text: string) => void;
}) {
  return (
    <div
      className={cn(
        "rounded-xl border",
        failedRun
          ? "border-red-500/20 bg-red-500/[0.035]"
          : "border-emerald-500/20 bg-emerald-500/[0.035]",
      )}
    >
      <div
        className={cn(
          "flex flex-wrap items-center gap-2 border-b px-3.5 py-2.5",
          failedRun ? "border-red-500/15" : "border-emerald-500/15",
        )}
      >
        {failedRun ? (
          <XCircle size={14} className="text-red-400" />
        ) : (
          <CheckCircle2 size={14} className="text-emerald-400" />
        )}
        <span
          className={cn(
            "text-[13px] font-semibold",
            failedRun ? "text-red-100" : "text-emerald-100",
          )}
        >
          Resultado
        </span>
        <span className="font-mono text-[11.5px] text-zinc-500">
          {formatDuration(timeline?.result.duration_ms ?? 0)}
        </span>
        <span className="font-mono text-[11.5px] text-zinc-500">
          {formatClock(timeline?.result.started_at)} →{" "}
          {formatClock(timeline?.result.ended_at)}
        </span>
        <Button
          variant="ghost"
          size="sm"
          className="ml-auto h-6"
          onClick={() => onCopy(timeline?.result.text ?? summary ?? "")}
        >
          <Copy size={11} />
          copiar
        </Button>
      </div>
      <div className="px-3.5 py-3">
        <pre className="whitespace-pre-wrap break-words font-sans text-[13px] leading-relaxed text-zinc-200">
          {timeline?.result.text?.trim() ||
            summary ||
            "Sem texto final. Veja as ferramentas executadas e o modo bruto."}
        </pre>
        {timeline?.result.truncated ? (
          <p className="mt-2 text-[11.5px] text-amber-300">
            Resultado truncado. Use o modo bruto para ver o log completo.
          </p>
        ) : null}
      </div>
    </div>
  );
}

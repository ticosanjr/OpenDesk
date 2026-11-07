import { Loader2 } from "lucide-react";
import { cn } from "@/lib/utils";

/**
 * Animated loading feedback. A bare "Carregando…" text left users unsure if
 * the app was alive; this gives constant motion plus optional skeletons.
 */
export function LoadingState({
  label = "Carregando…",
  className,
  rows = 0,
  compact = false,
}: {
  label?: string;
  className?: string;
  /** Number of shimmer rows rendered below the spinner. */
  rows?: number;
  compact?: boolean;
}) {
  return (
    <div
      role="status"
      aria-live="polite"
      className={cn(
        "flex w-full flex-col items-center justify-center gap-2 text-zinc-500",
        compact ? "py-4" : "py-10",
        className,
      )}
    >
      <span className="inline-flex items-center gap-2 text-[11px] font-medium">
        <Loader2 size={15} className="animate-spin text-blue-400" />
        {label}
      </span>
      {rows > 0 ? (
        <div className="mt-1 w-full max-w-md space-y-1.5">
          {Array.from({ length: rows }).map((_, index) => (
            <div
              key={index}
              className="h-2.5 animate-pulse rounded-full bg-zinc-800/80"
              style={{ width: `${88 - index * 12}%` }}
            />
          ))}
        </div>
      ) : null}
    </div>
  );
}

/** Inline shimmer bar for headers and toolbars. */
export function LoadingBar({ className }: { className?: string }) {
  return (
    <span
      className={cn(
        "relative block h-1 w-full overflow-hidden rounded-full bg-zinc-900",
        className,
      )}
    >
      <span className="absolute inset-y-0 w-1/3 animate-[loading_1.2s_ease-in-out_infinite] rounded-full bg-blue-500/70" />
    </span>
  );
}

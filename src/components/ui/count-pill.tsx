import { cn } from "@/lib/utils";

const TONES: Record<string, string> = {
  default: "border-zinc-700/70 bg-zinc-800/80 text-zinc-200",
  blue: "border-blue-500/30 bg-blue-500/15 text-blue-200",
  green: "border-emerald-500/30 bg-emerald-500/15 text-emerald-200",
  yellow: "border-amber-500/30 bg-amber-500/15 text-amber-200",
  red: "border-red-500/30 bg-red-500/15 text-red-200",
  purple: "border-violet-500/30 bg-violet-500/15 text-violet-200",
};

/** Prominent numeric counter used next to labels, tabs and artifact links. */
export function CountPill({
  value,
  tone = "default",
  className,
  title,
}: {
  value: number | string;
  tone?: keyof typeof TONES | string;
  className?: string;
  title?: string;
}) {
  return (
    <span
      title={title}
      className={cn(
        "inline-flex min-w-[1.35rem] items-center justify-center rounded-full border px-1.5 py-px font-mono text-[10px] font-semibold leading-[1.35] tabular-nums",
        TONES[tone] ?? TONES.default,
        className,
      )}
    >
      {value}
    </span>
  );
}

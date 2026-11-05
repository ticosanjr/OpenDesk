import { cn } from "@/lib/utils";

export type DotTone = "online" | "offline" | "starting" | "warning";

const tones: Record<DotTone, string> = {
  online: "bg-emerald-400 shadow-[0_0_10px_rgba(52,211,153,0.65)]",
  offline: "bg-red-400 shadow-[0_0_10px_rgba(248,113,113,0.55)]",
  starting: "bg-amber-400 shadow-[0_0_10px_rgba(251,191,36,0.55)] animate-pulse",
  warning: "bg-amber-400 shadow-[0_0_10px_rgba(251,191,36,0.55)]",
};

export function StatusDot({
  tone,
  className,
}: {
  tone: DotTone;
  className?: string;
}) {
  return (
    <span
      className={cn("inline-block h-2 w-2 shrink-0 rounded-full", tones[tone], className)}
    />
  );
}

export function statusTone(online: boolean, state?: string): DotTone {
  if (online) return "online";
  if (state === "starting" || state === "restarting") return "starting";
  return "offline";
}

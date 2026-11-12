import { cn } from "@/lib/utils";

export type DotTone = "online" | "offline" | "starting" | "warning";

const tones: Record<DotTone, string> = {
  online: "bg-emerald-400 ring-2 ring-emerald-400/20",
  offline: "bg-red-400 ring-2 ring-red-400/20",
  starting: "bg-amber-400 ring-2 ring-amber-400/20 animate-pulse",
  warning: "bg-amber-400 ring-2 ring-amber-400/20",
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

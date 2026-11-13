import {
  AlertTriangle,
  CheckCircle2,
  Clock3,
  Loader2,
  XCircle,
  type LucideIcon,
} from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";
import { runStatusLabel, runStatusTone, type StatusTone } from "@/lib/status";

export interface RunStatusMeta {
  label: string;
  tone: StatusTone;
  icon: LucideIcon;
  spin: boolean;
}

const LABELS: Record<string, string> = {
  DONE: "concluída",
  COMPLETED: "concluída",
  QUEUED: "na fila",
  PAUSED: "pausada",
  BLOCKED: "bloqueada",
  MISSED: "perdida",
  REJECTED: "rejeitada",
  WAITING: "aguardando",
  WAITING_INPUT: "aguardando",
  WAITING_APPROVAL: "aguardando",
};

/** Single source of truth for run/execution icons, colors and labels. */
export function runStatusMeta(status: string): RunStatusMeta {
  const normalized = (status || "").toUpperCase();
  const tone = runStatusTone(normalized);
  const label =
    LABELS[normalized] ??
    (runStatusLabel(normalized) || normalized.toLowerCase());
  if (normalized === "RUNNING") {
    return { label, tone: "blue", icon: Loader2, spin: true };
  }
  if (["SUCCESS", "DONE", "COMPLETED"].includes(normalized)) {
    return { label, tone: "green", icon: CheckCircle2, spin: false };
  }
  if (["FAILED", "REJECTED"].includes(normalized)) {
    return { label, tone: "red", icon: XCircle, spin: false };
  }
  if (normalized === "CANCELLED") {
    return { label, tone: "default", icon: XCircle, spin: false };
  }
  if (["BLOCKED", "MISSED"].includes(normalized)) {
    return { label, tone: "yellow", icon: AlertTriangle, spin: false };
  }
  if (
    ["PENDING", "QUEUED", "PAUSED", "WAITING", "WAITING_INPUT", "WAITING_APPROVAL"].includes(
      normalized,
    )
  ) {
    return { label, tone: "yellow", icon: Clock3, spin: false };
  }
  return { label, tone, icon: Clock3, spin: false };
}

export const RUN_STATUS_ICON_CLASS: Record<StatusTone, string> = {
  default: "text-zinc-500",
  blue: "text-blue-400",
  green: "text-emerald-400",
  yellow: "text-amber-400",
  red: "text-red-400",
  purple: "text-violet-400",
};

export function RunStatusIcon({
  status,
  size = 14,
  className,
}: {
  status: string;
  size?: number;
  className?: string;
}) {
  const meta = runStatusMeta(status);
  const Icon = meta.icon;
  return (
    <Icon
      size={size}
      className={cn(
        "shrink-0",
        RUN_STATUS_ICON_CLASS[meta.tone],
        meta.spin && "animate-spin",
        className,
      )}
      aria-hidden
    />
  );
}

export function RunStatusBadge({ status }: { status: string }) {
  const meta = runStatusMeta(status);
  return <Badge variant={meta.tone}>{meta.label}</Badge>;
}

import type { LucideIcon } from "lucide-react";
import {
  AlertTriangle,
  CheckCircle2,
  Circle,
  ClipboardCheck,
  Clock3,
  Cog,
  MessageCircle,
  Pause,
  XCircle,
} from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Tooltip } from "@/components/ui/tooltip";
import { cn } from "@/lib/utils";
import { workflowMeta, type WorkflowLike } from "@/lib/workflow";

const ICONS: Record<string, LucideIcon> = {
  READY: Circle,
  QUEUED: Clock3,
  HELD: Pause,
  RUNNING: Cog,
  NEEDS_APPROVAL: ClipboardCheck,
  WAITING_INPUT: MessageCircle,
  BLOCKED: AlertTriangle,
  DONE: CheckCircle2,
  CANCELLED: XCircle,
};

/**
 * Single source of visual truth for task workflow state. Every screen must use
 * this badge instead of inventing its own status/phase chips.
 */
export function WorkflowStatusBadge({
  task,
  className,
  showReason = false,
}: {
  task: WorkflowLike;
  className?: string;
  showReason?: boolean;
}) {
  const meta = workflowMeta(task);
  const Icon = ICONS[meta.state] ?? Circle;
  return (
    <Tooltip content={meta.tooltip}>
      <Badge variant={meta.tone} className={cn("max-w-full", className)}>
        <Icon
          size={10}
          className={meta.state === "RUNNING" ? "opendesk-gear shrink-0" : "shrink-0"}
        />
        <span className="truncate">{meta.label}</span>
        {showReason && meta.reason !== meta.label ? (
          <span className="max-w-[120px] truncate normal-case tracking-normal text-current/70">
            · {meta.reason}
          </span>
        ) : null}
      </Badge>
    </Tooltip>
  );
}

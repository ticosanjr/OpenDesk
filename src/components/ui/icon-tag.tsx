import type { LucideIcon } from "lucide-react";
import {
  CalendarClock,
  GitBranch,
  Scale,
  SlidersHorizontal,
  Sparkles,
  Terminal,
  User,
  Webhook,
  Workflow,
  Zap,
} from "lucide-react";
import { Tooltip } from "@/components/ui/tooltip";
import { cn } from "@/lib/utils";

export function IconTag({
  icon: Icon,
  label,
  className,
}: {
  icon: LucideIcon;
  label: string;
  className?: string;
}) {
  return (
    <Tooltip content={label}>
      <span
        role="img"
        aria-label={label}
        tabIndex={0}
        className={cn(
          "inline-flex h-6 w-6 shrink-0 items-center justify-center rounded-md border border-zinc-800/80 bg-zinc-900/40 text-zinc-400 outline-none transition-colors hover:border-zinc-700 hover:text-zinc-200 focus-visible:ring-2 focus-visible:ring-blue-500/50",
          className,
        )}
      >
        <Icon size={14} strokeWidth={1.8} />
      </span>
    </Tooltip>
  );
}

const ROUTER_POLICY_ICONS: Record<string, { icon: LucideIcon; label: string }> = {
  COST_BENEFIT: { icon: Scale, label: "melhor custo-benefício" },
  MAX_QUALITY: { icon: Sparkles, label: "máxima qualidade" },
  FASTEST: { icon: Zap, label: "mais rápido/barato" },
  MANUAL: { icon: SlidersHorizontal, label: "manual" },
};

export function routerPolicyTag(policy: string) {
  return (
    ROUTER_POLICY_ICONS[policy] ?? {
      icon: SlidersHorizontal,
      label: policy || "política manual",
    }
  );
}

const ORIGIN_ICONS: Record<string, { icon: LucideIcon; label: string }> = {
  RECIPE: { icon: Workflow, label: "origem: receita" },
  ROUTINE: { icon: CalendarClock, label: "origem: rotina" },
  IO_BUS: { icon: Webhook, label: "origem: I/O Bus" },
  DELEGATION: { icon: GitBranch, label: "origem: delegação" },
  PROMPT: { icon: Terminal, label: "origem: prompt" },
  MANUAL: { icon: User, label: "origem: manual" },
};

export function originTag(origin: string) {
  return ORIGIN_ICONS[origin] ?? { icon: Terminal, label: `origem: ${origin}` };
}

const TRIGGER_ICONS: Record<string, { icon: LucideIcon; label: string }> = {
  CRON: { icon: CalendarClock, label: "gatilho: agenda cron" },
  MANUAL: { icon: User, label: "gatilho: manual" },
  WEBHOOK: { icon: Webhook, label: "gatilho: webhook" },
  FOLDER_WATCH: { icon: Workflow, label: "gatilho: pasta" },
  CALENDAR: { icon: CalendarClock, label: "gatilho: calendário" },
};

export function triggerTag(trigger: string) {
  return TRIGGER_ICONS[trigger] ?? { icon: Webhook, label: `gatilho: ${trigger}` };
}

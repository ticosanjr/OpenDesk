import { Link } from "react-router-dom";
import {
  CalendarDays,
  Cloud,
  HardDrive,
  Mail,
  MessageCircle,
  Plug,
  Send,
} from "lucide-react";
import type { IntegrationOverviewItem } from "@/lib/types";
import { cn } from "@/lib/utils";

const ICONS: Record<string, typeof Plug> = {
  smtp: Mail,
  google: Cloud,
  drive: HardDrive,
  calendar: CalendarDays,
  signal: MessageCircle,
  telegram: Send,
};

export function IntegrationFootprint({
  items,
}: {
  items: IntegrationOverviewItem[];
}) {
  if (items.length === 0) return null;
  return (
    <div className="flex flex-wrap items-center gap-2 rounded-xl border border-zinc-800/60 bg-zinc-950/30 px-3 py-2 text-[11.5px] text-zinc-500">
      <span className="mr-1 inline-flex items-center gap-1.5 font-medium uppercase tracking-[0.1em]">
        <Plug size={11} />
        integrações
      </span>
      {items.map((item) => {
        const Icon = ICONS[item.id] ?? Plug;
        return (
          <Link
            key={item.id}
            to={item.href}
            title={[
              item.label,
              item.active ? "ligada" : item.configured ? "configurada, parada" : "desligada",
              item.detail,
            ]
              .filter(Boolean)
              .join(" · ")}
            className={cn(
              "inline-flex max-w-full items-center gap-1.5 rounded-md border px-2 py-1 transition-colors",
              item.active
                ? "border-emerald-500/25 bg-emerald-500/[0.06] text-emerald-200/90 hover:border-emerald-500/40"
                : item.configured
                  ? "border-amber-500/20 bg-amber-500/[0.04] text-amber-200/80 hover:border-amber-500/35"
                  : "border-zinc-800 bg-zinc-900/30 text-zinc-500 hover:border-zinc-700 hover:text-zinc-300",
            )}
          >
            <span
              className={cn(
                "h-1.5 w-1.5 shrink-0 rounded-full",
                item.active
                  ? "bg-emerald-400"
                  : item.configured
                    ? "bg-amber-400"
                    : "bg-zinc-600",
              )}
            />
            <Icon size={11} className="shrink-0" />
            <span className="truncate">{item.label}</span>
            {item.detail ? (
              <span className="hidden max-w-32 truncate font-mono text-[10.5px] opacity-70 sm:inline">
                {item.detail}
              </span>
            ) : null}
          </Link>
        );
      })}
    </div>
  );
}

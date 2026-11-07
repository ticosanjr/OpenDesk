import type { LucideIcon } from "lucide-react";
import { cn } from "@/lib/utils";

export interface SectionTabItem {
  id: string;
  label: string;
  icon?: LucideIcon;
  badge?: string | number;
}

export function SectionTabs({
  items,
  value,
  onChange,
  className,
}: {
  items: SectionTabItem[];
  value: string;
  onChange: (id: string) => void;
  className?: string;
}) {
  return (
    <div
      role="tablist"
      aria-label="Seções"
      className={cn(
        "sticky top-0 z-20 -mx-5 flex flex-wrap gap-1 border-b border-zinc-800/60 bg-[#09090b]/95 px-5 py-2 backdrop-blur lg:-mx-7 lg:px-7",
        className,
      )}
    >
      {items.map((item) => {
        const Icon = item.icon;
        const active = item.id === value;
        return (
          <button
            key={item.id}
            type="button"
            role="tab"
            aria-selected={active}
            onClick={() => onChange(item.id)}
            className={cn(
              "inline-flex items-center gap-1.5 rounded-lg px-2.5 py-1.5 text-[10px] font-medium transition-colors",
              active
                ? "bg-zinc-800 text-zinc-100"
                : "text-zinc-500 hover:bg-zinc-900 hover:text-zinc-200",
            )}
          >
            {Icon ? <Icon size={12} /> : null}
            {item.label}
            {item.badge !== undefined ? (
              <span className="rounded bg-zinc-900 px-1 font-mono text-[9px] text-zinc-400">
                {item.badge}
              </span>
            ) : null}
          </button>
        );
      })}
    </div>
  );
}

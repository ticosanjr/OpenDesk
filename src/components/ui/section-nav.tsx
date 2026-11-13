import type { LucideIcon } from "lucide-react";
import { cn } from "@/lib/utils";

export interface SectionNavItem {
  id: string;
  label: string;
  icon?: LucideIcon;
  group?: string;
}

/** Grouped vertical navigation used by settings-like surfaces. */
export function SectionNav({
  items,
  value,
  onChange,
  ariaLabel = "Seções",
}: {
  items: ReadonlyArray<SectionNavItem>;
  value: string;
  onChange: (id: string) => void;
  ariaLabel?: string;
}) {
  const groups = Array.from(
    new Set(items.map((item) => item.group ?? "Geral")),
  );
  return (
    <>
    <nav
      role="tablist"
      aria-label={ariaLabel}
      aria-orientation="vertical"
      className="hidden space-y-1 lg:sticky lg:top-0 lg:block"
    >
      {groups.map((group) => (
        <div key={group} className="space-y-0.5">
          <p className="px-2.5 pb-1 pt-3 font-mono text-[13px] uppercase tracking-[0.1em] text-zinc-500">
            {group}
          </p>
          {items
            .filter((item) => (item.group ?? "Geral") === group)
            .map((item) => {
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
                    "flex w-full items-center gap-2.5 rounded-lg px-2.5 py-2 text-left text-[14px] font-medium transition-colors",
                    active
                      ? "workspace-nav-active text-zinc-100"
                      : "text-zinc-500 hover:bg-zinc-900/70 hover:text-zinc-200",
                  )}
                >
                  {Icon ? (
                    <Icon size={15} strokeWidth={1.8} className="shrink-0" />
                  ) : null}
                  <span className="truncate">{item.label}</span>
                </button>
              );
            })}
        </div>
      ))}
    </nav>
    <div className="lg:hidden">
      <select
        value={value}
        onChange={(event) => onChange(event.target.value)}
        aria-label={ariaLabel}
        className="h-9 w-full rounded-lg border border-zinc-800 bg-zinc-900/50 px-2.5 text-[14px] text-zinc-200 outline-none focus:border-blue-500/60"
      >
        {groups.map((group) => (
          <optgroup key={group} label={group}>
            {items
              .filter((item) => (item.group ?? "Geral") === group)
              .map((item) => (
                <option key={item.id} value={item.id}>
                  {item.label}
                </option>
              ))}
          </optgroup>
        ))}
      </select>
    </div>
    </>
  );
}

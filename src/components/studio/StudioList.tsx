import type { ReactNode } from "react";
import { Plus } from "lucide-react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

export function StudioList({
  title,
  count,
  onNew,
  newLabel = "Novo",
  children,
}: {
  title: string;
  count: number;
  onNew: () => void;
  newLabel?: string;
  children: ReactNode;
}) {
  return (
    <div className="flex min-h-0 w-[260px] shrink-0 flex-col border-r border-zinc-800/70">
      <div className="flex items-center justify-between gap-2 border-b border-zinc-800/60 px-3 py-2.5">
        <div className="flex items-center gap-2">
          <span className="text-[11px] font-medium text-zinc-300">{title}</span>
          <span className="rounded border border-zinc-800 bg-zinc-900 px-1.5 py-0.5 font-mono text-[9px] text-zinc-500">
            {count}
          </span>
        </div>
        <Button variant="ghost" size="icon" className="h-7 w-7" onClick={onNew} title={newLabel}>
          <Plus size={14} />
        </Button>
      </div>
      <div className="min-h-0 flex-1 overflow-y-auto p-1.5">{children}</div>
    </div>
  );
}

export function StudioListItem({
  selected,
  title,
  subtitle,
  badge,
  onClick,
}: {
  selected: boolean;
  title: string;
  subtitle?: string;
  badge?: ReactNode;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        "mb-1 w-full rounded-lg border px-2.5 py-2 text-left transition-colors",
        selected
          ? "border-blue-500/30 bg-blue-500/10"
          : "border-transparent hover:border-zinc-800 hover:bg-zinc-900/60",
      )}
    >
      <div className="flex items-center justify-between gap-2">
        <span className="truncate font-mono text-[11px] text-zinc-200">{title}</span>
        {badge}
      </div>
      {subtitle ? (
        <p className="mt-0.5 truncate text-[9px] text-zinc-600">{subtitle}</p>
      ) : null}
    </button>
  );
}

export function StudioEmptyEditor({
  icon: Icon,
  title,
  description,
}: {
  icon: React.ComponentType<{ size?: number; className?: string }>;
  title: string;
  description: string;
}) {
  return (
    <div className="flex h-full flex-col items-center justify-center px-8 text-center">
      <div className="flex h-10 w-10 items-center justify-center rounded-lg border border-zinc-800 bg-zinc-900/60 text-zinc-500">
        <Icon size={18} />
      </div>
      <p className="mt-3 text-[12px] font-medium text-zinc-300">{title}</p>
      <p className="mt-1 max-w-sm text-[10px] leading-relaxed text-zinc-600">
        {description}
      </p>
    </div>
  );
}

export function StudioError({ error }: { error: string | null }) {
  if (!error) return null;
  return (
    <p className="mb-3 rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[11px] text-red-300">
      {error}
    </p>
  );
}

export function StudioField({
  label,
  hint,
  children,
}: {
  label: string;
  hint?: string;
  children: ReactNode;
}) {
  return (
    <label className="block">
      <span className="mb-1.5 block text-[10px] font-medium uppercase tracking-[0.12em] text-zinc-500">
        {label}
      </span>
      {children}
      {hint ? <p className="mt-1 text-[9px] leading-relaxed text-zinc-600">{hint}</p> : null}
    </label>
  );
}

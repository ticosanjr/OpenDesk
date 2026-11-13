import type { ReactNode } from "react";
import { Search, X } from "lucide-react";
import { cn } from "@/lib/utils";

/** Shared toolbar shell so filter rows keep the same height and rhythm. */
export function FilterBar({
  children,
  className,
}: {
  children: ReactNode;
  className?: string;
}) {
  return (
    <div
      className={cn(
        "flex flex-wrap items-center gap-2 rounded-xl border border-zinc-800/70 bg-zinc-900/20 px-2.5 py-2",
        className,
      )}
    >
      {children}
    </div>
  );
}

export function FilterSearch({
  value,
  onChange,
  placeholder,
  ariaLabel,
  className,
  autoFocus = false,
}: {
  value: string;
  onChange: (value: string) => void;
  placeholder: string;
  ariaLabel: string;
  className?: string;
  autoFocus?: boolean;
}) {
  return (
    <div
      className={cn(
        "flex h-8 min-w-[200px] flex-1 items-center gap-2 rounded-lg border border-zinc-800 bg-zinc-950/50 px-2.5 transition-colors focus-within:border-zinc-600",
        className,
      )}
    >
      <Search size={13} className="shrink-0 text-zinc-500" />
      <input
        autoFocus={autoFocus}
        value={value}
        onChange={(event) => onChange(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === "Escape" && value) {
            event.stopPropagation();
            onChange("");
          }
        }}
        placeholder={placeholder}
        aria-label={ariaLabel}
        className="h-full min-w-0 flex-1 bg-transparent text-[13.5px] text-zinc-100 outline-none placeholder:text-zinc-500"
      />
      {value ? (
        <button
          type="button"
          onClick={() => onChange("")}
          aria-label="Limpar busca"
          title="Limpar busca"
          className="rounded p-0.5 text-zinc-500 transition-colors hover:bg-zinc-800 hover:text-zinc-200"
        >
          <X size={12} />
        </button>
      ) : null}
    </div>
  );
}

export function FilterSelect({
  value,
  onChange,
  ariaLabel,
  children,
  className,
}: {
  value: string;
  onChange: (value: string) => void;
  ariaLabel: string;
  children: ReactNode;
  className?: string;
}) {
  return (
    <select
      value={value}
      onChange={(event) => onChange(event.target.value)}
      aria-label={ariaLabel}
      className={cn(
        "h-8 rounded-lg border border-zinc-800 bg-zinc-900/50 px-2 text-[13.5px] text-zinc-300 outline-none transition-colors hover:border-zinc-700 focus:border-zinc-600",
        className,
      )}
    >
      {children}
    </select>
  );
}

import { cn } from "@/lib/utils";

export function Switch({
  checked,
  onCheckedChange,
  disabled = false,
  label,
  className,
}: {
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
  disabled?: boolean;
  label: string;
  className?: string;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      title={label}
      disabled={disabled}
      onClick={(event) => {
        event.stopPropagation();
        onCheckedChange(!checked);
      }}
      className={cn(
        "workspace-switch relative inline-flex h-5 w-9 shrink-0 items-center rounded-full border transition-colors outline-none focus-visible:ring-2 focus-visible:ring-[color-mix(in_srgb,var(--workspace-accent)_60%,transparent)] disabled:cursor-not-allowed disabled:opacity-50",
        checked
          ? "workspace-switch-on"
          : "border-zinc-700 bg-zinc-800",
        className,
      )}
    >
      <span
        aria-hidden
        className={cn(
          "pointer-events-none block h-3.5 w-3.5 rounded-full bg-zinc-100 shadow transition-transform",
          checked ? "translate-x-[18px]" : "translate-x-[2px]",
        )}
      />
    </button>
  );
}

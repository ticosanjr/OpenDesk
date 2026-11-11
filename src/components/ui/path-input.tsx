import { FolderOpen } from "lucide-react";
import { pickFolder } from "@/lib/folder";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { FieldHint } from "@/components/ui/field-hint";

/**
 * Path field with a native folder picker. Every local path field should use
 * this instead of a bare input so users never have to type paths by hand.
 */
export function PathInput({
  value,
  onChange,
  placeholder = "Selecione ou digite o caminho",
  title = "Escolher pasta",
  className,
  hint,
  disabled,
  inputClassName,
}: {
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  title?: string;
  className?: string;
  hint?: string;
  disabled?: boolean;
  inputClassName?: string;
}) {
  async function choose() {
    const selected = await pickFolder(title, value || undefined);
    if (selected) onChange(selected);
  }

  return (
    <div className={cn("flex min-w-0 items-center gap-1.5", className)}>
      <input
        value={value}
        onChange={(event) => onChange(event.target.value)}
        placeholder={placeholder}
        disabled={disabled}
        spellCheck={false}
        className={cn(
          "h-9 min-w-0 flex-1 rounded-lg border border-zinc-800 bg-zinc-900/50 px-2.5 font-mono text-[11px] text-zinc-100 outline-none placeholder:font-sans placeholder:text-zinc-500 focus:border-blue-500/60 disabled:opacity-50",
          inputClassName,
        )}
      />
      <Button
        type="button"
        variant="outline"
        size="sm"
        className="h-9 shrink-0 px-2"
        onClick={() => void choose()}
        disabled={disabled}
        title={title}
        aria-label={title}
      >
        <FolderOpen size={14} />
        <span className="hidden xl:inline">Escolher</span>
      </Button>
      {hint ? <FieldHint hint={hint} /> : null}
    </div>
  );
}

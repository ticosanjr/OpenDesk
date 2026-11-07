import { CircleHelp } from "lucide-react";
import type { ReactNode } from "react";
import { Tooltip } from "@/components/ui/tooltip";
import { cn } from "@/lib/utils";

/**
 * Small "?" affordance for fields whose meaning is not obvious to a final
 * user. Hovering or focusing shows the explanation.
 */
export function FieldHint({
  hint,
  className,
  side = "inline",
}: {
  hint: ReactNode;
  className?: string;
  side?: "inline" | "trailing";
}) {
  if (!hint) return null;
  return (
    <Tooltip content={hint}>
      <button
        type="button"
        tabIndex={0}
        aria-label="Ajuda sobre este campo"
        className={cn(
          "inline-flex shrink-0 items-center justify-center rounded-full text-zinc-500 transition-colors hover:text-zinc-200 focus:outline-none focus-visible:text-zinc-100",
          side === "trailing" && "ml-1",
          className,
        )}
      >
        <CircleHelp size={13} />
      </button>
    </Tooltip>
  );
}

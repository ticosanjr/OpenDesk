import type { ReactNode } from "react";
import { cn } from "@/lib/utils";

/**
 * Keeps an icon and its label on the same line. The UI had several buttons
 * where the icon wrapped above the text at narrow widths.
 */
export function IconLabel({
  icon,
  children,
  className,
  gap = "normal",
}: {
  icon: ReactNode;
  children?: ReactNode;
  className?: string;
  gap?: "tight" | "normal";
}) {
  return (
    <span
      className={cn(
        "inline-flex min-w-0 items-center whitespace-nowrap",
        gap === "tight" ? "gap-1" : "gap-1.5",
        className,
      )}
    >
      <span className="inline-flex shrink-0 items-center justify-center [&>svg]:block">
        {icon}
      </span>
      {children !== undefined && children !== null && children !== "" ? (
        <span className="truncate">{children}</span>
      ) : null}
    </span>
  );
}

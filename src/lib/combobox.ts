import type { FocusEvent } from "react";

/**
 * A combobox should stay open while focus moves into its own dropdown.
 * Timers and unconditional blur closes make clicks race against React.
 */
export function shouldCloseOnBlur(event: FocusEvent<HTMLElement>): boolean {
  const next = event.relatedTarget;
  if (!(next instanceof Node)) return true;
  return !event.currentTarget.parentElement?.contains(next);
}

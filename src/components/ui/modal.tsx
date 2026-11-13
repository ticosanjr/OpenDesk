import * as React from "react";
import { X } from "lucide-react";
import { cn } from "@/lib/utils";

export function Modal({
  open,
  onClose,
  title,
  description,
  children,
  className,
  size = "lg",
  initialFocus = "field",
}: {
  open: boolean;
  onClose: () => void;
  title: string;
  description?: string;
  children: React.ReactNode;
  className?: string;
  /** Dialog width. `xl` keeps the large working dialogs; short confirms use `sm`. */
  size?: "sm" | "md" | "lg" | "xl";
  /** "field" focuses the first form control; "dialog" avoids scrolling long detail views. */
  initialFocus?: "field" | "dialog";
}) {
  const dialogRef = React.useRef<HTMLDivElement | null>(null);
  const onCloseRef = React.useRef(onClose);
  const titleId = React.useId();
  const descriptionId = React.useId();

  React.useEffect(() => {
    onCloseRef.current = onClose;
  }, [onClose]);

  React.useEffect(() => {
    if (!open) return;
    const previous = document.activeElement as HTMLElement | null;
    const dialog = dialogRef.current;
    const focusable = () =>
      Array.from(
        dialog?.querySelectorAll<HTMLElement>(
          'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])',
        ) ?? [],
      ).filter((element) => element.offsetParent !== null);
    // Focus the first form field on open, never the close button. The effect
    // intentionally depends only on `open`: parent re-renders must not steal
    // focus while the user is typing.
    const timer = window.setTimeout(() => {
      const active = document.activeElement as HTMLElement | null;
      // Respect a child's autoFocus (React focuses it before this timer).
      if (active && active !== document.body && dialog?.contains(active)) {
        return;
      }
      if (initialFocus === "dialog") {
        dialog?.focus({ preventScroll: true });
        return;
      }
      const items = focusable();
      const preferred = items.find((element) =>
        element.matches("input, textarea, select, [autofocus]"),
      );
      (preferred ?? dialog)?.focus({ preventScroll: true });
    }, 0);

    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        onCloseRef.current();
        return;
      }
      if (event.key !== "Tab") return;
      const items = focusable();
      if (items.length === 0) {
        event.preventDefault();
        dialog?.focus();
        return;
      }
      const first = items[0];
      const last = items[items.length - 1];
      const active = document.activeElement as HTMLElement | null;
      if (active === dialog || !dialog?.contains(active)) {
        event.preventDefault();
        (event.shiftKey ? last : first).focus();
      } else if (event.shiftKey && active === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && active === last) {
        event.preventDefault();
        first.focus();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => {
      window.clearTimeout(timer);
      window.removeEventListener("keydown", onKeyDown);
      previous?.focus?.();
    };
  }, [initialFocus, open]);

  if (!open) return null;

  const widthClass = {
    sm: "w-full max-w-md",
    md: "w-full max-w-[42rem]",
    lg: "w-full max-w-[56rem]",
    xl: "w-full max-w-[64rem]",
  }[size];

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-3 sm:p-5">
      <button
        type="button"
        aria-label="Fechar"
        className="absolute inset-0 bg-black/70 backdrop-blur-[2px]"
        onClick={onClose}
      />
      <div
        ref={dialogRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        aria-describedby={description ? descriptionId : undefined}
        tabIndex={-1}
        className={cn(
          "relative max-h-[calc(100dvh-1.5rem)] overflow-y-auto overscroll-contain rounded-2xl border border-zinc-800 bg-zinc-950/95 p-0 shadow-[0_24px_80px_-20px_rgba(0,0,0,0.85)] outline-none backdrop-blur-xl",
          widthClass,
          className,
        )}
      >
        <div className="sticky top-0 z-10 flex items-start justify-between gap-4 border-b border-zinc-800/70 bg-zinc-950/95 px-6 py-4 backdrop-blur">
          <div className="min-w-0">
            <h2 id={titleId} className="text-lg font-semibold tracking-tight text-zinc-100">
              {title}
            </h2>
            {description ? (
              <p id={descriptionId} className="mt-1 max-w-prose text-[14px] leading-relaxed text-zinc-400">
                {description}
              </p>
            ) : null}
          </div>
          <button
            type="button"
            aria-label="Fechar"
            onClick={onClose}
            className="rounded-md p-1.5 text-zinc-500 transition-colors hover:bg-zinc-800/70 hover:text-zinc-200"
          >
            <X size={18} />
          </button>
        </div>
        <div className="px-6 pb-6">{children}</div>
      </div>
    </div>
  );
}

import { useEffect, useRef } from "react";
import { useSearchParams } from "react-router-dom";

/**
 * Reads ?<name>=<id>, scrolls to [data-focus-id="<id>"], highlights it and
 * removes the parameter so the highlight happens only once. The highlight
 * itself survives the parameter cleanup and fades after a couple of seconds.
 */
export function useFocusParam(name: string): string | null {
  const [params, setParams] = useSearchParams();
  const value = params.get(name);
  const highlighted = useRef<HTMLElement | null>(null);
  const removeTimer = useRef<number | null>(null);

  useEffect(() => {
    if (!value) return;
    const timer = window.setTimeout(() => {
      const escaped = value.replace(/"/g, '\\"');
      const element = document.querySelector<HTMLElement>(
        `[data-focus-id="${escaped}"]`,
      );
      if (!element) return;
      highlighted.current = element;
      element.scrollIntoView({ behavior: "smooth", block: "center" });
      element.classList.add("focus-pulse");
      removeTimer.current = window.setTimeout(
        () => element.classList.remove("focus-pulse"),
        2400,
      );
      setParams(
        (previous) => {
          const next = new URLSearchParams(previous);
          next.delete(name);
          return next;
        },
        { replace: true },
      );
    }, 180);
    return () => window.clearTimeout(timer);
  }, [name, setParams, value]);

  useEffect(
    () => () => {
      if (removeTimer.current) window.clearTimeout(removeTimer.current);
      highlighted.current?.classList.remove("focus-pulse");
    },
    [],
  );

  return value;
}

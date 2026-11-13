import { useCallback, useEffect, useRef, useState } from "react";

/**
 * Keeps an incremental list pinned to the newest entry. Auto-follow stops as
 * soon as the reader scrolls away from the bottom and resumes when they jump
 * back or get close enough.
 */
export function useStickToBottom<T extends HTMLElement>(revision: unknown) {
  const ref = useRef<T | null>(null);
  const stick = useRef(true);
  const [atBottom, setAtBottom] = useState(true);

  const scrollToBottom = useCallback((behavior: ScrollBehavior = "smooth") => {
    const element = ref.current;
    if (!element) return;
    stick.current = true;
    setAtBottom(true);
    if (behavior === "smooth" && typeof element.scrollTo === "function") {
      element.scrollTo({ top: element.scrollHeight, behavior });
    } else {
      element.scrollTop = element.scrollHeight;
    }
  }, []);

  const onScroll = useCallback(() => {
    const element = ref.current;
    if (!element) return;
    const distance =
      element.scrollHeight - element.scrollTop - element.clientHeight;
    const near = distance < 80;
    stick.current = near;
    setAtBottom(near);
  }, []);

  useEffect(() => {
    if (!stick.current) return;
    const element = ref.current;
    if (!element) return;
    element.scrollTop = element.scrollHeight;
    setAtBottom(true);
  }, [revision]);

  return { ref, atBottom, onScroll, scrollToBottom };
}

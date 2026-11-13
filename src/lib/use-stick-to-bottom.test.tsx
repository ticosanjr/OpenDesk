import { act, renderHook } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { useStickToBottom } from "./use-stick-to-bottom";

function element() {
  const node = document.createElement("div");
  Object.defineProperties(node, {
    scrollHeight: { configurable: true, value: 1000 },
    clientHeight: { configurable: true, value: 200 },
    scrollTop: { configurable: true, writable: true, value: 800 },
  });
  return node;
}

describe("useStickToBottom", () => {
  it("follows new entries and stops when the user scrolls away", () => {
    const { result } = renderHook(() => useStickToBottom<HTMLDivElement>("a"));
    const node = element();
    result.current.ref.current = node;

    act(() => result.current.onScroll());
    expect(result.current.atBottom).toBe(true);

    node.scrollTop = 100;
    act(() => result.current.onScroll());
    expect(result.current.atBottom).toBe(false);

    act(() => result.current.scrollToBottom("auto"));
    expect(result.current.atBottom).toBe(true);
    expect(node.scrollTop).toBe(1000);
  });

  it("pins to the bottom when the revision changes while following", () => {
    const { result, rerender } = renderHook(
      ({ revision }) => useStickToBottom<HTMLDivElement>(revision),
      { initialProps: { revision: "a" } },
    );
    const node = element();
    node.scrollTop = 0;
    result.current.ref.current = node;

    rerender({ revision: "b" });
    expect(node.scrollTop).toBe(1000);
    expect(result.current.atBottom).toBe(true);
  });
});

import { act, renderHook } from "@testing-library/react";
import type { ReactNode } from "react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useFocusParam } from "./use-focus-param";

describe("useFocusParam", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    const target = document.createElement("div");
    target.setAttribute("data-focus-id", "target");
    target.scrollIntoView = vi.fn();
    document.body.appendChild(target);
  });

  afterEach(() => {
    vi.useRealTimers();
    document.body.innerHTML = "";
  });

  it("scrolls, highlights and clears the focus parameter", () => {
    const wrapper = ({ children }: { children: ReactNode }) => (
      <MemoryRouter initialEntries={["/?focus=target"]}>{children}</MemoryRouter>
    );
    const { result } = renderHook(() => useFocusParam("focus"), { wrapper });
    expect(result.current).toBe("target");

    act(() => {
      vi.advanceTimersByTime(200);
    });

    const target = document.querySelector<HTMLElement>(
      '[data-focus-id="target"]',
    );
    expect(target?.scrollIntoView).toHaveBeenCalled();
    expect(target?.classList.contains("focus-pulse")).toBe(true);

    act(() => {
      vi.advanceTimersByTime(2500);
    });
    expect(target?.classList.contains("focus-pulse")).toBe(false);
  });

  it("does nothing when the parameter is absent", () => {
    const wrapper = ({ children }: { children: ReactNode }) => (
      <MemoryRouter>{children}</MemoryRouter>
    );
    const { result } = renderHook(() => useFocusParam("focus"), { wrapper });
    expect(result.current).toBeNull();
  });
});

import { act, renderHook } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { useDismissable } from "./use-dismissable";

describe("useDismissable", () => {
  it("closes on outside pointer and Escape, but not inside", () => {
    const element = document.createElement("div");
    document.body.appendChild(element);
    const ref = { current: element };
    const onClose = vi.fn();
    renderHook(() => useDismissable(ref, true, onClose));

    const inside = document.createElement("button");
    element.appendChild(inside);
    act(() => {
      inside.dispatchEvent(new Event("pointerdown", { bubbles: true }));
    });
    expect(onClose).not.toHaveBeenCalled();

    act(() => {
      document.body.dispatchEvent(new Event("pointerdown", { bubbles: true }));
    });
    expect(onClose).toHaveBeenCalledTimes(1);

    act(() => {
      document.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
    });
    expect(onClose).toHaveBeenCalledTimes(2);

    act(() => {
      document.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter" }));
    });
    expect(onClose).toHaveBeenCalledTimes(2);
    document.body.removeChild(element);
  });

  it("does nothing while closed and handles a null ref", () => {
    const onClose = vi.fn();
    const ref = { current: null };
    const { rerender } = renderHook(
      ({ open }) => useDismissable(ref, open, onClose),
      { initialProps: { open: false } },
    );
    act(() => {
      document.body.dispatchEvent(new Event("pointerdown", { bubbles: true }));
    });
    expect(onClose).not.toHaveBeenCalled();

    rerender({ open: true });
    act(() => {
      document.body.dispatchEvent(new Event("pointerdown", { bubbles: true }));
    });
    expect(onClose).toHaveBeenCalledTimes(1);
  });
});

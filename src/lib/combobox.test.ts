import { describe, expect, it } from "vitest";
import type { FocusEvent } from "react";
import { shouldCloseOnBlur } from "./combobox";

function focusEvent(current: HTMLElement, related: Node | null): FocusEvent<HTMLElement> {
  return {
    currentTarget: current,
    relatedTarget: related,
  } as unknown as FocusEvent<HTMLElement>;
}

describe("shouldCloseOnBlur", () => {
  it("keeps the menu open when focus moves inside the combobox", () => {
    const wrapper = document.createElement("div");
    const input = document.createElement("input");
    const option = document.createElement("button");
    wrapper.append(input, option);
    document.body.append(wrapper);

    expect(shouldCloseOnBlur(focusEvent(input, option))).toBe(false);
    wrapper.remove();
  });

  it("closes when focus leaves the combobox or is lost", () => {
    const wrapper = document.createElement("div");
    const input = document.createElement("input");
    const outside = document.createElement("button");
    wrapper.append(input);
    document.body.append(wrapper, outside);

    expect(shouldCloseOnBlur(focusEvent(input, outside))).toBe(true);
    expect(shouldCloseOnBlur(focusEvent(input, null))).toBe(true);
    wrapper.remove();
    outside.remove();
  });
});

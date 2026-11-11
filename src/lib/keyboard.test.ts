import { describe, expect, it } from "vitest";
import { isEditableTarget } from "./keyboard";

describe("isEditableTarget", () => {
  it("detects text inputs, textareas and selects", () => {
    expect(isEditableTarget(document.createElement("input"))).toBe(true);
    expect(isEditableTarget(document.createElement("textarea"))).toBe(true);
    expect(isEditableTarget(document.createElement("select"))).toBe(true);
  });

  it("ignores regular elements and null", () => {
    expect(isEditableTarget(document.createElement("button"))).toBe(false);
    expect(isEditableTarget(document.createElement("div"))).toBe(false);
    expect(isEditableTarget(null)).toBe(false);
  });

  it("detects contenteditable hosts", () => {
    const element = document.createElement("div");
    element.contentEditable = "true";
    Object.defineProperty(element, "isContentEditable", {
      configurable: true,
      value: true,
    });
    expect(isEditableTarget(element)).toBe(true);
  });
});

import { describe, expect, it } from "vitest";
import {
  isRouterSelection,
  modelSelectionLabel,
  toModelSelection,
} from "./model-selection";

describe("model selection", () => {
  it("normalizes empty values to the router", () => {
    expect(toModelSelection(undefined)).toBeNull();
    expect(toModelSelection(null)).toBeNull();
    expect(toModelSelection("")).toBeNull();
    expect(toModelSelection("   ")).toBeNull();
  });

  it("keeps concrete model references and trims whitespace", () => {
    expect(toModelSelection(" opencode-go/glm-5.3 ")).toBe(
      "opencode-go/glm-5.3",
    );
  });

  it("identifies router selections and labels them explicitly", () => {
    expect(isRouterSelection(null)).toBe(true);
    expect(isRouterSelection("")).toBe(true);
    expect(isRouterSelection("opencode-go/glm-5.3")).toBe(false);
    expect(modelSelectionLabel(null)).toBe("Roteador automático");
    expect(modelSelectionLabel("opencode-go/glm-5.3")).toBe(
      "opencode-go/glm-5.3",
    );
  });
});

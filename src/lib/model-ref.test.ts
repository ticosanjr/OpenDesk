import { describe, expect, it } from "vitest";
import { isValidModelRef, normalizeModelRef } from "./model-ref";

describe("model refs", () => {
  it("normalizes plain and variant refs", () => {
    expect(normalizeModelRef(" opencode-go/glm-5.3 ")).toBe(
      "opencode-go/glm-5.3",
    );
    expect(normalizeModelRef("opencode-go/glm-5.3# high ")).toBe(
      "opencode-go/glm-5.3#high",
    );
    expect(normalizeModelRef("opencode-go/glm-5.3#  ")).toBe(
      "opencode-go/glm-5.3",
    );
  });

  it("rejects malformed refs", () => {
    expect(normalizeModelRef("")).toBeNull();
    expect(normalizeModelRef("glm-5.3")).toBeNull();
    expect(normalizeModelRef("opencode-go/")).toBeNull();
    expect(normalizeModelRef("/glm-5.3")).toBeNull();
    expect(isValidModelRef("glm-5.3")).toBe(false);
    expect(isValidModelRef("")).toBe(true);
  });
});

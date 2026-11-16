import { describe, expect, it } from "vitest";
import { harnessLabel, HARNESS_OPTIONS } from "./harnesses";

describe("harness labels", () => {
  it("maps known ids to labels", () => {
    expect(harnessLabel("opencode")).toBe("OpenCode V2");
    expect(harnessLabel("claude-code")).toBe("Claude Code");
    expect(harnessLabel("codex")).toBe("Codex CLI");
  });

  it("falls back to the id or opencode", () => {
    expect(harnessLabel("custom")).toBe("custom");
    expect(harnessLabel(null)).toBe("opencode");
    expect(harnessLabel(undefined)).toBe("opencode");
  });

  it("exposes unique options", () => {
    const ids = new Set(HARNESS_OPTIONS.map((option) => option.id));
    expect(ids.size).toBe(HARNESS_OPTIONS.length);
  });
});

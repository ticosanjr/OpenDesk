import { describe, expect, it } from "vitest";
import { TRIGGER_TYPES, triggerDescription, triggerLabel } from "./trigger-types";

describe("trigger types", () => {
  it("returns friendly labels and descriptions", () => {
    expect(triggerLabel("FOLDER_WATCH")).toContain("Pasta monitorada");
    expect(triggerDescription("WEBHOOK")).toContain("HTTP");
  });

  it("falls back for unknown triggers", () => {
    expect(triggerLabel("MYSTERY")).toBe("MYSTERY");
    expect(triggerDescription("MYSTERY")).toBe("Gatilho de entrada.");
    expect(TRIGGER_TYPES.some((item) => item.value === "CRON")).toBe(true);
    expect(TRIGGER_TYPES.some((item) => item.value === "CALENDAR_EVENT")).toBe(true);
  });
});

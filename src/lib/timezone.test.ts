import { describe, expect, it } from "vitest";
import { formatOffset, timezoneOptions, timezoneOffsetMinutes } from "./timezone";

describe("timezone helpers", () => {
  it("formats signed offsets", () => {
    expect(formatOffset(0)).toBe("UTC+00:00");
    expect(formatOffset(-180)).toBe("UTC-03:00");
    expect(formatOffset(330)).toBe("UTC+05:30");
  });

  it("calculates common offsets and exposes a sorted list", () => {
    expect(timezoneOffsetMinutes("UTC")).toBe(0);
    expect(timezoneOffsetMinutes("America/Sao_Paulo")).toBe(-180);
    const options = timezoneOptions("UTC");
    expect(options[0].offsetMinutes).toBeLessThanOrEqual(options[options.length - 1].offsetMinutes);
    const utc = options.find((option) => option.value === "UTC");
    expect(utc?.label).toContain("UTC+00:00");
    expect(utc?.label).toContain("seu fuso");
  });

  it("catches invalid zones and accepts a custom current zone", () => {
    expect(timezoneOffsetMinutes("not/a-zone")).toBe(0);
    const options = timezoneOptions("Custom/Zone");
    expect(
      options.find((option) => option.value === "Custom/Zone")?.label,
    ).toContain("seu fuso");
    const withoutCurrent = timezoneOptions();
    expect(
      withoutCurrent.every((option) => !option.label.includes("seu fuso")),
    ).toBe(true);
  });
});

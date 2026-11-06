import { describe, expect, it } from "vitest";
import { formatNumber, formatPercent, formatUsd, relativeTime, shortPath } from "./format";

describe("format", () => {
  it("formats USD and falls back to zero for invalid values", () => {
    expect(formatUsd(1.5)).toContain("1,50");
    expect(formatUsd(null)).toContain("0,00");
    expect(formatUsd(Number.NaN)).toContain("0,00");
  });

  it("formats numbers and percentages", () => {
    expect(formatNumber(1234567)).toBe("1.234.567");
    expect(formatNumber(null)).toBe("0");
    expect(formatPercent(12.34)).toBe("12.3%");
    expect(formatPercent(null)).toBe("0.0%");
  });

  it("formats relative time defensively", () => {
    expect(relativeTime(null)).toBe("—");
    expect(relativeTime("not-a-date")).toBe("not-a-date");
    expect(relativeTime(new Date().toISOString())).toMatch(/agora|segundos|minuto/i);
  });

  it("shortens long paths while keeping the tail", () => {
    expect(shortPath("/tmp/short", 48)).toBe("/tmp/short");
    const shortened = shortPath("/home/user/a/very/long/path/to/an/artifact/file.md", 20);
    expect(shortened).toContain("…");
    expect(shortened.endsWith("file.md")).toBe(true);
    expect(shortened.length).toBeLessThanOrEqual(20);
  });
});

import { describe, expect, it, vi } from "vitest";
import {
  formatClock,
  formatDateTime,
  formatDuration,
  formatNumber,
  formatPercent,
  formatUsd,
  relativeTime,
  shortPath,
} from "./format";

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


describe("formatDuration", () => {
  it("formats milliseconds, seconds, minutes and hours", () => {
    expect(formatDuration(0)).toBe("0 ms");
    expect(formatDuration(840)).toBe("840 ms");
    expect(formatDuration(1250)).toBe("1,3 s");
    expect(formatDuration(63_000)).toBe("1m 03s");
    expect(formatDuration(3_723_000)).toBe("1h 02m");
  });
});

describe("formatClock and formatDateTime", () => {
  it("formats clock and date-time values defensively", () => {
    expect(formatClock(null)).toBe("—");
    expect(formatDateTime(undefined)).toBe("—");
    expect(formatClock("2026-11-08T12:34:56Z")).toMatch(/\d{2}:\d{2}:\d{2}/);
    expect(formatDateTime("2026-11-08T12:34:56Z")).toMatch(/2026/);
  });

  it("falls back when the locale formatter throws", () => {
    const clock = vi
      .spyOn(Date.prototype, "toLocaleTimeString")
      .mockImplementationOnce(() => {
        throw new Error("clock");
      });
    expect(formatClock("2026-11-08T12:34:56Z")).toBe("—");
    clock.mockRestore();

    const dateTime = vi
      .spyOn(Date.prototype, "toLocaleString")
      .mockImplementationOnce(() => {
        throw new Error("date");
      });
    expect(formatDateTime("2026-11-08T12:34:56Z")).toBe(
      "2026-11-08T12:34:56Z",
    );
    dateTime.mockRestore();
  });
});

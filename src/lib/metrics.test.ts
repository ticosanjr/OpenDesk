import { describe, expect, it } from "vitest";
import {
  barPercent,
  dailyMax,
  failureLabel,
  fillDailySeries,
  formatMs,
  successTone,
} from "./metrics";

describe("metrics helpers", () => {
  it("formats durations across scales", () => {
    expect(formatMs(0)).toBe("—");
    expect(formatMs(850)).toBe("850 ms");
    expect(formatMs(1200)).toBe("1.2 s");
    expect(formatMs(125_000)).toBe("2m 05s");
  });

  it("maps failure codes and tones", () => {
    expect(failureLabel("timeout")).toBe("timeout");
    expect(failureLabel("custom")).toBe("custom");
    expect(successTone(0.95)).toBe("green");
    expect(successTone(0.8)).toBe("yellow");
    expect(successTone(0.5)).toBe("red");
  });

  it("scales daily bars", () => {
    const daily = [
      { date: "2026-01-01", runs: 3, failed: 1, cost_usd: 0.1 },
      { date: "2026-01-02", runs: 5, failed: 0, cost_usd: 0.2 },
    ];
    expect(dailyMax(daily)).toBe(5);
    expect(barPercent(3, 5)).toBe(60);
    expect(barPercent(-1, 5)).toBe(0);
    expect(barPercent(10, 5)).toBe(100);
    expect(barPercent(1, 0)).toBe(0);
  });

  it("fills missing days so the chart keeps its period", () => {
    const series = fillDailySeries(
      [{ date: "2026-01-03", runs: 2, failed: 0, cost_usd: 0.2 }],
      3,
      new Date("2026-01-03T12:00:00Z"),
    );
    expect(series.map((day) => day.date)).toEqual([
      "2026-01-01",
      "2026-01-02",
      "2026-01-03",
    ]);
    expect(series[2]?.runs).toBe(2);
    expect(series[0]?.runs).toBe(0);
  });
});

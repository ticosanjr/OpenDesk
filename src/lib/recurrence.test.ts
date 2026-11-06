import { describe, expect, it } from "vitest";
import { describeRecurrence, parseRecurrence } from "./recurrence";
import type { Recurrence } from "./types";

function recurrence(part: Partial<Recurrence>): Recurrence {
  return {
    frequency: "daily",
    interval: 1,
    time: "08:00",
    weekdays: [],
    day_of_month: null,
    start_date: null,
    end_date: null,
    cron_expression: null,
    ...part,
  };
}

describe("parseRecurrence", () => {
  it("parses stored recurrence json", () => {
    const value = parseRecurrence(
      '{"frequency":"weekly","interval":2,"time":"09:30","weekdays":[1,3]}',
    );
    expect(value).toMatchObject({
      frequency: "weekly",
      interval: 2,
      time: "09:30",
      weekdays: [1, 3],
    });
  });

  it("rejects missing or malformed values", () => {
    expect(parseRecurrence(null)).toBeNull();
    expect(parseRecurrence("not json")).toBeNull();
    expect(parseRecurrence('{"frequency":"never"}')).toBeNull();
  });
});

describe("describeRecurrence", () => {
  it("describes common recurrences in portuguese", () => {
    expect(describeRecurrence(recurrence({}), "0 8 * * *")).toBe(
      "diariamente às 08:00",
    );
    expect(
      describeRecurrence(recurrence({ frequency: "weekdays" }), "0 8 * * 1-5"),
    ).toBe("dias úteis às 08:00");
    expect(
      describeRecurrence(
        recurrence({ frequency: "weekly", weekdays: [1, 3], time: "09:30" }),
        "30 9 * * 1,3",
      ),
    ).toBe("seg, qua às 09:30");
    expect(
      describeRecurrence(recurrence({ frequency: "hourly", interval: 6 }), "0 */6 * * *"),
    ).toBe("a cada 6 horas");
    expect(
      describeRecurrence(recurrence({ frequency: "monthly", day_of_month: 5 }), "0 8 5 * *"),
    ).toBe("todo dia 5 do mês às 08:00");
  });

  it("describes once and end dates", () => {
    expect(
      describeRecurrence(
        recurrence({
          frequency: "once",
          start_date: "2026-12-24",
          time: "09:30",
        }),
        "30 9 24 12 *",
      ),
    ).toBe("uma vez em 24/12/2026 às 09:30");

    expect(
      describeRecurrence(
        recurrence({ end_date: "2026-11-30" }),
        "0 8 * * *",
      ),
    ).toBe("diariamente às 08:00 até 30/11/2026");
  });

  it("falls back to cron when recurrence is unknown", () => {
    expect(describeRecurrence(null, "0 8 * * 1")).toBe("0 8 * * 1");
  });
});

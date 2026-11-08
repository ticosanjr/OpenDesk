import { describe, expect, it } from "vitest";
import {
  calendarRange,
  categoryOf,
  isVisibleItem,
  itemTimeLabel,
  itemsForDay,
  monthGrid,
} from "./calendar";
import type { CalendarItem } from "./types";

function item(partial: Partial<CalendarItem>): CalendarItem {
  return {
    id: "item",
    kind: "event",
    title: "Evento",
    start: "2026-11-08T12:00:00Z",
    all_day: false,
    status: "FUTURE",
    source: "LOCAL",
    editable: true,
    ...partial,
  };
}

describe("calendar helpers", () => {
  it("builds a six-week grid around the month", () => {
    const days = monthGrid(new Date(2026, 10, 1));
    expect(days).toHaveLength(42);
    expect(days[0].getDay()).toBe(0);
    expect(days[41].getDay()).toBe(6);
  });

  it("returns the visible range including adjacent weeks", () => {
    const range = calendarRange(new Date(2026, 10, 1));
    expect(new Date(range.from).getTime()).toBeLessThanOrEqual(
      new Date(2026, 10, 1).getTime(),
    );
    expect(new Date(range.to).getTime()).toBeGreaterThan(
      new Date(2026, 10, 30).getTime(),
    );
  });

  it("groups items by local day and sorts by time", () => {
    const day = new Date(2026, 10, 8);
    const later = item({
      id: "later",
      start: new Date(2026, 10, 8, 15, 0).toISOString(),
    });
    const earlier = item({
      id: "earlier",
      start: new Date(2026, 10, 8, 9, 0).toISOString(),
    });
    const other = item({
      id: "other",
      start: new Date(2026, 10, 9, 9, 0).toISOString(),
    });
    expect(itemsForDay([later, other, earlier], day).map((entry) => entry.id)).toEqual([
      "earlier",
      "later",
    ]);
  });

  it("maps kinds to categories and respects filters", () => {
    expect(categoryOf("routine")).toBe("routines");
    expect(categoryOf("recipe")).toBe("recipes");
    expect(categoryOf("run")).toBe("executions");
    expect(categoryOf("recipe_run")).toBe("executions");
    expect(categoryOf("queued")).toBe("executions");
    expect(categoryOf("event")).toBe("events");
    const filters = {
      routines: false,
      recipes: true,
      executions: true,
      events: true,
    };
    expect(isVisibleItem(item({ kind: "routine" }), filters)).toBe(false);
    expect(isVisibleItem(item({ kind: "recipe" }), filters)).toBe(true);
  });

  it("formats timed and all-day labels", () => {
    const timed = item({
      start: new Date(2026, 10, 8, 9, 0).toISOString(),
      end: new Date(2026, 10, 8, 10, 30).toISOString(),
    });
    expect(itemTimeLabel(timed)).toBe("09:00–10:30");
    expect(itemTimeLabel(item({ all_day: true }))).toBe("dia inteiro");
    expect(
      itemTimeLabel(
        item({
          start: new Date(2026, 10, 8, 9, 0).toISOString(),
          end: null,
        }),
      ),
    ).toBe("09:00");
  });
});

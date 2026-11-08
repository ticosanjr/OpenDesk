import {
  endOfMonth,
  endOfWeek,
  format,
  isSameDay,
  startOfMonth,
  startOfWeek,
} from "date-fns";
import type { CalendarItem } from "@/lib/types";

export type CalendarCategory =
  | "routines"
  | "recipes"
  | "executions"
  | "events";

export const CALENDAR_CATEGORIES: Array<{
  id: CalendarCategory;
  label: string;
}> = [
  { id: "routines", label: "Rotinas" },
  { id: "recipes", label: "Receitas" },
  { id: "executions", label: "Execuções" },
  { id: "events", label: "Eventos" },
];

/** Six full weeks so the month grid never jumps between layouts. */
export function monthGrid(month: Date): Date[] {
  const start = startOfWeek(startOfMonth(month), { weekStartsOn: 0 });
  const days: Date[] = [];
  for (let index = 0; index < 42; index += 1) {
    days.push(
      new Date(start.getFullYear(), start.getMonth(), start.getDate() + index),
    );
  }
  return days;
}

export function calendarRange(month: Date): { from: string; to: string } {
  return {
    from: startOfWeek(startOfMonth(month), { weekStartsOn: 0 }).toISOString(),
    to: endOfWeek(endOfMonth(month), { weekStartsOn: 0 }).toISOString(),
  };
}

export function categoryOf(kind: string): CalendarCategory {
  if (kind === "routine") return "routines";
  if (kind === "recipe") return "recipes";
  if (kind === "event") return "events";
  return "executions";
}

export function itemsForDay(
  items: CalendarItem[],
  day: Date,
): CalendarItem[] {
  const dayKey = format(day, "yyyy-MM-dd");
  return items
    .filter((item) =>
      item.all_day
        ? item.start.slice(0, 10) === dayKey
        : isSameDay(new Date(item.start), day),
    )
    .sort((left, right) => left.start.localeCompare(right.start));
}

export function isVisibleItem(
  item: CalendarItem,
  filters: Record<CalendarCategory, boolean>,
): boolean {
  return filters[categoryOf(item.kind)];
}

export function itemTimeLabel(item: CalendarItem): string {
  if (item.all_day) return "dia inteiro";
  const start = format(new Date(item.start), "HH:mm");
  if (!item.end) return start;
  return `${start}–${format(new Date(item.end), "HH:mm")}`;
}

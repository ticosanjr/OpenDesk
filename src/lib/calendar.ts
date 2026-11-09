import {
  addDays,
  endOfDay,
  endOfMonth,
  endOfWeek,
  format,
  isSameDay,
  startOfDay,
  startOfMonth,
  startOfWeek,
} from "date-fns";
import type { CalendarItem } from "@/lib/types";

export type CalendarView = "month" | "week" | "seven" | "day";

export const CALENDAR_VIEWS: Array<{ id: CalendarView; label: string }> = [
  { id: "month", label: "Mês" },
  { id: "week", label: "Semana" },
  { id: "seven", label: "7 dias" },
  { id: "day", label: "Dia" },
];

function viewKey(workspaceId?: string | null): string {
  return `opendesk.calendar.view.${workspaceId ?? "global"}`;
}

export function loadCalendarView(workspaceId?: string | null): CalendarView {
  try {
    const value = localStorage.getItem(viewKey(workspaceId));
    return CALENDAR_VIEWS.some((view) => view.id === value)
      ? (value as CalendarView)
      : "month";
  } catch {
    return "month";
  }
}

export function saveCalendarView(
  workspaceId: string | null | undefined,
  view: CalendarView,
): void {
  try {
    localStorage.setItem(viewKey(workspaceId), view);
  } catch {
    // Storage can be unavailable; the in-memory view still works.
  }
}

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

export function weekDays(day: Date): Date[] {
  const start = startOfWeek(day, { weekStartsOn: 0 });
  return Array.from({ length: 7 }, (_, index) => addDays(start, index));
}

export function rollingDays(day: Date, count = 7): Date[] {
  const start = startOfDay(day);
  return Array.from({ length: count }, (_, index) => addDays(start, index));
}

export function viewDays(
  view: CalendarView,
  month: Date,
  selectedDay: Date,
): Date[] {
  if (view === "month") return monthGrid(month);
  if (view === "week") return weekDays(selectedDay);
  if (view === "seven") return rollingDays(selectedDay);
  return [startOfDay(selectedDay)];
}

export function viewRange(
  view: CalendarView,
  month: Date,
  selectedDay: Date,
): { from: string; to: string } {
  if (view === "month") return calendarRange(month);
  const days = viewDays(view, month, selectedDay);
  return {
    from: startOfDay(days[0]).toISOString(),
    to: endOfDay(days[days.length - 1]).toISOString(),
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

import { format, isSameMonth, isToday, startOfDay } from "date-fns";
import { ptBR } from "date-fns/locale";
import { CalendarItemChip } from "@/components/calendar/calendar-item";
import type { CalendarItem } from "@/lib/types";
import { itemsForDay } from "@/lib/calendar";
import { cn } from "@/lib/utils";
import { capitalized } from "./calendar-utils";

const WEEKDAYS = ["dom", "seg", "ter", "qua", "qui", "sex", "sáb"];

export function MonthGridView({
  days,
  month,
  selectedDay,
  items,
  onSelectDay,
  onOpenItem,
}: {
  days: Date[];
  month: Date;
  selectedDay: Date;
  items: CalendarItem[];
  onSelectDay: (day: Date) => void;
  onOpenItem: (item: CalendarItem) => void;
}) {
  return (
    <div className="grid grid-cols-7 gap-1">
      {WEEKDAYS.map((weekday) => (
        <div
          key={weekday}
          className="px-1 py-1 text-center text-[11.5px] uppercase tracking-[0.12em] text-zinc-500"
        >
          {weekday}
        </div>
      ))}
      {days.map((day) => {
        const dayItems = itemsForDay(items, day);
        const selected = day.toDateString() === selectedDay.toDateString();
        return (
          <div
            key={day.toISOString()}
            role="button"
            tabIndex={0}
            aria-label={capitalized(
              format(day, "EEEE, d 'de' MMMM", { locale: ptBR }),
            )}
            onClick={() => onSelectDay(startOfDay(day))}
            onKeyDown={(event) => {
              if (event.key === "Enter" || event.key === " ") {
                onSelectDay(startOfDay(day));
              }
            }}
            className={cn(
              "min-h-[104px] rounded-lg border p-1.5 text-left transition-colors",
              selected
                ? "workspace-selected"
                : "border-zinc-800/70 bg-zinc-950/30 hover:border-zinc-700",
              isSameMonth(day, month)
                ? undefined
                : "border-zinc-800/40 bg-zinc-950/10",
            )}
          >
            <div className="flex items-center justify-between">
              <span
                className={cn(
                  "flex h-5 w-5 items-center justify-center rounded-full text-[12px]",
                  isToday(day)
                    ? "bg-blue-600 font-semibold text-white"
                    : "text-zinc-400",
                )}
              >
                {format(day, "d")}
              </span>
              {dayItems.length > 0 ? (
                <span className="font-mono text-[11px] text-zinc-500">
                  {dayItems.length}
                </span>
              ) : null}
            </div>
            <div className="mt-1 space-y-0.5">
              {dayItems.slice(0, 3).map((item) => (
                <CalendarItemChip
                  key={item.id}
                  item={item}
                  onOpen={onOpenItem}
                />
              ))}
              {dayItems.length > 3 ? (
                <p className="px-1 text-[11px] text-zinc-500">
                  +{dayItems.length - 3} mais
                </p>
              ) : null}
            </div>
          </div>
        );
      })}
    </div>
  );
}

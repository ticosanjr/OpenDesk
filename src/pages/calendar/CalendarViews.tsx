import { format, isToday, startOfDay } from "date-fns";
import { ptBR } from "date-fns/locale";
import { CalendarDays } from "lucide-react";
import {
  CalendarItemChip,
  CalendarItemRow,
} from "@/components/calendar/calendar-item";
import { EmptyState } from "@/components/ui/empty-state";
import { itemsForDay } from "@/lib/calendar";
import type { CalendarItem } from "@/lib/types";
import { cn } from "@/lib/utils";
import { capitalized } from "./calendar-utils";

export function CalendarColumns({
  days,
  items,
  selectedDay,
  onSelectDay,
  onOpenItem,
}: {
  days: Date[];
  items: CalendarItem[];
  selectedDay: Date;
  onSelectDay: (day: Date) => void;
  onOpenItem: (item: CalendarItem) => void;
}) {
  return (
    <div className="grid grid-cols-2 gap-2 md:grid-cols-4 xl:grid-cols-7">
      {days.map((day) => {
        const dayItems = itemsForDay(items, day);
        const selected = day.toDateString() === selectedDay.toDateString();
        return (
          <div
            key={day.toISOString()}
            onClick={() => onSelectDay(startOfDay(day))}
            className={cn(
              "min-h-[320px] rounded-lg border p-2 transition-colors",
              selected
                ? "workspace-selected"
                : "border-zinc-800/70 bg-zinc-950/30",
            )}
          >
            <div className="flex items-center justify-between border-b border-zinc-800/70 pb-1.5">
              <div>
                <p className="text-[11.5px] uppercase tracking-[0.12em] text-zinc-500">
                  {format(day, "EEE", { locale: ptBR })}
                </p>
                <p
                  className={cn(
                    "text-[14px] font-medium",
                    isToday(day) ? "text-blue-300" : "text-zinc-300",
                  )}
                >
                  {format(day, "d MMM", { locale: ptBR })}
                </p>
              </div>
              {dayItems.length > 0 ? (
                <span className="font-mono text-[11px] text-zinc-500">
                  {dayItems.length}
                </span>
              ) : null}
            </div>
            <div className="mt-1.5 space-y-1">
              {dayItems.length === 0 ? (
                <p className="px-1 py-2 text-[11.5px] text-zinc-500">—</p>
              ) : (
                dayItems.map((item) => (
                  <CalendarItemChip
                    key={item.id}
                    item={item}
                    onOpen={onOpenItem}
                    detailed
                  />
                ))
              )}
            </div>
          </div>
        );
      })}
    </div>
  );
}

export function CalendarDayView({
  items,
  day,
  onOpenItem,
}: {
  items: CalendarItem[];
  day: Date;
  onOpenItem: (item: CalendarItem) => void;
}) {
  return (
    <div className="space-y-2">
      <p className="text-[13px] font-medium text-zinc-300">
        {capitalized(format(day, "EEEE, d 'de' MMMM 'de' yyyy", { locale: ptBR }))}
      </p>
      {items.length === 0 ? (
        <EmptyState
          icon={CalendarDays}
          title="Nada neste dia"
          description="Crie um evento ou navegue para outro dia."
          className="border-0 bg-transparent"
        />
      ) : (
        items.map((item) => (
          <CalendarItemRow
            key={item.id}
            item={item}
            onOpen={onOpenItem}
            size="md"
          />
        ))
      )}
    </div>
  );
}

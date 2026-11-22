import { format } from "date-fns";
import { ptBR } from "date-fns/locale";
import { CalendarDays } from "lucide-react";
import { CalendarItemRow } from "@/components/calendar/calendar-item";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { EmptyState } from "@/components/ui/empty-state";
import type { CalendarItem } from "@/lib/types";
import { capitalized } from "./calendar-utils";

export function SelectedDayCard({
  selectedDay,
  selectedItems,
  onOpenItem,
}: {
  selectedDay: Date;
  selectedItems: CalendarItem[];
  onOpenItem: (item: CalendarItem) => void;
}) {
  return (
    <Card className="min-h-0">
      <CardHeader className="py-3">
        <CardTitle className="text-[13.5px]">
          {capitalized(format(selectedDay, "EEEE, d 'de' MMMM", { locale: ptBR }))}
        </CardTitle>
        <Badge variant="default">{selectedItems.length}</Badge>
      </CardHeader>
      <CardContent className="max-h-[70dvh] space-y-1.5 overflow-y-auto">
        {selectedItems.length === 0 ? (
          <EmptyState
            icon={CalendarDays}
            title="Nada neste dia"
            description="Crie um evento ou navegue para outro dia."
            className="border-0 bg-transparent"
          />
        ) : (
          selectedItems.map((item) => (
            <CalendarItemRow
              key={item.id}
              item={item}
              onOpen={onOpenItem}
            />
          ))
        )}
      </CardContent>
    </Card>
  );
}

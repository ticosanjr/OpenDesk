import { useCallback, useEffect, useMemo, useState } from "react";
import { useNavigate, useSearchParams } from "react-router-dom";
import {
  addDays,
  addMonths,
  format,
  startOfDay,
  startOfMonth,
} from "date-fns";
import { ptBR } from "date-fns/locale";
import {
  CalendarDays,
  ChevronLeft,
  ChevronRight,
  CloudDownload,
  Plus,
  RefreshCw,
} from "lucide-react";
import { PageBody, PageHeader } from "@/components/layout/PageHeader";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { IconLabel } from "@/components/ui/icon-label";
import { EventModal } from "@/components/calendar/event-modal";
import { ImportGoogleModal } from "@/components/calendar/import-google-modal";
import { apiGet, apiPost } from "@/lib/api";
import {
  isVisibleItem,
  itemsForDay,
  loadCalendarView,
  saveCalendarView,
  viewDays,
  viewRange,
  type CalendarCategory,
  type CalendarView,
} from "@/lib/calendar";
import { useAppStore } from "@/store/app-store";
import { toast } from "@/store/toast-store";
import type {
  CalendarAgenda,
  CalendarEvent,
  CalendarItem,
  CalendarSource,
} from "@/lib/types";
import { cn } from "@/lib/utils";
import { capitalized } from "./calendar/calendar-utils";
import { CalendarToolbar } from "./calendar/CalendarToolbar";
import { MonthGridView } from "./calendar/MonthGridView";
import {
  CalendarColumns,
  CalendarDayView,
} from "./calendar/CalendarViews";
import { SelectedDayCard } from "./calendar/SelectedDayCard";

export function CalendarPage() {
  const activeWorkspaceId = useAppStore((state) => state.activeWorkspaceId);
  const navigate = useNavigate();
  const [params, setParams] = useSearchParams();
  const focusDate = params.get("date");
  const focusEvent = params.get("event");
  const initialDate = (() => {
    if (!focusDate) return new Date();
    const value = new Date(focusDate);
    return Number.isNaN(value.getTime()) ? new Date() : value;
  })();
  const [view, setView] = useState<CalendarView>(() => loadCalendarView(activeWorkspaceId));
  const [month, setMonth] = useState(() => startOfMonth(initialDate));
  const [selectedDay, setSelectedDay] = useState(() => startOfDay(initialDate));
  const [items, setItems] = useState<CalendarItem[]>([]);
  const [events, setEvents] = useState<CalendarEvent[]>([]);
  const [sources, setSources] = useState<CalendarSource[]>([]);
  const [loading, setLoading] = useState(false);
  const [syncing, setSyncing] = useState(false);
  const [importOpen, setImportOpen] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [editing, setEditing] = useState<CalendarEvent | "new" | null>(null);
  const [filters, setFilters] = useState<Record<CalendarCategory, boolean>>({
    routines: true,
    recipes: true,
    executions: true,
    events: true,
  });

  useEffect(() => {
    setView(loadCalendarView(activeWorkspaceId));
  }, [activeWorkspaceId]);

  useEffect(() => {
    saveCalendarView(activeWorkspaceId, view);
  }, [activeWorkspaceId, view]);

  const days = useMemo(
    () => viewDays(view, month, selectedDay),
    [view, month, selectedDay],
  );
  const range = useMemo(
    () => viewRange(view, month, selectedDay),
    [view, month, selectedDay],
  );
  const title = useMemo(
    () =>
      view === "month"
        ? capitalized(format(month, "MMMM 'de' yyyy", { locale: ptBR }))
        : `${format(days[0], "d MMM", { locale: ptBR })} – ${format(
            days[days.length - 1],
            "d 'de' MMM yyyy",
            { locale: ptBR },
          )}`,
    [days, month, view],
  );
  const visibleItems = useMemo(
    () => items.filter((item) => isVisibleItem(item, filters)),
    [items, filters],
  );
  const selectedItems = useMemo(
    () => itemsForDay(visibleItems, selectedDay),
    [visibleItems, selectedDay],
  );
  const eventMap = useMemo(
    () => new Map(events.map((event) => [event.id, event])),
    [events],
  );

  const load = useCallback(async () => {
    if (!activeWorkspaceId) return;
    setLoading(true);
    try {
      const query = `workspace_id=${encodeURIComponent(activeWorkspaceId)}&from=${encodeURIComponent(
        range.from,
      )}&to=${encodeURIComponent(range.to)}`;
      const [agenda, eventList, sourceList] = await Promise.all([
        apiGet<CalendarAgenda>(`/api/calendar/agenda?${query}`),
        apiGet<CalendarEvent[]>(`/api/calendar/events?${query}`),
        apiGet<CalendarSource[]>(
          `/api/calendar/sources?workspace_id=${encodeURIComponent(activeWorkspaceId)}`,
        ),
      ]);
      setItems(Array.isArray(agenda.items) ? agenda.items : []);
      setEvents(Array.isArray(eventList) ? eventList : []);
      setSources(Array.isArray(sourceList) ? sourceList : []);
      setError(null);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setLoading(false);
    }
  }, [activeWorkspaceId, range.from, range.to]);

  useEffect(() => {
    void load();
  }, [load]);

  useEffect(() => {
    if (!focusEvent) return;
    let cancelled = false;
    void apiGet<CalendarEvent>(
      `/api/calendar/events/${encodeURIComponent(focusEvent)}`,
    )
      .then((event) => {
        if (cancelled) return;
        const start = new Date(event.start_at);
        setMonth(startOfMonth(start));
        setSelectedDay(startOfDay(start));
        setEditing(event);
      })
      .catch(() => {
        // A stale link should not break the calendar.
      });
    return () => {
      cancelled = true;
    };
  }, [focusEvent]);

  async function syncCalendars() {
    setSyncing(true);
    try {
      const result = await apiPost<{
        reports: Array<{ source_name: string; imported: number; error?: string }>;
      }>("/api/calendar/sync");
      const errors = result.reports.filter((report) => report.error);
      const imported = result.reports.reduce(
        (total, report) => total + report.imported,
        0,
      );
      toast({
        tone: errors.length > 0 ? "error" : "success",
        title:
          errors.length > 0
            ? "Sincronização parcial"
            : "Calendários sincronizados",
        description:
          errors.length > 0
            ? errors.map((report) => `${report.source_name}: ${report.error}`).join(" · ")
            : `${imported} evento(s) atualizados.`,
      });
      await load();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setSyncing(false);
    }
  }

  function move(direction: -1 | 1) {
    if (view === "month") {
      const next = addMonths(month, direction);
      setMonth(next);
      setSelectedDay(startOfMonth(next));
      return;
    }
    const step = view === "week" || view === "seven" ? 7 : 1;
    const next = addDays(selectedDay, direction * step);
    setSelectedDay(startOfDay(next));
    setMonth(startOfMonth(next));
  }

  function goToday() {
    const today = new Date();
    setMonth(startOfMonth(today));
    setSelectedDay(startOfDay(today));
  }

  function openItem(item: CalendarItem) {
    setSelectedDay(startOfDay(new Date(item.start)));
    if (item.kind === "event") {
      const event = eventMap.get(item.id);
      if (event) setEditing(event);
      return;
    }
    if (item.href) navigate(item.href);
  }

  return (
    <PageBody>
      <PageHeader
        title="Calendário"
        description="Veja rotinas, receitas e execuções passadas, presentes e futuras, além dos seus próprios eventos."
        actions={
          <>
            <Button variant="outline" size="sm" onClick={() => void load()} disabled={loading}>
              <IconLabel
                icon={<RefreshCw size={13} className={loading ? "animate-spin" : undefined} />}
              >
                Atualizar
              </IconLabel>
            </Button>
            <Button
              variant="outline"
              size="sm"
              onClick={() => setImportOpen(true)}
            >
              <IconLabel icon={<CloudDownload size={13} />}>Google</IconLabel>
            </Button>
            <Button
              variant="outline"
              size="sm"
              onClick={() => void syncCalendars()}
              disabled={syncing}
            >
              <IconLabel
                icon={<RefreshCw size={13} className={syncing ? "animate-spin" : undefined} />}
              >
                Sincronizar
              </IconLabel>
            </Button>
            <Button size="sm" onClick={() => setEditing("new")}>
              <IconLabel icon={<Plus size={13} />}>Novo evento</IconLabel>
            </Button>
          </>
        }
      />

      {error ? (
        <p className="rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[13px] text-red-300">
          {error}
        </p>
      ) : null}

      <CalendarToolbar
        view={view}
        filters={filters}
        visibleCount={visibleItems.length}
        onViewChange={setView}
        onToggleFilter={(id) =>
          setFilters((current) => ({
            ...current,
            [id]: !current[id],
          }))
        }
      />

      <div
        className={cn(
          "grid min-h-0 gap-3",
          view === "month" ? "xl:grid-cols-[minmax(0,1fr)_330px]" : "grid-cols-1",
        )}
      >
        <Card className="min-h-0">
          <CardHeader className="flex-row items-center justify-between py-3">
            <CardTitle className="flex items-center gap-2 text-[13.5px]">
              <CalendarDays size={14} className="text-zinc-500" />
              {title}
            </CardTitle>
            <div className="flex items-center gap-1">
              <Button
                variant="ghost"
                size="icon"
                className="h-7 w-7"
                aria-label="Anterior"
                onClick={() => move(-1)}
              >
                <ChevronLeft size={14} />
              </Button>
              <Button
                variant="outline"
                size="sm"
                className="h-7"
                onClick={goToday}
              >
                Hoje
              </Button>
              <Button
                variant="ghost"
                size="icon"
                className="h-7 w-7"
                aria-label="Próximo"
                onClick={() => move(1)}
              >
                <ChevronRight size={14} />
              </Button>
            </div>
          </CardHeader>
          <CardContent className={view === "month" ? "p-2" : "p-3"}>
            {view === "month" ? (
            <MonthGridView
              days={days}
              month={month}
              selectedDay={selectedDay}
              items={visibleItems}
              onSelectDay={setSelectedDay}
              onOpenItem={openItem}
            />
            ) : view === "day" ? (
              <CalendarDayView
                items={selectedItems}
                day={selectedDay}
                onOpenItem={openItem}
              />
            ) : (
              <CalendarColumns
                days={days}
                items={visibleItems}
                selectedDay={selectedDay}
                onSelectDay={setSelectedDay}
                onOpenItem={openItem}
              />
            )}
          </CardContent>
        </Card>

        {view === "month" ? (
        <SelectedDayCard
          selectedDay={selectedDay}
          selectedItems={selectedItems}
          onOpenItem={openItem}
        />
        ) : null}
      </div>

      <ImportGoogleModal
        open={importOpen}
        sources={sources}
        onClose={() => setImportOpen(false)}
        onChanged={async () => {
          await load();
        }}
      />

      <EventModal
        open={editing !== null}
        event={editing === "new" ? null : editing}
        sources={sources}
        selectedDay={selectedDay}
        onClose={() => {
          setEditing(null);
          if (focusEvent) setParams({}, { replace: true });
        }}
        onSaved={async () => {
          setEditing(null);
          if (focusEvent) setParams({}, { replace: true });
          await load();
        }}
      />
    </PageBody>
  );
}

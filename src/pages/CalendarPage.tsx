import { useCallback, useEffect, useMemo, useState } from "react";
import { useNavigate, useSearchParams } from "react-router-dom";
import {
  addMonths,
  format,
  isSameMonth,
  isToday,
  startOfDay,
  startOfMonth,
  subMonths,
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
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { EmptyState } from "@/components/ui/empty-state";
import { IconLabel } from "@/components/ui/icon-label";
import { EventModal } from "@/components/calendar/event-modal";
import { ImportGoogleModal } from "@/components/calendar/import-google-modal";
import { apiGet, apiPost } from "@/lib/api";
import {
  CALENDAR_CATEGORIES,
  calendarRange,
  isVisibleItem,
  itemTimeLabel,
  itemsForDay,
  monthGrid,
  type CalendarCategory,
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

const WEEKDAYS = ["dom", "seg", "ter", "qua", "qui", "sex", "sáb"];

const KIND_LABEL: Record<string, string> = {
  event: "Evento",
  routine: "Rotina",
  recipe: "Receita",
  run: "Execução",
  recipe_run: "Receita",
  queued: "Fila",
};

function capitalized(value: string): string {
  return value.charAt(0).toUpperCase() + value.slice(1);
}

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

  const days = useMemo(() => monthGrid(month), [month]);
  const range = useMemo(() => calendarRange(month), [month]);
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

  useEffect(() => {
    setSelectedDay(
      isSameMonth(new Date(), month) ? startOfDay(new Date()) : startOfMonth(month),
    );
  }, [month]);

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
        eyebrow="tempo e execução"
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
        <p className="rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[11px] text-red-300">
          {error}
        </p>
      ) : null}

      <div className="flex flex-wrap items-center gap-1.5">
        {CALENDAR_CATEGORIES.map((category) => (
          <button
            key={category.id}
            type="button"
            aria-pressed={filters[category.id]}
            onClick={() =>
              setFilters((current) => ({
                ...current,
                [category.id]: !current[category.id],
              }))
            }
            className={cn(
              "rounded-full border px-2.5 py-1 text-[10px] transition-colors",
              filters[category.id]
                ? "border-blue-500/40 bg-blue-500/10 text-blue-200"
                : "border-zinc-800 bg-zinc-950/40 text-zinc-500 hover:text-zinc-300",
            )}
          >
            {category.label}
          </button>
        ))}
        <span className="ml-auto font-mono text-[9px] text-zinc-600">
          {visibleItems.length} item(ns) visíveis
        </span>
      </div>

      <div className="grid min-h-0 gap-3 xl:grid-cols-[minmax(0,1fr)_330px]">
        <Card className="min-h-0">
          <CardHeader className="flex-row items-center justify-between py-3">
            <CardTitle className="flex items-center gap-2 text-[12px]">
              <CalendarDays size={14} className="text-zinc-500" />
              {capitalized(format(month, "MMMM 'de' yyyy", { locale: ptBR }))}
            </CardTitle>
            <div className="flex items-center gap-1">
              <Button
                variant="ghost"
                size="icon"
                className="h-7 w-7"
                aria-label="Mês anterior"
                onClick={() => setMonth((current) => subMonths(current, 1))}
              >
                <ChevronLeft size={14} />
              </Button>
              <Button
                variant="outline"
                size="sm"
                className="h-7"
                onClick={() => setMonth(startOfMonth(new Date()))}
              >
                Hoje
              </Button>
              <Button
                variant="ghost"
                size="icon"
                className="h-7 w-7"
                aria-label="Próximo mês"
                onClick={() => setMonth((current) => addMonths(current, 1))}
              >
                <ChevronRight size={14} />
              </Button>
            </div>
          </CardHeader>
          <CardContent className="p-2">
            <div className="grid grid-cols-7 gap-1">
              {WEEKDAYS.map((weekday) => (
                <div
                  key={weekday}
                  className="px-1 py-1 text-center text-[9px] uppercase tracking-[0.12em] text-zinc-600"
                >
                  {weekday}
                </div>
              ))}
              {days.map((day) => {
                const dayItems = itemsForDay(visibleItems, day);
                const selected = day.toDateString() === selectedDay.toDateString();
                return (
                  <div
                    key={day.toISOString()}
                    role="button"
                    tabIndex={0}
                    onClick={() => setSelectedDay(startOfDay(day))}
                    onKeyDown={(event) => {
                      if (event.key === "Enter" || event.key === " ") {
                        setSelectedDay(startOfDay(day));
                      }
                    }}
                    className={cn(
                      "min-h-[104px] rounded-lg border p-1.5 text-left transition-colors",
                      selected
                        ? "border-blue-500/50 bg-blue-500/5"
                        : "border-zinc-800/70 bg-zinc-950/30 hover:border-zinc-700",
                      isSameMonth(day, month) ? undefined : "opacity-45",
                    )}
                  >
                    <div className="flex items-center justify-between">
                      <span
                        className={cn(
                          "flex h-5 w-5 items-center justify-center rounded-full text-[10px]",
                          isToday(day)
                            ? "bg-blue-500 font-semibold text-white"
                            : "text-zinc-400",
                        )}
                      >
                        {format(day, "d")}
                      </span>
                      {dayItems.length > 0 ? (
                        <span className="font-mono text-[8px] text-zinc-600">
                          {dayItems.length}
                        </span>
                      ) : null}
                    </div>
                    <div className="mt-1 space-y-0.5">
                      {dayItems.slice(0, 3).map((item) => (
                        <button
                          key={item.id}
                          type="button"
                          onClick={(event) => {
                            event.stopPropagation();
                            openItem(item);
                          }}
                          title={`${itemTimeLabel(item)} · ${item.title}`}
                          className={cn(
                            "block w-full truncate rounded border-l-2 px-1 py-0.5 text-left text-[8px] leading-tight",
                            item.status === "PAST"
                              ? "opacity-60"
                              : item.status === "ACTIVE"
                                ? "ring-1 ring-blue-500/40"
                                : undefined,
                          )}
                          style={{
                            borderLeftColor: item.color ?? "#3b82f6",
                            backgroundColor: `${item.color ?? "#3b82f6"}18`,
                          }}
                        >
                          {item.all_day ? "" : `${format(new Date(item.start), "HH:mm")} `}
                          {item.title}
                        </button>
                      ))}
                      {dayItems.length > 3 ? (
                        <p className="px-1 text-[8px] text-zinc-600">
                          +{dayItems.length - 3} mais
                        </p>
                      ) : null}
                    </div>
                  </div>
                );
              })}
            </div>
          </CardContent>
        </Card>

        <Card className="min-h-0">
          <CardHeader className="py-3">
            <CardTitle className="text-[12px]">
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
                <button
                  key={item.id}
                  type="button"
                  onClick={() => openItem(item)}
                  className={cn(
                    "flex w-full items-start gap-2 rounded-lg border border-zinc-800/70 px-2.5 py-2 text-left transition-colors hover:border-zinc-700",
                    item.status === "PAST" ? "opacity-60" : undefined,
                  )}
                >
                  <span
                    aria-hidden
                    className="mt-1 h-2 w-2 shrink-0 rounded-full"
                    style={{ backgroundColor: item.color ?? "#3b82f6" }}
                  />
                  <span className="min-w-0 flex-1">
                    <span className="flex items-center gap-1.5">
                      <span className="truncate text-[11px] text-zinc-200">{item.title}</span>
                      <Badge variant={item.status === "ACTIVE" ? "blue" : "default"}>
                        {KIND_LABEL[item.kind] ?? item.kind}
                      </Badge>
                    </span>
                    <span className="mt-0.5 block truncate font-mono text-[9px] text-zinc-600">
                      {itemTimeLabel(item)}
                      {item.subtitle ? ` · ${item.subtitle}` : ""}
                    </span>
                  </span>
                </button>
              ))
            )}
          </CardContent>
        </Card>
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

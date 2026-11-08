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
  Loader2,
  Plus,
  RefreshCw,
  Trash2,
} from "lucide-react";
import { PageBody, PageHeader } from "@/components/layout/PageHeader";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { EmptyState } from "@/components/ui/empty-state";
import { IconLabel } from "@/components/ui/icon-label";
import { Modal } from "@/components/ui/modal";
import { Switch } from "@/components/ui/switch";
import { apiDelete, apiGet, apiPatch, apiPost } from "@/lib/api";
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
import { confirmAction } from "@/store/confirm-store";
import { toast } from "@/store/toast-store";
import type {
  CalendarAgenda,
  CalendarEvent,
  CalendarItem,
  CalendarSource,
} from "@/lib/types";
import { cn } from "@/lib/utils";
import {
  fieldInputClass as inputClass,
  fieldLabelClass as labelClass,
} from "@/lib/ui-classes";

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

type GoogleCalendar = {
  id: string;
  summary?: string;
  backgroundColor?: string;
  primary?: boolean;
  timeZone?: string;
  accessRole?: string;
};

function ImportGoogleModal({
  open,
  sources,
  onClose,
  onChanged,
}: {
  open: boolean;
  sources: CalendarSource[];
  onClose: () => void;
  onChanged: () => Promise<void>;
}) {
  const activeWorkspaceId = useAppStore((state) => state.activeWorkspaceId);
  const navigate = useNavigate();
  const [calendars, setCalendars] = useState<GoogleCalendar[]>([]);
  const [selected, setSelected] = useState<string[]>([]);
  const [loading, setLoading] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [reports, setReports] = useState<
    Array<{ source_name: string; imported: number; error?: string }>
  >([]);
  const googleSources = sources.filter((source) => source.provider === "GOOGLE");

  useEffect(() => {
    if (!open || !activeWorkspaceId) return;
    let cancelled = false;
    setLoading(true);
    setError(null);
    setReports([]);
    void apiGet<{ items?: GoogleCalendar[] }>(
      `/api/calendar/google/calendars?workspace_id=${encodeURIComponent(activeWorkspaceId)}`,
    )
      .then((value) => {
        if (cancelled) return;
        const items = Array.isArray(value.items) ? value.items : [];
        setCalendars(items);
        setSelected(
          items.filter((item) => item.primary).map((item) => item.id).slice(0, 1),
        );
      })
      .catch((cause) => {
        if (!cancelled) {
          setCalendars([]);
          setError(cause instanceof Error ? cause.message : String(cause));
        }
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [activeWorkspaceId, open]);

  if (!open) return null;

  async function importSelected() {
    const chosen = calendars.filter((calendar) => selected.includes(calendar.id));
    if (chosen.length === 0) {
      setError("Selecione pelo menos um calendário.");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const result = await apiPost<{
        sources: CalendarSource[];
        reports: Array<{ source_name: string; imported: number; error?: string }>;
      }>(
        `/api/calendar/google/import?workspace_id=${encodeURIComponent(activeWorkspaceId ?? "")}`,
        {
          calendars: chosen.map((calendar) => ({
            external_id: calendar.id,
            name: calendar.summary ?? calendar.id,
            color_hex: calendar.backgroundColor,
            timezone: calendar.timeZone ?? "UTC",
          })),
        },
      );
      setReports(result.reports ?? []);
      const failed = (result.reports ?? []).filter((report) => report.error);
      toast({
        tone: failed.length > 0 ? "error" : "success",
        title:
          failed.length > 0
            ? "Importação parcial"
            : "Calendários importados",
        description:
          failed.length > 0
            ? failed.map((report) => `${report.source_name}: ${report.error}`).join(" · ")
            : `${result.sources.length} calendário(s) conectados.`,
      });
      await onChanged();
      if (failed.length === 0) onClose();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(false);
    }
  }

  async function toggleSource(
    source: CalendarSource,
    patch: { is_visible?: boolean; sync_enabled?: boolean },
  ) {
    try {
      await apiPatch(`/api/calendar/sources/${source.id}`, patch);
      await onChanged();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  }

  async function syncSource(source: CalendarSource) {
    setBusy(true);
    try {
      const report = await apiPost<{
        source_name: string;
        imported: number;
        error?: string;
      }>(`/api/calendar/sources/${source.id}/sync`);
      toast({
        tone: report.error ? "error" : "success",
        title: report.error ? "Falha ao sincronizar" : "Calendário sincronizado",
        description: report.error ?? `${report.imported} evento(s) atualizados.`,
      });
      await onChanged();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(false);
    }
  }

  async function removeSource(source: CalendarSource) {
    if (
      !(await confirmAction({
        title: `Desconectar “${source.name}”?`,
        description:
          "Os eventos em cache deste calendário serão removidos. O Google Calendar não é alterado.",
        confirmLabel: "Desconectar",
        tone: "destructive",
      }))
    ) {
      return;
    }
    try {
      await apiDelete(`/api/calendar/sources/${source.id}`);
      toast({ tone: "success", title: "Calendário desconectado", description: source.name });
      await onChanged();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  }

  return (
    <Modal
      open={open}
      onClose={onClose}
      title="Calendários Google"
      description="Importe calendários para leitura e sincronização. Eventos criados ou editados no OpenDesk são enviados ao Google."
      className="max-w-lg"
    >
      <div className="space-y-4">
        {googleSources.length > 0 ? (
          <section className="space-y-2">
            <p className={labelClass}>Conectados</p>
            {googleSources.map((source) => (
              <div
                key={source.id}
                className="flex flex-wrap items-center gap-2 rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-2.5 py-2"
              >
                <span
                  aria-hidden
                  className="h-2 w-2 shrink-0 rounded-full"
                  style={{ backgroundColor: source.color_hex }}
                />
                <span className="min-w-0 flex-1 truncate text-[11px] text-zinc-200">
                  {source.name}
                </span>
                <span className="text-[9px] text-zinc-600">
                  {source.last_synced_at
                    ? `sincronizado ${new Date(source.last_synced_at).toLocaleString("pt-BR")}`
                    : "nunca sincronizado"}
                </span>
                <Switch
                  checked={source.is_visible}
                  onCheckedChange={(checked) =>
                    void toggleSource(source, { is_visible: checked })
                  }
                  label={`Exibir ${source.name}`}
                />
                <Switch
                  checked={source.sync_enabled}
                  onCheckedChange={(checked) =>
                    void toggleSource(source, { sync_enabled: checked })
                  }
                  label={`Sincronizar ${source.name}`}
                />
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() => void syncSource(source)}
                  disabled={busy}
                >
                  <RefreshCw size={12} />
                  Sincronizar
                </Button>
                <Button
                  variant="ghost"
                  size="icon"
                  className="h-7 w-7 text-red-300"
                  aria-label={`Desconectar ${source.name}`}
                  onClick={() => void removeSource(source)}
                >
                  <Trash2 size={12} />
                </Button>
              </div>
            ))}
          </section>
        ) : null}

        <section className="space-y-2">
          <p className={labelClass}>Importar do Google</p>
          {loading ? (
            <p className="flex items-center gap-2 text-[11px] text-zinc-500">
              <Loader2 size={13} className="animate-spin" />
              Buscando calendários…
            </p>
          ) : error ? (
            <div className="space-y-2 rounded-lg border border-amber-500/20 bg-amber-500/10 px-3 py-2 text-[11px] text-amber-200">
              <p>{error}</p>
              <Button
                variant="outline"
                size="sm"
                onClick={() => navigate("/integrations?section=google")}
              >
                Abrir Integrações → Google
              </Button>
            </div>
          ) : calendars.length === 0 ? (
            <p className="text-[11px] text-zinc-500">
              Nenhum calendário disponível. Conecte a conta Google primeiro.
            </p>
          ) : (
            <div className="max-h-60 space-y-1 overflow-y-auto">
              {calendars.map((calendar) => (
                <label
                  key={calendar.id}
                  className="flex items-center gap-2 rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-2.5 py-2 text-[11px] text-zinc-300"
                >
                  <input
                    type="checkbox"
                    checked={selected.includes(calendar.id)}
                    onChange={(event) =>
                      setSelected((current) =>
                        event.target.checked
                          ? [...current, calendar.id]
                          : current.filter((id) => id !== calendar.id),
                      )
                    }
                    className="accent-blue-500"
                  />
                  <span
                    aria-hidden
                    className="h-2 w-2 rounded-full"
                    style={{ backgroundColor: calendar.backgroundColor ?? "#3b82f6" }}
                  />
                  <span className="min-w-0 flex-1 truncate">
                    {calendar.summary ?? calendar.id}
                  </span>
                  {calendar.primary ? <Badge variant="blue">principal</Badge> : null}
                </label>
              ))}
            </div>
          )}
        </section>

        {reports.some((report) => report.error) ? (
          <div className="space-y-1 rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[10px] text-red-300">
            {reports
              .filter((report) => report.error)
              .map((report) => (
                <p key={report.source_name}>
                  {report.source_name}: {report.error}
                </p>
              ))}
          </div>
        ) : null}

        <div className="flex justify-end gap-2 border-t border-zinc-800/70 pt-3">
          <Button variant="ghost" size="sm" onClick={onClose} disabled={busy}>
            Fechar
          </Button>
          <Button
            size="sm"
            onClick={() => void importSelected()}
            disabled={busy || loading || selected.length === 0}
          >
            {busy ? <Loader2 size={13} className="animate-spin" /> : <CloudDownload size={13} />}
            Importar selecionados
          </Button>
        </div>
      </div>
    </Modal>
  );
}

function EventModal({
  open,
  event,
  sources,
  selectedDay,
  onClose,
  onSaved,
}: {
  open: boolean;
  event: CalendarEvent | null;
  sources: CalendarSource[];
  selectedDay: Date;
  onClose: () => void;
  onSaved: () => Promise<void>;
}) {
  const activeWorkspaceId = useAppStore((state) => state.activeWorkspaceId);
  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");
  const [location, setLocation] = useState("");
  const [allDay, setAllDay] = useState(false);
  const [sourceId, setSourceId] = useState("");
  const [startText, setStartText] = useState("");
  const [endText, setEndText] = useState("");
  const [startDate, setStartDate] = useState("");
  const [endDate, setEndDate] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    setError(null);
    if (event) {
      setTitle(event.title);
      setDescription(event.description ?? "");
      setLocation(event.location ?? "");
      setAllDay(event.all_day);
      setSourceId(event.source_id ?? "");
      setStartText(format(new Date(event.start_at), "yyyy-MM-dd'T'HH:mm"));
      setEndText(format(new Date(event.end_at), "yyyy-MM-dd'T'HH:mm"));
      setStartDate(
        event.all_day
          ? event.start_at.slice(0, 10)
          : format(new Date(event.start_at), "yyyy-MM-dd"),
      );
      setEndDate(
        event.all_day
          ? event.end_at.slice(0, 10)
          : format(new Date(event.end_at), "yyyy-MM-dd"),
      );
    } else {
      const start = new Date(
        selectedDay.getFullYear(),
        selectedDay.getMonth(),
        selectedDay.getDate(),
        9,
        0,
      );
      const end = new Date(start.getTime() + 60 * 60 * 1000);
      setTitle("");
      setDescription("");
      setLocation("");
      setAllDay(false);
      setSourceId("");
      setStartText(format(start, "yyyy-MM-dd'T'HH:mm"));
      setEndText(format(end, "yyyy-MM-dd'T'HH:mm"));
      setStartDate(format(start, "yyyy-MM-dd"));
      setEndDate(format(end, "yyyy-MM-dd"));
    }
  }, [event, open, selectedDay]);

  useEffect(() => {
    if (!open || event || sourceId || sources.length === 0) return;
    setSourceId(
      sources.find((source) => source.provider === "LOCAL")?.id ??
        sources[0].id,
    );
  }, [event, open, sourceId, sources]);

  if (!open) return null;

  function payload() {
    const start = allDay
      ? new Date(`${startDate}T00:00:00Z`)
      : new Date(startText);
    const end = allDay
      ? new Date(`${endDate}T23:59:59Z`)
      : new Date(endText);
    if (Number.isNaN(start.getTime()) || Number.isNaN(end.getTime())) {
      throw new Error("Informe início e fim válidos.");
    }
    if (end <= start) {
      throw new Error("O fim precisa ser depois do início.");
    }
    return {
      title: title.trim(),
      description: description.trim() || null,
      location: location.trim() || null,
      start_at: start.toISOString(),
      end_at: end.toISOString(),
      all_day: allDay,
      source_id: sourceId || null,
      timezone:
        Intl.DateTimeFormat().resolvedOptions().timeZone || "UTC",
    };
  }

  async function save() {
    if (!activeWorkspaceId) return;
    if (!title.trim()) {
      setError("O título é obrigatório.");
      return;
    }
    setSaving(true);
    setError(null);
    try {
      const body = payload();
      if (event) {
        await apiPatch(`/api/calendar/events/${event.id}`, body);
        toast({ tone: "success", title: "Evento atualizado", description: body.title });
      } else {
        await apiPost(
          `/api/calendar/events?workspace_id=${encodeURIComponent(activeWorkspaceId)}`,
          body,
        );
        toast({ tone: "success", title: "Evento criado", description: body.title });
      }
      await onSaved();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setSaving(false);
    }
  }

  async function remove() {
    if (!event) return;
    if (
      !(await confirmAction({
        title: `Excluir “${event.title}”?`,
        description: "O evento será removido do calendário local.",
        confirmLabel: "Excluir evento",
        tone: "destructive",
      }))
    ) {
      return;
    }
    setSaving(true);
    setError(null);
    try {
      await apiDelete(`/api/calendar/events/${event.id}`);
      toast({ tone: "success", title: "Evento excluído", description: event.title });
      await onSaved();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setSaving(false);
    }
  }

  return (
    <Modal
      open={open}
      onClose={onClose}
      title={event ? "Editar evento" : "Novo evento"}
      description="Eventos locais aparecem no calendário e podem ser usados como contexto para receitas."
      className="max-w-lg"
    >
      <div className="space-y-3">
        <label className="block">
          <span className={labelClass}>Título</span>
          <input
            autoFocus
            value={title}
            onChange={(event) => setTitle(event.target.value)}
            placeholder="Planejamento da semana"
            className={inputClass}
          />
        </label>
        <label className="block">
          <span className={labelClass}>Descrição</span>
          <textarea
            value={description}
            onChange={(event) => setDescription(event.target.value)}
            rows={3}
            placeholder="Pauta, links ou contexto…"
            className="w-full resize-y rounded-lg border border-zinc-800 bg-zinc-900/50 p-2.5 text-[11px] text-zinc-100 outline-none placeholder:text-zinc-600 focus:border-blue-500/60"
          />
        </label>
        <label className="block">
          <span className={labelClass}>Local</span>
          <input
            value={location}
            onChange={(event) => setLocation(event.target.value)}
            placeholder="Sala, link ou endereço"
            className={inputClass}
          />
        </label>
        {event ? null : (
          <label className="block">
            <span className={labelClass}>Calendário</span>
            <select
              value={sourceId}
              onChange={(event) => setSourceId(event.target.value)}
              className={inputClass}
            >
              {sources.map((source) => (
                <option key={source.id} value={source.id}>
                  {source.name}
                  {source.provider === "GOOGLE" ? " (Google)" : " (local)"}
                </option>
              ))}
            </select>
          </label>
        )}
        <label className="flex items-center gap-2 text-[11px] text-zinc-400">
          <input
            type="checkbox"
            checked={allDay}
            onChange={(event) => setAllDay(event.target.checked)}
            className="accent-blue-500"
          />
          Dia inteiro
        </label>
        <div className="grid gap-3 sm:grid-cols-2">
          {allDay ? (
            <>
              <label className="block">
                <span className={labelClass}>Início</span>
                <input
                  type="date"
                  value={startDate}
                  onChange={(event) => setStartDate(event.target.value)}
                  className={inputClass}
                />
              </label>
              <label className="block">
                <span className={labelClass}>Fim</span>
                <input
                  type="date"
                  value={endDate}
                  onChange={(event) => setEndDate(event.target.value)}
                  className={inputClass}
                />
              </label>
            </>
          ) : (
            <>
              <label className="block">
                <span className={labelClass}>Início</span>
                <input
                  type="datetime-local"
                  value={startText}
                  onChange={(event) => setStartText(event.target.value)}
                  className={inputClass}
                />
              </label>
              <label className="block">
                <span className={labelClass}>Fim</span>
                <input
                  type="datetime-local"
                  value={endText}
                  onChange={(event) => setEndText(event.target.value)}
                  className={inputClass}
                />
              </label>
            </>
          )}
        </div>
        <p className="text-[9px] text-zinc-600">
          Fuso: {Intl.DateTimeFormat().resolvedOptions().timeZone || "UTC"}
        </p>
        {error ? (
          <p className="rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[11px] text-red-300">
            {error}
          </p>
        ) : null}
        <div className="flex flex-wrap justify-end gap-2 border-t border-zinc-800/70 pt-3">
          {event ? (
            <Button
              variant="destructive"
              size="sm"
              onClick={() => void remove()}
              disabled={saving}
              className="mr-auto"
            >
              <Trash2 size={13} />
              Excluir
            </Button>
          ) : null}
          <Button variant="ghost" size="sm" onClick={onClose} disabled={saving}>
            Cancelar
          </Button>
          <Button size="sm" onClick={() => void save()} disabled={saving}>
            {saving ? <Loader2 size={13} className="animate-spin" /> : null}
            {event ? "Salvar" : "Criar evento"}
          </Button>
        </div>
      </div>
    </Modal>
  );
}

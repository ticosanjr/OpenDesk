import { useEffect, useState } from "react";
import { format } from "date-fns";
import { Loader2, Trash2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Modal } from "@/components/ui/modal";
import { apiDelete, apiPatch, apiPost } from "@/lib/api";
import { useAppStore } from "@/store/app-store";
import { confirmAction } from "@/store/confirm-store";
import { toast } from "@/store/toast-store";
import type { CalendarEvent, CalendarSource } from "@/lib/types";
import {
  fieldInputClass as inputClass,
  fieldLabelClass as labelClass,
} from "@/lib/ui-classes";

export function EventModal({
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
      size="md"
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
            className="w-full resize-y rounded-lg border border-zinc-800 bg-zinc-900/50 p-2.5 text-[13px] text-zinc-100 outline-none placeholder:text-zinc-500 focus:border-blue-500/60"
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
        <label className="flex items-center gap-2 text-[13px] text-zinc-400">
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
        <p className="text-[11.5px] text-zinc-500">
          Fuso: {Intl.DateTimeFormat().resolvedOptions().timeZone || "UTC"}
        </p>
        {error ? (
          <p className="rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[13px] text-red-300">
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

import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { CloudDownload, Loader2, RefreshCw, Trash2 } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Modal } from "@/components/ui/modal";
import { Switch } from "@/components/ui/switch";
import { apiDelete, apiGet, apiPatch, apiPost } from "@/lib/api";
import { useAppStore } from "@/store/app-store";
import { confirmAction } from "@/store/confirm-store";
import { toast } from "@/store/toast-store";
import type { CalendarSource } from "@/lib/types";
import { fieldLabelClass as labelClass } from "@/lib/ui-classes";

type GoogleCalendar = {
  id: string;
  summary?: string;
  backgroundColor?: string;
  primary?: boolean;
  timeZone?: string;
  accessRole?: string;
};

export function ImportGoogleModal({
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
                <span className="text-[9px] text-zinc-500">
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


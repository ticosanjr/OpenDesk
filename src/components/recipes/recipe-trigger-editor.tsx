import { useEffect, useState } from "react";
import { Trash2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { FieldHint } from "@/components/ui/field-hint";
import { PathInput } from "@/components/ui/path-input";
import { apiGet } from "@/lib/api";
import { TRIGGER_TYPES, triggerDescription } from "@/lib/trigger-types";
import type { CalendarSource } from "@/lib/types";
import { cn } from "@/lib/utils";
import { inputClass, LabeledField } from "./recipe-fields";
import type { Dictionary } from "./recipe-types";

const DEFAULT_TIMEZONE =
  Intl.DateTimeFormat().resolvedOptions().timeZone || "UTC";

export function TriggerEditor({
  trigger,
  index,
  workspaceId,
  onChange,
  onRemove,
}: {
  trigger: Dictionary;
  index: number;
  workspaceId?: string | null;
  onChange: (value: Dictionary) => void;
  onRemove: () => void;
}) {
  const type = String(trigger.type ?? "MANUAL");
  const config = (trigger.config ?? {}) as Dictionary;
  const [calendars, setCalendars] = useState<CalendarSource[]>([]);

  useEffect(() => {
    if (type !== "CALENDAR_EVENT" || !workspaceId) return;
    let cancelled = false;
    void apiGet<CalendarSource[]>(
      `/api/calendar/sources?workspace_id=${encodeURIComponent(workspaceId)}`,
    )
      .then((value) => {
        if (!cancelled) setCalendars(Array.isArray(value) ? value : []);
      })
      .catch(() => {
        if (!cancelled) setCalendars([]);
      });
    return () => {
      cancelled = true;
    };
  }, [type, workspaceId]);

  function patchConfig(value: Dictionary) {
    onChange({ ...trigger, config: { ...config, ...value } });
  }

  return (
    <div className="rounded-lg border border-blue-500/20 bg-zinc-950/40 p-2">
      <div className="flex items-center gap-1">
        <select
          value={type}
          onChange={(event) => {
            const type = event.target.value;
            onChange({
              ...trigger,
              type,
              config:
                type === "CRON"
                  ? { cron: "0 8 * * *", timezone: DEFAULT_TIMEZONE }
                  : {},
            });
          }}
          className={cn(inputClass, "flex-1")}
          aria-label={`Tipo do gatilho ${index + 1}`}
        >
          {TRIGGER_TYPES.map((item) => (
            <option key={item.value} value={item.value}>
              {item.label}
            </option>
          ))}
        </select>
        <FieldHint hint={triggerDescription(type)} />
        <Button
          variant="ghost"
          size="icon"
          className="h-7 w-7 text-red-300"
          aria-label="Remover gatilho"
          onClick={onRemove}
          title="Remover gatilho"
        >
          <Trash2 size={11} />
        </Button>
      </div>
      <p className="mt-1 text-[9px] leading-snug text-zinc-500">{triggerDescription(type)}</p>
      <div className="mt-1.5 grid gap-1.5 sm:grid-cols-2">
        {type === "FOLDER_WATCH" || type === "RSYNC_FOLDER" ? (
          <PathInput
            value={String(config.path ?? "")}
            onChange={(value) => patchConfig({ path: value })}
            title="Escolher pasta monitorada"
            placeholder="input/dropzone ou caminho absoluto"
            hint="Se o caminho for relativo, ele é resolvido dentro do workspace."
            inputClassName="h-8 text-[10px]"
          />
        ) : null}
        {type === "CRON" ? <CronFields config={config} onChange={patchConfig} /> : null}
        {type === "IMAP_IDLE" ? (
          <>
            <LabeledField label="Caixa de entrada">
              <input
                value={String(config.mailbox ?? "INBOX")}
                onChange={(event) => patchConfig({ mailbox: event.target.value })}
                className={inputClass}
                placeholder="INBOX"
              />
            </LabeledField>
            <LabeledField label="Filtrar remetente">
              <input
                value={String(config.from_filter ?? "")}
                onChange={(event) => patchConfig({ from_filter: event.target.value })}
                className={inputClass}
                placeholder="cliente@exemplo.com"
              />
            </LabeledField>
          </>
        ) : null}
        {type === "SIGNAL_MSG" ? (
          <LabeledField label="Filtrar número">
            <input
              value={String(config.from_filter ?? "")}
              onChange={(event) => patchConfig({ from_filter: event.target.value })}
              className={inputClass}
              placeholder="+5511999999999"
            />
          </LabeledField>
        ) : null}
        {type === "WEBHOOK" ? (
          <>
            <LabeledField label="Segredo do webhook">
              <input
                value={String(config.secret ?? "")}
                onChange={(event) => patchConfig({ secret: event.target.value })}
                className={inputClass}
                placeholder="opcional — usa o segredo global quando vazio"
              />
            </LabeledField>
            <LabeledField label="Formato esperado">
              <input
                value={String(config.format ?? "json")}
                onChange={(event) => patchConfig({ format: event.target.value })}
                className={inputClass}
                placeholder="json"
              />
            </LabeledField>
          </>
        ) : null}
        {type === "CALENDAR_EVENT" ? (
          <>
            <LabeledField label="Calendário">
              <select
                value={String(config.calendar_id ?? "")}
                onChange={(event) => patchConfig({ calendar_id: event.target.value })}
                className={inputClass}
              >
                <option value="">Qualquer calendário</option>
                {calendars.map((calendar) => (
                  <option key={calendar.id} value={calendar.id}>
                    {calendar.name}
                    {calendar.provider === "GOOGLE" ? " (Google)" : " (local)"}
                  </option>
                ))}
              </select>
            </LabeledField>
            <LabeledField label="Título contém">
              <input
                value={String(config.match ?? "")}
                onChange={(event) => patchConfig({ match: event.target.value })}
                className={inputClass}
                placeholder="opcional — ex.: Planejamento"
              />
            </LabeledField>
            <LabeledField label="Antecedência (min)">
              <input
                type="number"
                min={0}
                value={Number(config.lead_minutes ?? 0)}
                onChange={(event) =>
                  patchConfig({ lead_minutes: Number(event.target.value) })
                }
                className={inputClass}
              />
            </LabeledField>
            <LabeledField label="Tolerância (min)">
              <input
                type="number"
                min={1}
                value={Number(config.lookahead_minutes ?? 10)}
                onChange={(event) =>
                  patchConfig({ lookahead_minutes: Number(event.target.value) })
                }
                className={inputClass}
              />
            </LabeledField>
          </>
        ) : null}
        {type === "SQL_POLL" ? (
          <>
            <LabeledField label="Consulta SQL">
              <input
                value={String(config.query ?? "")}
                onChange={(event) => patchConfig({ query: event.target.value })}
                className={inputClass}
                placeholder="SELECT ..."
              />
            </LabeledField>
            <LabeledField label="Intervalo (segundos)">
              <input
                type="number"
                min={15}
                value={Number(config.interval_seconds ?? 60)}
                onChange={(event) =>
                  patchConfig({ interval_seconds: Number(event.target.value) })
                }
                className={inputClass}
              />
            </LabeledField>
          </>
        ) : null}
      </div>
    </div>
  );
}

function CronFields({
  config,
  onChange,
}: {
  config: Dictionary;
  onChange: (value: Dictionary) => void;
}) {
  const cron = String(config.cron ?? "0 8 * * *");
  const preset = cron === "0 * * * *" ? "hourly" : cron.startsWith("0 ") ? "daily" : cron.startsWith("*/") ? "minutes" : "custom";
  return (
    <>
      <LabeledField label="Frequência">
        <select
          value={preset}
          onChange={(event) => {
            const value = event.target.value;
            if (value === "hourly") onChange({ cron: "0 * * * *" });
            else if (value === "daily") onChange({ cron: "0 8 * * *" });
            else if (value === "minutes") onChange({ cron: "*/15 * * * *" });
          }}
          className={inputClass}
        >
          <option value="hourly">A cada hora</option>
          <option value="daily">Todos os dias às 8h</option>
          <option value="minutes">A cada 15 minutos</option>
          <option value="custom">Personalizado</option>
        </select>
      </LabeledField>
      <LabeledField label="Expressão cron">
        <input
          value={cron}
          onChange={(event) => onChange({ cron: event.target.value })}
          className={inputClass}
          placeholder="0 8 * * *"
        />
      </LabeledField>
      <LabeledField label="Fuso horário">
        <input
          value={String(config.timezone ?? DEFAULT_TIMEZONE)}
          onChange={(event) => onChange({ timezone: event.target.value })}
          className={inputClass}
          placeholder={DEFAULT_TIMEZONE}
        />
      </LabeledField>
    </>
  );
}


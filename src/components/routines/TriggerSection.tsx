import {
  CalendarClock,
  ChevronDown,
  ChevronRight,
} from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { PathInput } from "@/components/ui/path-input";
import { TimezoneSelect } from "@/components/ui/timezone-select";
import { RecurrenceEditor } from "@/components/routines/RecurrenceEditor";
import type { Recurrence, SchedulePreview } from "@/lib/types";
import { TRIGGER_TYPES, triggerDescription, triggerLabel } from "@/lib/trigger-types";
import { cn } from "@/lib/utils";
import {
  fieldInputClass as inputClass,
  fieldLabelClass as labelClass,
} from "@/lib/ui-classes";

export function TriggerSection({
  triggerType,
  setTriggerType,
  triggerConfig,
  setTriggerConfig,
  whenOpen,
  setWhenOpen,
  recurrence,
  setRecurrence,
  preview,
  timezone,
  setTimezone,
}: {
  triggerType: string;
  setTriggerType: (value: string) => void;
  triggerConfig: Record<string, unknown>;
  setTriggerConfig: (
    value:
      | Record<string, unknown>
      | ((current: Record<string, unknown>) => Record<string, unknown>),
  ) => void;
  whenOpen: boolean;
  setWhenOpen: (value: boolean | ((current: boolean) => boolean)) => void;
  recurrence: Recurrence;
  setRecurrence: (value: Recurrence) => void;
  preview: SchedulePreview | null;
  timezone: string;
  setTimezone: (value: string) => void;
}) {
  return (
          <div className="rounded-xl border border-zinc-800/70 bg-zinc-950/30">
            <button
              type="button"
              className="flex w-full flex-wrap items-center gap-2 px-3 py-2.5 text-left"
              onClick={() => setWhenOpen((current) => !current)}
              aria-expanded={whenOpen}
            >
              <CalendarClock size={14} className="shrink-0 text-blue-300" />
              <span className="text-[13px] font-semibold text-zinc-200">
                Quando executar
              </span>
              <Badge variant="blue">{triggerLabel(triggerType)}</Badge>
              {whenOpen ? (
                <ChevronDown size={14} className="ml-auto text-zinc-500" />
              ) : (
                <ChevronRight size={14} className="ml-auto text-zinc-500" />
              )}
            </button>
            {whenOpen ? (
              <div className="space-y-2 border-t border-zinc-800/70 p-3">
                <label>
                  <span className={labelClass}>Tipo de gatilho</span>
                  <select
                    aria-label="Tipo de gatilho"
                    value={triggerType}
                    onChange={(event) => {
                      setTriggerType(event.target.value);
                      setTriggerConfig({});
                    }}
                    className={inputClass}
                  >
                    {TRIGGER_TYPES.filter((item) => item.value !== "MANUAL").map((item) => (
                      <option key={item.value} value={item.value}>
                        {item.label}
                      </option>
                    ))}
                  </select>
                  <span className="mt-1 block text-[12px] leading-snug text-zinc-500">
                    {triggerDescription(triggerType)}
                  </span>
                </label>

                {triggerType === "CRON" ? (
                  <>
                    <RecurrenceEditor
                      value={recurrence}
                      preview={preview}
                      onChange={setRecurrence}
                    />
                    <label>
                      <span className={labelClass}>Fuso horário</span>
                      <TimezoneSelect
                        value={timezone}
                        onChange={setTimezone}
                        className={inputClass}
                      />
                    </label>
                  </>
                ) : null}

                {(triggerType === "FOLDER_WATCH" ||
                  triggerType === "RSYNC_FOLDER") ? (
                  <label className="block">
                    <span className={labelClass}>Pasta monitorada</span>
                    <PathInput
                      value={String(triggerConfig.path ?? "")}
                      onChange={(value) =>
                        setTriggerConfig((current) => ({ ...current, path: value }))
                      }
                      title="Escolher pasta monitorada"
                      placeholder="input/dropzone ou caminho absoluto"
                      hint="Caminhos relativos são resolvidos dentro do workspace."
                    />
                  </label>
                ) : null}

                {triggerType === "IMAP_IDLE" ? (
                  <div className="grid gap-2 md:grid-cols-2">
                    <label>
                      <span className={labelClass}>Caixa de entrada</span>
                      <input
                        value={String(triggerConfig.mailbox ?? "INBOX")}
                        onChange={(event) =>
                          setTriggerConfig((current) => ({
                            ...current,
                            mailbox: event.target.value,
                          }))
                        }
                        className={inputClass}
                        placeholder="INBOX"
                      />
                    </label>
                    <label>
                      <span className={labelClass}>Filtrar remetente</span>
                      <input
                        value={String(triggerConfig.from_filter ?? "")}
                        onChange={(event) =>
                          setTriggerConfig((current) => ({
                            ...current,
                            from_filter: event.target.value,
                          }))
                        }
                        className={inputClass}
                        placeholder="cliente@exemplo.com"
                      />
                    </label>
                  </div>
                ) : null}

                {triggerType === "SIGNAL_MSG" ? (
                  <label>
                    <span className={labelClass}>Filtrar número (opcional)</span>
                    <input
                      value={String(triggerConfig.from_filter ?? "")}
                      onChange={(event) =>
                        setTriggerConfig((current) => ({
                          ...current,
                          from_filter: event.target.value,
                        }))
                      }
                      className={inputClass}
                      placeholder="+5511999999999"
                    />
                  </label>
                ) : null}

                {triggerType === "WEBHOOK" ? (
                  <label>
                    <span className={labelClass}>Segredo do webhook</span>
                    <input
                      value={String(triggerConfig.secret ?? "")}
                      onChange={(event) =>
                        setTriggerConfig((current) => ({
                          ...current,
                          secret: event.target.value,
                        }))
                      }
                      className={inputClass}
                      placeholder="obrigatório para gatilho WEBHOOK"
                    />
                  </label>
                ) : null}

                {triggerType === "SQL_POLL" ? (
                  <div className="grid gap-2 md:grid-cols-2">
                    <label>
                      <span className={labelClass}>Consulta SQL</span>
                      <input
                        value={String(triggerConfig.query ?? "")}
                        onChange={(event) =>
                          setTriggerConfig((current) => ({
                            ...current,
                            query: event.target.value,
                          }))
                        }
                        className={cn(inputClass, "font-mono text-[13px]")}
                        placeholder="SELECT ..."
                      />
                    </label>
                    <label>
                      <span className={labelClass}>Intervalo (segundos)</span>
                      <input
                        type="number"
                        min={15}
                        value={Number(triggerConfig.interval_seconds ?? 60)}
                        onChange={(event) =>
                          setTriggerConfig((current) => ({
                            ...current,
                            interval_seconds: Number(event.target.value),
                          }))
                        }
                        className={inputClass}
                      />
                    </label>
                  </div>
                ) : null}
              </div>
            ) : null}
        </div>
  );
}

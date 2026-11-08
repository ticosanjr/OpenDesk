import { CalendarDays, Clock3 } from "lucide-react";
import type { Recurrence, SchedulePreview } from "@/lib/types";
import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";
import {
  smallInputClass as inputClass,
  smallLabelClass as labelClass,
} from "@/lib/ui-classes";

const FREQUENCIES: Array<{ id: Recurrence["frequency"]; label: string }> = [
  { id: "once", label: "Uma vez" },
  { id: "hourly", label: "A cada hora" },
  { id: "daily", label: "Diariamente" },
  { id: "weekly", label: "Semanal" },
  { id: "weekdays", label: "Dias úteis" },
  { id: "monthly", label: "Mensal" },
  { id: "custom", label: "Personalizado" },
];

const WEEKDAYS = [
  { id: 1, label: "Seg" },
  { id: 2, label: "Ter" },
  { id: 3, label: "Qua" },
  { id: 4, label: "Qui" },
  { id: 5, label: "Sex" },
  { id: 6, label: "Sáb" },
  { id: 7, label: "Dom" },
];

export function RecurrenceEditor({
  value,
  preview,
  onChange,
}: {
  value: Recurrence;
  preview: SchedulePreview | null;
  onChange: (value: Recurrence) => void;
}) {
  function patch(part: Partial<Recurrence>) {
    onChange({ ...value, ...part });
  }

  function toggleWeekday(day: number) {
    const weekdays = value.weekdays.includes(day)
      ? value.weekdays.filter((item) => item !== day)
      : [...value.weekdays, day];
    patch({ weekdays });
  }

  return (
    <div className="space-y-3 rounded-xl border border-zinc-800/70 bg-zinc-950/30 p-3">
      <div className="flex flex-wrap gap-1">
        {FREQUENCIES.map((frequency) => (
          <button
            key={frequency.id}
            type="button"
            onClick={() => patch({ frequency: frequency.id })}
            className={cn(
              "rounded-lg border px-2.5 py-1 text-[10px]",
              value.frequency === frequency.id
                ? "border-blue-500/50 bg-blue-500/10 text-blue-200"
                : "border-zinc-800 text-zinc-500 hover:text-zinc-300",
            )}
          >
            {frequency.label}
          </button>
        ))}
      </div>

      <div className="grid gap-2 sm:grid-cols-3">
        {value.frequency === "once" ? (
          <label>
            <span className={labelClass}>
              <CalendarDays size={10} className="mr-1 inline" />
              Data
            </span>
            <input
              type="date"
              value={value.start_date ?? ""}
              onChange={(event) => patch({ start_date: event.target.value })}
              className={inputClass}
            />
          </label>
        ) : null}
        <label>
          <span className={labelClass}>
            <Clock3 size={10} className="mr-1 inline" />
            Horário
          </span>
          <input
            type="time"
            value={value.time}
            onChange={(event) => patch({ time: event.target.value })}
            className={inputClass}
          />
        </label>
        {value.frequency === "hourly" ? (
          <label>
            <span className={labelClass}>Intervalo (horas)</span>
            <select
              value={value.interval}
              onChange={(event) => patch({ interval: Number(event.target.value) })}
              className={inputClass}
            >
              {[1, 2, 3, 4, 6, 8, 12].map((interval) => (
                <option key={interval} value={interval}>
                  {interval}h
                </option>
              ))}
            </select>
          </label>
        ) : null}
        {value.frequency === "monthly" ? (
          <label>
            <span className={labelClass}>Dia do mês</span>
            <input
              type="number"
              min={1}
              max={31}
              value={value.day_of_month ?? 1}
              onChange={(event) =>
                patch({ day_of_month: Number(event.target.value) })
              }
              className={inputClass}
            />
          </label>
        ) : null}
        {value.frequency !== "once" && value.frequency !== "custom" ? (
          <label>
            <span className={labelClass}>Repetir até (opcional)</span>
            <input
              type="date"
              value={value.end_date ?? ""}
              onChange={(event) => patch({ end_date: event.target.value })}
              className={inputClass}
            />
          </label>
        ) : null}
      </div>

      {value.frequency === "weekly" ? (
        <div className="flex flex-wrap gap-1">
          {WEEKDAYS.map((day) => (
            <button
              key={day.id}
              type="button"
              onClick={() => toggleWeekday(day.id)}
              className={cn(
                "h-7 min-w-9 rounded-md border px-2 text-[10px]",
                value.weekdays.includes(day.id)
                  ? "border-emerald-500/50 bg-emerald-500/10 text-emerald-200"
                  : "border-zinc-800 text-zinc-500",
              )}
            >
              {day.label}
            </button>
          ))}
        </div>
      ) : null}

      {value.frequency === "custom" ? (
        <label>
          <span className={labelClass}>Expressão cron (avançado)</span>
          <input
            value={value.cron_expression ?? ""}
            onChange={(event) => patch({ cron_expression: event.target.value })}
            placeholder="0 8 * * 1-5"
            className={cn(inputClass, "font-mono")}
          />
        </label>
      ) : null}

      <div className="rounded-lg border border-zinc-800/70 bg-zinc-900/30 px-3 py-2">
        <div className="flex flex-wrap items-center gap-2">
          <Badge variant="blue">{preview?.human ?? "calculando…"}</Badge>
          <span className="font-mono text-[9px] text-zinc-600">
            {preview?.cron_expression ?? ""}
          </span>
        </div>
        {preview && preview.next_runs.length > 0 ? (
          <p className="mt-1 text-[9px] text-zinc-500">
            próximas:{" "}
            {preview.next_runs
              .slice(0, 3)
              .map((run) => new Date(run).toLocaleString("pt-BR"))
              .join(" · ")}
          </p>
        ) : null}
      </div>
    </div>
  );
}


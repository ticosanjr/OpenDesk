import type { Recurrence } from "./types";

const WEEKDAY_NAMES = ["seg", "ter", "qua", "qui", "sex", "sáb", "dom"];

const FREQUENCY_NAMES: Record<Recurrence["frequency"], string> = {
  once: "uma vez",
  hourly: "a cada hora",
  daily: "diariamente",
  weekly: "semanal",
  weekdays: "dias úteis",
  monthly: "mensal",
  custom: "personalizado",
};

export function parseRecurrence(value?: string | null): Recurrence | null {
  if (!value) return null;
  try {
    const parsed = JSON.parse(value) as Partial<Recurrence> | null;
    if (!parsed || typeof parsed.frequency !== "string") return null;
    if (!(parsed.frequency in FREQUENCY_NAMES)) return null;
    return {
      frequency: parsed.frequency as Recurrence["frequency"],
      interval: parsed.interval ?? 1,
      time: parsed.time ?? "08:00",
      weekdays: Array.isArray(parsed.weekdays) ? parsed.weekdays : [],
      day_of_month: parsed.day_of_month ?? null,
      start_date: parsed.start_date ?? null,
      end_date: parsed.end_date ?? null,
      cron_expression: parsed.cron_expression ?? null,
    };
  } catch {
    return null;
  }
}

export function describeRecurrence(
  recurrence: Recurrence | null,
  cron: string,
): string {
  if (!recurrence) return cron;
  const time = recurrence.time || "08:00";
  let description: string;
  switch (recurrence.frequency) {
    case "once":
      description = `uma vez em ${formatDate(recurrence.start_date)} às ${time}`;
      break;
    case "hourly":
      description =
        recurrence.interval > 1
          ? `a cada ${recurrence.interval} horas`
          : "a cada hora";
      break;
    case "daily":
      description = `diariamente às ${time}`;
      break;
    case "weekly": {
      const days = recurrence.weekdays
        .map((day) => WEEKDAY_NAMES[day - 1])
        .filter(Boolean)
        .join(", ");
      description = days ? `${days} às ${time}` : `semanalmente às ${time}`;
      break;
    }
    case "weekdays":
      description = `dias úteis às ${time}`;
      break;
    case "monthly":
      description = `todo dia ${recurrence.day_of_month ?? 1} do mês às ${time}`;
      break;
    case "custom":
      description = recurrence.cron_expression || cron;
      break;
    default:
      description = FREQUENCY_NAMES[recurrence.frequency] ?? cron;
  }
  if (recurrence.end_date) {
    description += ` até ${formatDate(recurrence.end_date)}`;
  }
  return description;
}

function formatDate(value?: string | null): string {
  if (!value) return "data não definida";
  const [year, month, day] = value.split("-");
  if (!year || !month || !day) return value;
  return `${day}/${month}/${year}`;
}

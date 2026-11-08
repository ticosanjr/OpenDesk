export interface TriggerTypeInfo {
  value: string;
  label: string;
  description: string;
  /** Whether this trigger is time-based and needs a cron/visual recurrence. */
  cron?: boolean;
}

/** Shared catalogue so recipes and routines describe triggers identically. */
export const TRIGGER_TYPES: TriggerTypeInfo[] = [
  {
    value: "MANUAL",
    label: "Manual (MANUAL)",
    description: "Só executa quando você clica em Executar ou por uma rotina.",
  },
  {
    value: "CRON",
    label: "Horário programado (CRON)",
    description: "Executa em horários definidos, como todos os dias às 8h.",
    cron: true,
  },
  {
    value: "FOLDER_WATCH",
    label: "Pasta monitorada (FOLDER_WATCH)",
    description:
      "Dispara quando um arquivo novo aparece em uma pasta do workspace.",
  },
  {
    value: "RSYNC_FOLDER",
    label: "Pasta remota via Rsync (RSYNC_FOLDER)",
    description: "Monitora uma pasta sincronizada por Rsync.",
  },
  {
    value: "IMAP_IDLE",
    label: "E-mail recebido (IMAP_IDLE)",
    description: "Dispara quando chega um e-mail que combina com o filtro.",
  },
  {
    value: "SIGNAL_MSG",
    label: "Mensagem no Signal (SIGNAL_MSG)",
    description: "Dispara quando uma mensagem chega pelo bridge do Signal.",
  },
  {
    value: "WEBHOOK",
    label: "Webhook ou formulário (WEBHOOK)",
    description: "Dispara por uma chamada HTTP externa autenticada.",
  },
  {
    value: "CALENDAR_EVENT",
    label: "Evento de calendário (CALENDAR_EVENT)",
    description:
      "Dispara quando um evento começa e combina com o calendário e o título. Use {{trigger.calendar_event.title}} no prompt.",
  },
  {
    value: "SQL_POLL",
    label: "Consulta SQL periódica (SQL_POLL)",
    description: "Consultas SQL em intervalos e cria uma execução por novidade.",
  },
];

export function triggerLabel(value: string): string {
  return TRIGGER_TYPES.find((item) => item.value === value)?.label ?? value;
}

export function triggerDescription(value: string): string {
  return (
    TRIGGER_TYPES.find((item) => item.value === value)?.description ??
    "Gatilho de entrada."
  );
}

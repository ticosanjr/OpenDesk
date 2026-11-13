import { format } from "date-fns";
import { Badge } from "@/components/ui/badge";
import { runStatusMeta, RUN_STATUS_ICON_CLASS } from "@/components/ui/run-status";
import { itemTimeLabel } from "@/lib/calendar";
import type { CalendarItem } from "@/lib/types";
import { cn } from "@/lib/utils";

const KIND_LABEL: Record<string, string> = {
  event: "Evento",
  routine: "Rotina",
  recipe: "Receita",
  run: "Execução",
  recipe_run: "Receita",
  queued: "Fila",
};

function kindLabel(kind: string): string {
  return KIND_LABEL[kind] ?? kind;
}

function statusClass(item: CalendarItem, ringActive: boolean): string | undefined {
  if (item.status === "PAST" && !["MISSED", "FAILED", "BLOCKED"].includes(
    item.execution_status ?? "",
  )) {
    return "opacity-60";
  }
  if (ringActive && item.status === "ACTIVE") return "ring-1 ring-blue-500/40";
  return undefined;
}

function executionMeta(status?: string | null) {
  const normalized = (status ?? "").toUpperCase();
  if (!normalized) return null;
  const meta = runStatusMeta(normalized);
  const label =
    normalized === "SUCCESS" || normalized === "DONE"
      ? "executado"
      : normalized === "FAILED" || normalized === "BLOCKED"
        ? "falhou"
        : normalized === "CANCELLED"
          ? "cancelada"
          : normalized === "MISSED"
            ? "perdida"
            : normalized === "PENDING"
              ? "agendada"
              : meta.label;
  return { ...meta, label };
}

/**
 * Item rendered inside a day cell. `detailed` adds the kind line used by the
 * week/seven-day columns; the month grid stays single-line.
 */
export function CalendarItemChip({
  item,
  onOpen,
  detailed = false,
}: {
  item: CalendarItem;
  onOpen: (item: CalendarItem) => void;
  detailed?: boolean;
}) {
  const color = item.color ?? "#3b82f6";
  return (
    <button
      type="button"
      onClick={(event) => {
        event.stopPropagation();
        onOpen(item);
      }}
      title={`${itemTimeLabel(item)} · ${item.title}`}
      className={cn(
        "block w-full rounded border-l text-left leading-tight",
        detailed
          ? "px-2 py-1 text-[11.5px]"
          : "truncate px-1.5 py-0.5 text-[13px]",
        statusClass(item, true),
      )}
      style={{
        borderLeftColor: color,
        backgroundColor: `${color}18`,
      }}
    >
      {detailed ? (
        <>
          <span className="block truncate text-zinc-200">
            {item.all_day
              ? item.title
              : `${format(new Date(item.start), "HH:mm")} ${item.title}`}
          </span>
          <span className="flex items-center gap-1 truncate text-[11px] text-zinc-500">
            {(() => {
              const meta = executionMeta(item.execution_status);
              return meta ? (
                <meta.icon
                  size={10}
                  className={RUN_STATUS_ICON_CLASS[meta.tone]}
                />
              ) : null;
            })()}
            {kindLabel(item.kind)}
            {item.execution_status ? ` · ${executionMeta(item.execution_status)?.label ?? item.execution_status}` : ""}
          </span>
        </>
      ) : (
        <>
          {item.all_day ? "" : `${format(new Date(item.start), "HH:mm")} `}
          {item.title}
        </>
      )}
    </button>
  );
}

/**
 * Full-width agenda row shared by the month side panel and the day view.
 */
export function CalendarItemRow({
  item,
  onOpen,
  size = "sm",
}: {
  item: CalendarItem;
  onOpen: (item: CalendarItem) => void;
  size?: "sm" | "md";
}) {
  const color = item.color ?? "#3b82f6";
  return (
    <button
      type="button"
      onClick={() => onOpen(item)}
      className={cn(
        "flex w-full items-start rounded-lg border border-zinc-800/70 text-left transition-colors hover:border-zinc-700",
        statusClass(item, false),
        size === "md" ? "gap-3 px-3 py-2.5" : "gap-2 px-2.5 py-2",
      )}
    >
      <span
        aria-hidden
        className="mt-1 h-2 w-2 shrink-0 rounded-full"
        style={{ backgroundColor: color }}
      />
      <span className="min-w-0 flex-1">
        <span
          className={cn(
            "flex items-center gap-1.5",
            size === "md" && "flex-wrap",
          )}
        >
          <span
            className={cn(
              "truncate text-zinc-200",
              size === "md" ? "text-[13.5px]" : "text-[13px]",
            )}
          >
            {item.title}
          </span>
          <Badge variant={item.status === "ACTIVE" ? "blue" : "default"}>
            {kindLabel(item.kind)}
          </Badge>
          {executionMeta(item.execution_status) ? (
            <Badge variant={executionMeta(item.execution_status)!.tone}>
              {executionMeta(item.execution_status)!.label}
            </Badge>
          ) : null}
        </span>
        <span
          className={cn(
            "mt-0.5 block truncate font-mono text-zinc-500",
            size === "md" ? "text-[12px]" : "text-[11.5px]",
          )}
        >
          {itemTimeLabel(item)}
          {item.subtitle
            ? ` · ${item.subtitle
                .replace(/RUNNING_STEP/g, "executando etapa")
                .replace(/AWAITING_APPROVAL/g, "aguardando aprovação")
                .replace(/AWAITING_INPUT/g, "aguardando resposta")}`
            : ""}
        </span>
      </span>
    </button>
  );
}

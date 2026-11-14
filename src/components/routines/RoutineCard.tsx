import { useEffect, useRef, useState } from "react";
import {
  CalendarClock,
  Copy,
  FlaskConical,
  History,
  Mail,
  MoreHorizontal,
  Pencil,
  Play,
  Send,
  Trash2,
} from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EntityLink } from "@/components/ui/entity-link";
import { Switch } from "@/components/ui/switch";
import { Tooltip } from "@/components/ui/tooltip";
import { IconLabel } from "@/components/ui/icon-label";
import { links } from "@/lib/links";
import { relativeTime } from "@/lib/format";
import { describeRecurrence, parseRecurrence } from "@/lib/recurrence";
import { ROUTER_POLICY_LABELS, type Schedule } from "@/lib/types";
import { triggerLabel } from "@/lib/trigger-types";
import { cn } from "@/lib/utils";

export function RoutineCard({
  schedule,
  onToggle,
  onHistory,
  onDuplicate,
  onDryRun,
  onTestNotification,
  onEdit,
  onRun,
  onRemove,
}: {
  schedule: Schedule;
  onToggle: () => void;
  onHistory: () => void;
  onDuplicate: () => void;
  onDryRun: () => void;
  onTestNotification: () => void;
  onEdit: () => void;
  onRun: () => void;
  onRemove: () => void;
}) {
  const [menuOpen, setMenuOpen] = useState(false);
  const menuRef = useRef<HTMLDivElement | null>(null);
  const recurrences =
    schedule.trigger_type === "CRON"
      ? describeRecurrence(
          parseRecurrence(schedule.recurrence_json),
          schedule.cron_expression,
        )
      : null;
  const meta = [
    <Badge
      key="trigger"
      variant="blue"
      title={`Gatilho: ${triggerLabel(schedule.trigger_type ?? "CRON")}`}
    >
      {triggerLabel(schedule.trigger_type ?? "CRON")}
    </Badge>,
    schedule.trigger_type === "CRON" ? (
      <Badge
        key="recurrence"
        variant="default"
        title={`Recorrência: ${recurrences}`}
      >
        {recurrences}
      </Badge>
    ) : null,
    <Badge
      key="target"
      variant={schedule.target_kind === "RECIPE" ? "purple" : "default"}
      title={
        schedule.target_kind === "RECIPE"
          ? `Executa a receita ${schedule.recipe_id ?? "não definida"}`
          : "Executa um prompt livre"
      }
    >
      {schedule.target_kind === "RECIPE"
        ? `receita: ${schedule.recipe_id ?? "—"}`
        : "prompt"}
    </Badge>,
    schedule.notify_only_on_change ? (
      <Badge key="novelty" variant="green">
        só novidades
      </Badge>
    ) : null,
    schedule.notify_signal ? (
      <Badge key="signal" variant="green">
        Signal
      </Badge>
    ) : null,
    schedule.notify_telegram ? (
      <Badge key="telegram" variant="green">
        Telegram
      </Badge>
    ) : null,
    schedule.one_time ? (
      <Badge key="once" variant="purple">
        uma vez
      </Badge>
    ) : null,
    schedule.agent_name ? (
      <EntityLink
        key="agent"
        to={links.agent(schedule.agent_name, "workspace", schedule.workspace_id)}
        chip
        className="max-w-[88px]"
      >
        @{schedule.agent_name}
      </EntityLink>
    ) : null,
    <Badge
      key="policy"
      variant="default"
      title="Política do roteador aplicada a esta rotina"
    >
      {ROUTER_POLICY_LABELS[schedule.router_policy] ?? schedule.router_policy}
    </Badge>,
    schedule.trigger_type === "CRON" ? (
      <Badge
        key="timezone"
        variant="default"
        title="Fuso horário usado para calcular o próximo horário"
      >
        {schedule.timezone}
      </Badge>
    ) : null,
  ].filter(Boolean) as React.ReactNode[];

  function action(callback: () => void) {
    setMenuOpen(false);
    callback();
  }

  useEffect(() => {
    if (!menuOpen) return;
    const closeOnOutside = (event: PointerEvent) => {
      if (!menuRef.current?.contains(event.target as Node)) {
        setMenuOpen(false);
      }
    };
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") setMenuOpen(false);
    };
    document.addEventListener("pointerdown", closeOnOutside);
    document.addEventListener("keydown", closeOnEscape);
    return () => {
      document.removeEventListener("pointerdown", closeOnOutside);
      document.removeEventListener("keydown", closeOnEscape);
    };
  }, [menuOpen]);

  return (
    <div
      data-card="routine"
      className={cn(
        "relative flex min-h-[188px] flex-col rounded-xl border px-3.5 py-3",
        schedule.is_active
          ? "border-zinc-800/70 bg-zinc-950/40"
          : "border-zinc-800/50 bg-zinc-950/20",
      )}
    >
      <div className="flex min-h-[24px] items-start justify-between gap-2">
        <div className="flex min-w-0 flex-wrap items-center gap-2">
          <p className="line-clamp-1 min-w-0 text-[14px] font-semibold text-zinc-100">
            {schedule.name}
          </p>
          <Badge variant={schedule.is_active ? "green" : "default"}>
            {schedule.is_active ? "ativa" : "pausada"}
          </Badge>
        </div>
        <div className="flex shrink-0 items-center gap-1.5">
          <Switch
            checked={schedule.is_active}
            onCheckedChange={onToggle}
            label={
              schedule.is_active
                ? `Desativar rotina ${schedule.name}`
                : `Ativar rotina ${schedule.name}`
            }
          />

        </div>
      </div>

      <div className="mt-1.5 flex min-h-5 items-center gap-1.5 overflow-hidden">
        {meta.slice(0, 4)}
        {meta.length > 4 ? (
          <Badge variant="default" title={`${meta.length - 4} metadados ocultos`}>
            +{meta.length - 4}
          </Badge>
        ) : null}
      </div>

      <p className="mt-1 line-clamp-2 min-h-[34px] text-[13.5px] leading-relaxed text-zinc-400">
        {schedule.target_kind === "RECIPE"
          ? `Executa a receita ${schedule.recipe_id ?? "(não definida)"}`
          : schedule.command_or_prompt || "—"}
      </p>

      <div className="mt-1 flex min-h-4 flex-wrap items-center gap-x-3 gap-y-1 overflow-hidden text-[11.5px] text-zinc-400">
        <span className="inline-flex min-w-0 items-center gap-1.5">
          <CalendarClock size={12} className="shrink-0 text-zinc-500" />
          <span className="truncate">
            {schedule.next_run_at
              ? `próxima ${new Date(schedule.next_run_at).toLocaleString("pt-BR")}`
              : "sem próxima execução"}
          </span>
        </span>
        <span
          className="inline-flex min-w-0 items-center gap-1.5"
          title={
            schedule.last_run_at
              ? `Última execução: ${schedule.last_run_status ?? "sem status"}`
              : "Ainda não executou"
          }
        >
          <History size={12} className="shrink-0 text-zinc-500" />
          <span className="truncate">
            {schedule.last_run_at
              ? `${relativeTime(schedule.last_run_at)} · ${schedule.last_run_status ?? "—"}`
              : "nunca executou"}
          </span>
        </span>
        {schedule.notify_email_recipients ? (
          <Tooltip content={schedule.notify_email_recipients}>
            <span className="flex items-center gap-1 text-zinc-500">
              <Mail size={11} />
              e-mail
            </span>
          </Tooltip>
        ) : null}
      </div>

      <div className="mt-auto flex min-h-9 items-center justify-end gap-1.5 border-t border-zinc-800/60 pt-2">
        <Button size="xs" onClick={onRun}>
          <IconLabel icon={<Play size={13} />} gap="tight">
            Executar agora
          </IconLabel>
        </Button>
        <Button
          variant="outline"
          size="xs"
          onClick={onEdit}
        >
          <IconLabel icon={<Pencil size={13} />} gap="tight">
            Editar
          </IconLabel>
        </Button>
        <div ref={menuRef} className="relative">
          <Button
            variant="ghost"
            size="xs"
            className="w-7 px-0"
            aria-label={`Mais ações da rotina ${schedule.name}`}
            aria-expanded={menuOpen}
            onClick={() => setMenuOpen((value) => !value)}
          >
            <MoreHorizontal size={15} />
          </Button>

          {menuOpen ? (
            <div className="absolute right-0 top-9 z-40 w-52 rounded-lg border border-zinc-700 bg-zinc-950 p-1 shadow-2xl">
              <MenuItem icon={<History size={14} />} onClick={() => action(onHistory)}>
                Histórico
              </MenuItem>
              <MenuItem icon={<Copy size={14} />} onClick={() => action(onDuplicate)}>
                Duplicar
              </MenuItem>
              <MenuItem icon={<FlaskConical size={14} />} onClick={() => action(onDryRun)}>
                Dry-run
              </MenuItem>
              {schedule.notify_email_recipients ||
              schedule.notify_signal ||
              schedule.notify_telegram ? (
                <MenuItem icon={<Send size={14} />} onClick={() => action(onTestNotification)}>
                  Testar aviso
                </MenuItem>
              ) : null}
              <MenuItem
                icon={<Trash2 size={14} />}
                destructive
                onClick={() => action(onRemove)}
              >
                Excluir
              </MenuItem>
            </div>
          ) : null}
        </div>
      </div>
    </div>
  );
}

function MenuItem({
  icon,
  children,
  onClick,
  destructive = false,
}: {
  icon: React.ReactNode;
  children: React.ReactNode;
  onClick: () => void;
  destructive?: boolean;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        "flex w-full items-center gap-2 rounded-md px-2.5 py-2 text-left text-[13.5px] hover:bg-zinc-800",
        destructive ? "text-red-300" : "text-zinc-300",
      )}
    >
      {icon}
      {children}
    </button>
  );
}

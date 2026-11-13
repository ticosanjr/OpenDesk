import { useCallback, useEffect, useMemo, useState } from "react";

import { Link } from "react-router-dom";
import {
  CalendarClock,
  Clock3,
  Mail,
  Play,
  Plus,
  RefreshCw,
  Timer,
} from "lucide-react";
import { PageBody, PageHeader } from "@/components/layout/PageHeader";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Modal } from "@/components/ui/modal";
import { EmptyState } from "@/components/ui/empty-state";
import { LoadingState } from "@/components/ui/loading-state";
import { useAppStore } from "@/store/app-store";
import { useUiStore } from "@/store/ui-store";
import { apiDelete, apiGet, apiPost } from "@/lib/api";

import type {
  Schedule,
  TaskRun,
} from "@/lib/types";
import { relativeTime } from "@/lib/format";
import { confirmAction } from "@/store/confirm-store";
import { toast } from "@/store/toast-store";
import { useFocusParam } from "@/lib/use-focus-param";
import { links } from "@/lib/links";
import { RoutineCard } from "@/components/routines/RoutineCard";
import { runStatusLabel } from "@/lib/task-ui";
import { CreateScheduleModal } from "@/components/routines/RoutineFormModal";

export function RoutinesPage() {
  const activeWorkspaceId = useAppStore((state) => state.activeWorkspaceId);
  const workspaces = useAppStore((state) => state.workspaces);
  const lastScheduleEvent = useAppStore((state) => state.lastScheduleEvent);
  const setWorkspaceDialogOpen = useUiStore(
    (state) => state.setWorkspaceDialogOpen,
  );

  const [schedules, setSchedules] = useState<Schedule[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [createOpen, setCreateOpen] = useState(false);
  const [editing, setEditing] = useState<Schedule | null>(null);
  const [historySchedule, setHistorySchedule] = useState<Schedule | null>(null);
  const [historyRuns, setHistoryRuns] = useState<TaskRun[]>([]);
  const [historyLoading, setHistoryLoading] = useState(false);
  useFocusParam("schedule");

  const workspace = workspaces.find((item) => item.id === activeWorkspaceId);
  const query = useMemo(
    () => (activeWorkspaceId ? `?workspace_id=${encodeURIComponent(activeWorkspaceId)}` : ""),
    [activeWorkspaceId],
  );

  const load = useCallback(async () => {
    if (!activeWorkspaceId) {
      setSchedules([]);
      return;
    }
    setLoading(true);
    try {
      const items = await apiGet<Schedule[]>(`/api/schedules${query}`);
      setSchedules(items);
      setError(null);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setLoading(false);
    }
  }, [activeWorkspaceId, query]);

  useEffect(() => {
    void load();
  }, [load, lastScheduleEvent?.schedule_id]);

  async function toggle(schedule: Schedule) {
    if (!schedule.is_active) {
      const confirmed = await confirmAction({
        title: `Ativar a rotina “${schedule.name}”?`,
        description:
          "Ela passará a executar automaticamente conforme o gatilho configurado. Você pode pausar a qualquer momento.",
        confirmLabel: "Ativar rotina",
      });
      if (!confirmed) return;
    }
    try {
      await apiPost(`/api/schedules/${schedule.id}/active`, {
        active: !schedule.is_active,
      });
      await load();
      toast({
        tone: "success",
        title: schedule.is_active ? "Rotina pausada" : "Rotina retomada",
        description: schedule.name,
        href: links.schedule(schedule.id),
      });
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  }

  async function duplicate(schedule: Schedule) {
    try {
      const created = await apiPost<Schedule>("/api/schedules", {
        workspace_id: schedule.workspace_id,
        name: `${schedule.name} (cópia)`,
        description: schedule.description,
        cron_expression: schedule.cron_expression,
        timezone: schedule.timezone,
        agent_name: schedule.agent_name,
        command_or_prompt: schedule.command_or_prompt,
        router_policy: schedule.router_policy,
        output_folder_template: schedule.output_folder_template,
        notify_email_recipients: schedule.notify_email_recipients,
        notify_signal: schedule.notify_signal,
        notify_signal_recipients: schedule.notify_signal_recipients,
        notify_only_on_change: schedule.notify_only_on_change,
        recurrence_json: schedule.recurrence_json,
        one_time: schedule.one_time,
        sync_to_google_docs: schedule.sync_to_google_docs,
        sync_to_google_calendar: schedule.sync_to_google_calendar,
        is_active: schedule.is_active,
      });
      await load();
      toast({
        tone: "success",
        title: "Rotina duplicada",
        description: schedule.name,
        href: links.schedule(created.id),
      });
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível duplicar",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    }
  }

  async function testNotification(schedule: Schedule) {
    const channel = schedule.notify_email_recipients ? "email" : "signal";
    try {
      const result = await apiPost<{ sent: number; channel: string }>(
        `/api/schedules/${schedule.id}/notify-test`,
        { channel },
      );
      toast({
        tone: "success",
        title: "Teste enviado",
        description: `${result.sent} destino(s) via ${result.channel}.`,
        href: links.schedule(schedule.id),
      });
    } catch (cause) {
      toast({
        tone: "error",
        title: "Falha no teste de notificação",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    }
  }

  async function dryRun(schedule: Schedule) {
    try {
      const result = await apiPost<{
        ok: boolean;
        checks: Array<{ name: string; ok: boolean; detail: string }>;
      }>(`/api/schedules/${schedule.id}/dry-run`, {});
      const failed = result.checks.filter((check) => !check.ok);
      toast({
        tone: failed.length === 0 ? "success" : "warning",
        title: failed.length === 0 ? "Dry-run validado" : "Dry-run com avisos",
        description:
          failed.length === 0
            ? "Prompt, pasta, engine, SMTP e Google verificados."
            : failed.map((check) => `${check.name}: ${check.detail}`).join(" · "),
        href: links.schedule(schedule.id),
      });
    } catch (cause) {
      toast({
        tone: "error",
        title: "Dry-run falhou",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    }
  }

  async function openHistory(schedule: Schedule) {
    setHistorySchedule(schedule);
    setHistoryRuns([]);
    setHistoryLoading(true);
    try {
      const runsList = await apiGet<TaskRun[]>(`/api/schedules/${schedule.id}/runs`);
      setHistoryRuns(runsList);
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível carregar o histórico",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setHistoryLoading(false);
    }
  }

  async function runNow(schedule: Schedule) {
    try {
      await apiPost(`/api/schedules/${schedule.id}/run`, {});
      await load();
      toast({
        tone: "success",
        title: "Rotina enfileirada",
        description: `${schedule.name} entrará na fila assim que houver um worker livre.`,
        href: links.schedule(schedule.id),
      });
    } catch (cause) {
      const message = cause instanceof Error ? cause.message : String(cause);
      setError(message);
      toast({
        tone: "error",
        title: "Não foi possível executar a rotina",
        description: message,
        href: links.schedule(schedule.id),
      });
    }
  }

  async function remove(schedule: Schedule) {
    if (!(await confirmAction({ title: `Excluir a rotina ${schedule.name}?`, description: "Os runs já executados e artefatos não serão apagados.", confirmLabel: "Excluir rotina", tone: "destructive" }))) return;
    try {
      await apiDelete(`/api/schedules/${schedule.id}`);
      await load();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  }

  if (!activeWorkspaceId) {
    return (
      <PageBody>
        <PageHeader title="Rotinas & Gatilhos" />
        <EmptyState
          icon={CalendarClock}
          title="Nenhum workspace ativo"
          description="Selecione um workspace para gerenciar rotinas."
          action={
            <Button size="sm" onClick={() => setWorkspaceDialogOpen(true)}>
              Criar workspace
            </Button>
          }
        />
      </PageBody>
    );
  }

  return (
    <PageBody>
      <PageHeader
        title="Rotinas & Gatilhos"
        description={`${workspace?.name ?? "Workspace"} · horário, pasta, e-mail, Signal, webhook ou SQL; executa prompt ou receita.`}
        actions={
          <>
            <Button variant="outline" size="sm" onClick={() => void load()}>
              <RefreshCw size={13} className={loading ? "animate-spin" : undefined} />
              Atualizar
            </Button>
            <Button
              size="sm"
              onClick={() => {
                setEditing(null);
                setCreateOpen(true);
              }}
            >
              <Plus size={15} />
              Nova rotina
            </Button>
          </>
        }
      />

      {error ? (
        <p className="rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[13px] text-red-300">
          {error}
        </p>
      ) : null}

      {schedules.length === 0 ? (
      <div className="grid gap-3 md:grid-cols-3">
        {[
          {
            icon: Clock3,
            title: "Agendamento",
            description: "Expressão cron, timezone, próxima execução e recuperação de slots.",
          },
          {
            icon: Play,
            title: "Execução determinística",
            description: "output/{YYYY}-{MM}/{schedule_slug} criado antes de enfileirar.",
          },
          {
            icon: Mail,
            title: "Entrega",
            description: "Resumo e anexos por SMTP, com idempotência em integration_deliveries.",
          },
        ].map((item) => (
          <Card key={item.title}>
            <CardContent className="p-4">
              <div className="flex h-8 w-8 items-center justify-center rounded-lg border border-zinc-800 bg-zinc-900/60 text-zinc-500">
                <item.icon size={15} strokeWidth={1.7} />
              </div>
              <p className="mt-3 text-[13.5px] font-medium text-zinc-200">{item.title}</p>
              <p className="mt-1 text-[13px] leading-relaxed text-zinc-500">{item.description}</p>
            </CardContent>
          </Card>
        ))}
      </div>
      ) : null}

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Timer size={15} className="text-zinc-500" />
            Rotinas do workspace
          </CardTitle>
          <Badge variant="default">{schedules.length} agendadas</Badge>
        </CardHeader>
        <CardContent>
          {schedules.length === 0 ? (
            <EmptyState
              icon={CalendarClock}
              title="Nenhuma rotina configurada"
              description="Crie rotinas por horário, pasta, e-mail, Signal, webhook ou SQL. A rotina pode executar um prompt ou uma receita."
              compact
              action={
                <Button
                  size="sm"
                  onClick={() => {
                    setEditing(null);
                    setCreateOpen(true);
                  }}
                >
                  <Plus size={14} />
                  Criar rotina
                </Button>
              }
            />
          ) : (
            <div className="space-y-2">
              {schedules.map((schedule) => (
                <RoutineCard
                  key={schedule.id}
                  schedule={schedule}
                  onToggle={() => void toggle(schedule)}
                  onHistory={() => void openHistory(schedule)}
                  onDuplicate={() => void duplicate(schedule)}
                  onDryRun={() => void dryRun(schedule)}
                  onTestNotification={() => void testNotification(schedule)}
                  onEdit={() => {
                    setEditing(schedule);
                    setCreateOpen(true);
                  }}
                  onRun={() => void runNow(schedule)}
                  onRemove={() => void remove(schedule)}
                />
              ))}
            </div>
          )}
        </CardContent>
      </Card>

      <Modal
        open={!!historySchedule}
        onClose={() => setHistorySchedule(null)}
        title={`Histórico · ${historySchedule?.name ?? "rotina"}`}
        description="Últimos 100 runs disparados por esta rotina."
        size="lg"
      >
        {historyLoading ? (
          <LoadingState compact label="Carregando histórico…" rows={3} />
        ) : historyRuns.length === 0 ? (
          <p className="rounded-lg border border-dashed border-zinc-800 px-4 py-8 text-center text-[13px] text-zinc-500">
            Nenhum run registrado ainda. Use “Executar agora” ou aguarde o próximo disparo.
          </p>
        ) : (
          <div className="max-h-[55dvh] space-y-1.5 overflow-y-auto pr-1">
            {historyRuns.map((run) => (
              <Link
                key={run.id}
                to={links.run(run.id)}
                className="flex flex-wrap items-center gap-2 rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2.5 text-[13.5px] transition-colors hover:border-blue-500/30 hover:bg-zinc-900/50"
              >
                <Badge
                  variant={
                    run.status === "SUCCESS"
                      ? "green"
                      : run.status === "FAILED"
                        ? "red"
                        : run.status === "RUNNING"
                          ? "blue"
                          : "yellow"
                  }
                >
                  {runStatusLabel(run.status)}
                </Badge>
                <span className="font-mono text-zinc-400">{run.phase ?? "execução"}</span>
                <span className="text-zinc-500">
                  {run.agent_name} · {run.model_id} · {Math.round(run.duration_ms / 1000)}s ·{" "}
                  {run.tokens_input + run.tokens_output} tokens
                </span>
                <span className="ml-auto text-zinc-500">
                  {relativeTime(run.started_at)}
                </span>
                {run.error_message ? (
                  <p className="w-full truncate font-mono text-[13px] text-red-300/80">
                    {run.error_message}
                  </p>
                ) : null}
              </Link>
            ))}
          </div>
        )}
        <div className="mt-4 flex justify-end border-t border-zinc-800/70 pt-4">
          <Button variant="ghost" size="sm" onClick={() => setHistorySchedule(null)}>
            Fechar
          </Button>
        </div>
      </Modal>

      <CreateScheduleModal
        open={createOpen}
        workspaceId={activeWorkspaceId}
        schedule={editing}
        onClose={() => {
          setCreateOpen(false);
          setEditing(null);
        }}
        onCreated={async () => {
          setCreateOpen(false);
          setEditing(null);
          await load();
        }}
      />
    </PageBody>
  );
}

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Link } from "react-router-dom";
import {
  CalendarClock,
  ChevronDown,
  ChevronRight,
  Clock3,
  Loader2,
  Mail,
  Pause,
  Play,
  Plus,
  RefreshCw,
  Timer,
  Trash2,
} from "lucide-react";
import { PageBody, PageHeader } from "@/components/layout/PageHeader";
import type { OutboundSink } from "@/components/integrations/OutboundSinksCard";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Modal } from "@/components/ui/modal";
import { EmptyState } from "@/components/ui/empty-state";
import { useAppStore } from "@/store/app-store";
import { useUiStore } from "@/store/ui-store";
import { apiDelete, apiGet, apiPatch, apiPost } from "@/lib/api";
import type {
  AgentFile,
  Recurrence,
  Schedule,
  SchedulePreview,
  StudioRuntime,
  TaskRun,
} from "@/lib/types";
import { RecurrenceEditor } from "@/components/routines/RecurrenceEditor";
import { describeRecurrence, parseRecurrence } from "@/lib/recurrence";
import { ROUTER_POLICY_LABELS } from "@/lib/types";
import { relativeTime } from "@/lib/format";
import { cn } from "@/lib/utils";
import { confirmAction } from "@/store/confirm-store";
import { toast } from "@/store/toast-store";
import { useFocusParam } from "@/lib/use-focus-param";
import { links } from "@/lib/links";
import { EntityLink } from "@/components/ui/entity-link";
import { FieldHint } from "@/components/ui/field-hint";
import { IconLabel } from "@/components/ui/icon-label";
import { PathInput } from "@/components/ui/path-input";
import { TRIGGER_TYPES, triggerDescription, triggerLabel } from "@/lib/trigger-types";

interface RecipeOption {
  id: string;
  name: string;
}

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
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
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
        <PageHeader eyebrow="automação recorrente" title="Rotinas (Cron)" />
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
        eyebrow="automação recorrente"
        title="Rotinas (Cron)"
        description={`${workspace?.name ?? "Workspace"} · agendamento com pasta determinística, fila, e-mail e histórico.`}
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
        <p className="rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[11px] text-red-300">
          {error}
        </p>
      ) : null}

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
              <p className="mt-3 text-[12px] font-medium text-zinc-200">{item.title}</p>
              <p className="mt-1 text-[11px] leading-relaxed text-zinc-500">{item.description}</p>
            </CardContent>
          </Card>
        ))}
      </div>

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
                <div
                  key={schedule.id}
                  data-focus-id={schedule.id}
                  className={cn(
                    "rounded-xl border px-4 py-3",
                    schedule.is_active
                      ? "border-zinc-800/70 bg-zinc-950/40"
                      : "border-zinc-800/50 bg-zinc-950/20 opacity-70",
                  )}
                >
                  <div className="flex flex-wrap items-start justify-between gap-3">
                    <div className="min-w-0">
                      <div className="flex flex-wrap items-center gap-2">
                        <p className="text-[12px] font-medium text-zinc-100">{schedule.name}</p>
                        <Badge variant={schedule.is_active ? "green" : "default"}>
                          {schedule.is_active ? "ativa" : "pausada"}
                        </Badge>
                        <Badge variant="blue">
                          {triggerLabel(schedule.trigger_type ?? "CRON")}
                        </Badge>
                        {schedule.trigger_type === "CRON" ? (
                          <>
                            <Badge variant="default">{schedule.timezone}</Badge>
                            <Badge variant="default">
                              {describeRecurrence(
                                parseRecurrence(schedule.recurrence_json),
                                schedule.cron_expression,
                              )}
                            </Badge>
                          </>
                        ) : null}
                        <Badge
                          variant={schedule.target_kind === "RECIPE" ? "purple" : "default"}
                        >
                          {schedule.target_kind === "RECIPE"
                            ? `receita: ${schedule.recipe_id ?? "—"}`
                            : "prompt"}
                        </Badge>
                        {schedule.one_time ? (
                          <Badge variant="purple">uma vez</Badge>
                        ) : null}
                        {schedule.notify_only_on_change ? (
                          <Badge variant="green">só novidades</Badge>
                        ) : null}
                        {schedule.notify_signal ? (
                          <Badge variant="green">Signal</Badge>
                        ) : null}
                        {schedule.agent_name ? (
                          <EntityLink
                            to={links.agent(
                              schedule.agent_name,
                              "workspace",
                              schedule.workspace_id,
                            )}
                            chip
                          >
                            @{schedule.agent_name}
                          </EntityLink>
                        ) : null}
                        <Badge variant="default">
                          {ROUTER_POLICY_LABELS[schedule.router_policy] ?? schedule.router_policy}
                        </Badge>
                      </div>
                      <p className="mt-1 truncate font-mono text-[10px] text-zinc-500">
                        {schedule.target_kind === "RECIPE"
                          ? `Executa a receita ${schedule.recipe_id ?? "(não definida)"}`
                          : schedule.command_or_prompt}
                      </p>
                      <p className="mt-1 font-mono text-[9px] text-zinc-600">
                        {schedule.output_folder_template}
                      </p>
                      <div className="mt-2 flex flex-wrap items-center gap-3 font-mono text-[9px] text-zinc-600">
                        <span>
                          próxima:{" "}
                          {schedule.next_run_at
                            ? new Date(schedule.next_run_at).toLocaleString("pt-BR")
                            : "—"}
                        </span>
                        <span>
                          última:{" "}
                          {schedule.last_run_at
                            ? `${relativeTime(schedule.last_run_at)} (${schedule.last_run_status ?? "—"})`
                            : "—"}
                        </span>
                        {schedule.notify_email_recipients ? (
                          <span className="flex items-center gap-1 text-zinc-500">
                            <Mail size={10} />
                            {schedule.notify_email_recipients}
                          </span>
                        ) : null}
                      </div>
                    </div>
                    <div className="flex shrink-0 flex-wrap gap-2">
                      <Button
                        variant="outline"
                        size="sm"
                        className="h-7"
                        onClick={() => void openHistory(schedule)}
                      >
                        Histórico
                      </Button>
                      <Button
                        variant="outline"
                        size="sm"
                        className="h-7"
                        onClick={() => void duplicate(schedule)}
                      >
                        Duplicar
                      </Button>
                      <Button
                        variant="outline"
                        size="sm"
                        className="h-7"
                        onClick={() => void dryRun(schedule)}
                        title="Validar prompt, pasta, engine, SMTP e Google sem executar"
                      >
                        Dry-run
                      </Button>
                      {schedule.notify_email_recipients || schedule.notify_signal ? (
                        <Button
                          variant="outline"
                          size="sm"
                          className="h-7"
                          onClick={() => void testNotification(schedule)}
                        >
                          Testar aviso
                        </Button>
                      ) : null}
                      <Button
                        variant="outline"
                        size="sm"
                        className="h-7"
                        onClick={() => {
                          setEditing(schedule);
                          setCreateOpen(true);
                        }}
                      >
                        Editar
                      </Button>
                      <Button
                        variant="outline"
                        size="sm"
                        className="h-7"
                        onClick={() => void runNow(schedule)}
                      >
                        <Play size={12} />
                        Executar agora
                      </Button>
                      <Button
                        variant="outline"
                        size="sm"
                        className="h-7"
                        onClick={() => void toggle(schedule)}
                      >
                        {schedule.is_active ? <Pause size={12} /> : <Play size={12} />}
                        {schedule.is_active ? "Pausar" : "Retomar"}
                      </Button>
                      <Button
                        variant="destructive"
                        size="sm"
                        className="h-7"
                        onClick={() => void remove(schedule)}
                      >
                        <Trash2 size={12} />
                      </Button>
                    </div>
                  </div>
                </div>
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
        className="max-w-2xl"
      >
        {historyLoading ? (
          <p className="text-[11px] text-zinc-500">Carregando histórico…</p>
        ) : historyRuns.length === 0 ? (
          <p className="rounded-lg border border-dashed border-zinc-800 px-4 py-8 text-center text-[11px] text-zinc-600">
            Nenhum run registrado ainda. Use “Executar agora” ou aguarde o próximo disparo.
          </p>
        ) : (
          <div className="max-h-[55dvh] space-y-1.5 overflow-y-auto pr-1">
            {historyRuns.map((run) => (
              <Link
                key={run.id}
                to={links.run(run.id)}
                className="flex flex-wrap items-center gap-2 rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2 text-[10px] transition-colors hover:border-blue-500/30 hover:bg-zinc-900/50"
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
                  {run.status}
                </Badge>
                <span className="font-mono text-zinc-400">{run.phase ?? "run"}</span>
                <span className="text-zinc-500">
                  {run.agent_name} · {run.model_id} · {Math.round(run.duration_ms / 1000)}s ·{" "}
                  {run.tokens_input + run.tokens_output} tokens
                </span>
                <span className="ml-auto text-zinc-600">
                  {relativeTime(run.started_at)}
                </span>
                {run.error_message ? (
                  <p className="w-full truncate font-mono text-[9px] text-red-300/80">
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

function CreateScheduleModal({
  open,
  workspaceId,
  schedule,
  onClose,
  onCreated,
}: {
  open: boolean;
  workspaceId: string;
  schedule: Schedule | null;
  onClose: () => void;
  onCreated: () => Promise<void>;
}) {
  const workspaces = useAppStore((state) => state.workspaces);
  const workspace = workspaces.find((item) => item.id === workspaceId);
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [triggerType, setTriggerType] = useState("CRON");
  const [triggerConfig, setTriggerConfig] = useState<Record<string, unknown>>({});
  const [targetKind, setTargetKind] = useState<"PROMPT" | "RECIPE">("PROMPT");
  const [recipeId, setRecipeId] = useState("");
  const [recipes, setRecipes] = useState<RecipeOption[]>([]);
  const [recipeQuery, setRecipeQuery] = useState("");
  const [recipeOpen, setRecipeOpen] = useState(false);
  const [whenOpen, setWhenOpen] = useState(true);
  const [prompt, setPrompt] = useState("");
  const [agent, setAgent] = useState("");
  const [agentOpen, setAgentOpen] = useState(false);
  const [agents, setAgents] = useState<AgentFile[]>([]);
  const [policy, setPolicy] = useState("COST_BENEFIT");
  const [policyTouched, setPolicyTouched] = useState(false);
  const [template, setTemplate] = useState("output/{YYYY}-{MM}/{schedule_slug}");
  const [recipients, setRecipients] = useState("");
  const [signalEnabled, setSignalEnabled] = useState(false);
  const [signalRecipients, setSignalRecipients] = useState("");
  const [onlyOnChange, setOnlyOnChange] = useState(false);
  const [timezone, setTimezone] = useState(
    Intl.DateTimeFormat().resolvedOptions().timeZone || "UTC",
  );
  const [misfirePolicy, setMisfirePolicy] = useState("SKIP_TO_NEXT");
  const [safeReadonly, setSafeReadonly] = useState(true);
  const [sinks, setSinks] = useState<OutboundSink[]>([]);
  const [selectedSinks, setSelectedSinks] = useState<Set<string>>(new Set());
  const [recurrence, setRecurrence] = useState<Recurrence>(() => ({
    frequency: "daily",
    interval: 1,
    time: "08:00",
    weekdays: [],
    day_of_month: 1,
    start_date: new Date().toISOString().slice(0, 10),
    end_date: null,
    cron_expression: null,
  }));
  const [preview, setPreview] = useState<SchedulePreview | null>(null);
  const [runtime, setRuntime] = useState<StudioRuntime | null>(null);
  const [slash, setSlash] = useState<{ query: string; start: number } | null>(null);
  const promptRef = useRef<HTMLTextAreaElement | null>(null);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    setError(null);
    setName(schedule?.name ?? "");
    setDescription(schedule?.description ?? "");
    setTriggerType(schedule?.trigger_type ?? "CRON");
    try {
      setTriggerConfig(
        schedule?.trigger_config_json
          ? (JSON.parse(schedule.trigger_config_json) as Record<string, unknown>)
          : {},
      );
    } catch {
      setTriggerConfig({});
    }
    setTargetKind(
      schedule?.target_kind === "RECIPE" ? "RECIPE" : "PROMPT",
    );
    setRecipeId(schedule?.recipe_id ?? "");
    setRecipeQuery("");
    setWhenOpen(true);
    setPrompt(schedule?.command_or_prompt ?? "");
    setAgent(schedule?.agent_name ?? "");
    setPolicy(schedule?.router_policy ?? workspace?.default_router_policy ?? "COST_BENEFIT");
    setPolicyTouched(false);
    setTemplate(schedule?.output_folder_template ?? "output/{YYYY}-{MM}/{schedule_slug}");
    setRecipients(schedule?.notify_email_recipients ?? workspace?.default_email_recipient ?? "");
    setSignalEnabled(schedule?.notify_signal ?? false);
    setSignalRecipients(schedule?.notify_signal_recipients ?? "");
    setOnlyOnChange(schedule?.notify_only_on_change ?? false);
    setTimezone(schedule?.timezone ?? Intl.DateTimeFormat().resolvedOptions().timeZone ?? "UTC");
    setMisfirePolicy(schedule?.misfire_policy ?? "SKIP_TO_NEXT");
    setSafeReadonly(schedule?.safe_readonly_mode ?? true);
    if (schedule?.recurrence_json) {
      try {
        setRecurrence(JSON.parse(schedule.recurrence_json) as Recurrence);
      } catch {
        setRecurrence({
          frequency: "custom",
          interval: 1,
          time: "08:00",
          weekdays: [],
          day_of_month: 1,
          start_date: null,
          end_date: null,
          cron_expression: schedule.cron_expression,
        });
      }
    } else if (schedule) {
      setRecurrence({
        frequency: "custom",
        interval: 1,
        time: "08:00",
        weekdays: [],
        day_of_month: 1,
        start_date: null,
        end_date: null,
        cron_expression: schedule.cron_expression,
      });
    }
    const workspaceQuery = `scope=workspace&workspace_id=${encodeURIComponent(workspaceId)}`;
    Promise.all([
      apiGet<AgentFile[]>(`/api/studio/agents?${workspaceQuery}`).catch(() => []),
      apiGet<AgentFile[]>("/api/studio/agents?scope=global").catch(() => []),
      apiGet<StudioRuntime>(
        `/api/studio/runtime?workspace_id=${encodeURIComponent(workspaceId)}`,
      ).catch(() => null),
      apiGet<{ sinks: OutboundSink[] }>(
        `/api/io/outbound?workspace_id=${encodeURIComponent(workspaceId)}`,
      ).catch(() => ({ sinks: [] })),
      apiGet<{ recipes: RecipeOption[] }>(
        `/api/recipes?workspace_id=${encodeURIComponent(workspaceId)}`,
      ).catch(() => ({ recipes: [] })),
      schedule
        ? apiGet<{ bindings: Array<{ sink_id: string }> }>(
            `/api/schedules/${schedule.id}/sinks`,
          ).catch(() => ({ bindings: [] }))
        : Promise.resolve({ bindings: [] }),
    ]).then(([workspaceAgents, globalAgents, runtimeResponse, sinkList, recipeList, bindings]) => {
      const seen = new Set<string>();
      setAgents(
        [...workspaceAgents, ...globalAgents].filter((item) => {
          if (seen.has(item.id)) return false;
          seen.add(item.id);
          return true;
        }),
      );
      setRuntime(runtimeResponse);
      setSinks(sinkList.sinks);
      setSelectedSinks(new Set(bindings.bindings.map((binding) => binding.sink_id)));
      setRecipes(recipeList.recipes);
    });
  }, [open, schedule, workspaceId, workspace?.default_router_policy, workspace?.default_email_recipient]);

  useEffect(() => {
    if (!open) return;
    const handle = window.setTimeout(() => {
      apiPost<SchedulePreview>("/api/schedules/preview", { recurrence, timezone })
        .then((value) => {
          setPreview(value);
          setError(null);
        })
        .catch((cause) =>
          setError(cause instanceof Error ? cause.message : String(cause)),
        );
    }, 300);
    return () => window.clearTimeout(handle);
  }, [open, recurrence, timezone]);

  function chooseAgent(value: AgentFile | null) {
    setAgent(value?.id ?? "");
    setAgentOpen(false);
    if (!policyTouched) {
      setPolicy(value?.router_policy || workspace?.default_router_policy || "COST_BENEFIT");
    }
  }

  function updatePrompt(value: string, cursor: number) {
    setPrompt(value);
    const before = value.slice(0, cursor);
    const match = before.match(/(?:^|\s)\/([\w:.-]*)$/);
    if (match) {
      setSlash({ query: match[1], start: cursor - match[1].length - 1 });
    } else {
      setSlash(null);
    }
  }

  function insertSlash(value: string) {
    if (!slash) return;
    const before = prompt.slice(0, slash.start);
    const after = prompt.slice(slash.start + slash.query.length + 1);
    setPrompt(`${before}${value} ${after}`);
    setSlash(null);
    window.setTimeout(() => promptRef.current?.focus(), 0);
  }

  const slashItems = useMemo(() => {
    if (!slash) return [];
    const query = slash.query.toLowerCase();
    const commands = (runtime?.commands ?? [])
      .filter((command) => command.name.toLowerCase().includes(query))
      .slice(0, 12)
      .map((command) => ({
        value: `/${command.name}`,
        label: `/${command.name}`,
        detail: command.description ?? "comando",
      }));
    const skills = (runtime?.skills ?? [])
      .filter((skill) => skill.id.toLowerCase().includes(query))
      .slice(0, 12)
      .map((skill) => ({
        value: `skill:${skill.id}`,
        label: `skill: ${skill.id}`,
        detail: skill.description ?? "skill",
      }));
    return [...commands, ...skills];
  }, [slash, runtime]);

  async function submit() {
    if (!name.trim()) {
      setError("Informe o nome da rotina.");
      return;
    }
    if (triggerType === "CRON" && !preview) {
      setError("A recorrência ainda não foi validada.");
      return;
    }
    if (targetKind === "RECIPE" && !recipeId) {
      setError("Escolha a receita que a rotina deve executar.");
      return;
    }
    if (targetKind === "PROMPT" && !prompt.trim()) {
      setError("Informe o comando ou prompt da rotina.");
      return;
    }
    setSaving(true);
    setError(null);
    try {
      const payload = {
        workspace_id: workspaceId,
        name: name.trim(),
        description: description.trim() || null,
        cron_expression:
          triggerType === "CRON" ? (preview?.cron_expression ?? "") : "",
        timezone: timezone.trim(),
        agent_name: agent.trim() || null,
        command_or_prompt: targetKind === "PROMPT" ? prompt.trim() : "",
        trigger_type: triggerType,
        trigger_config_json: JSON.stringify(triggerConfig),
        target_kind: targetKind,
        recipe_id: targetKind === "RECIPE" ? recipeId : null,
        router_policy: policy,
        output_folder_template: template.trim(),
        notify_email_recipients: recipients.trim() || null,
        notify_signal: signalEnabled,
        notify_signal_recipients: signalEnabled ? signalRecipients.trim() || null : null,
        notify_only_on_change: onlyOnChange,
        recurrence_json:
          triggerType === "CRON" ? JSON.stringify(recurrence) : null,
        one_time: triggerType === "CRON" && recurrence.frequency === "once",
        misfire_policy: misfirePolicy,
        safe_readonly_mode: safeReadonly,
        sync_to_google_docs: schedule?.sync_to_google_docs ?? false,
        sync_to_google_calendar: schedule?.sync_to_google_calendar ?? false,
        is_active: schedule?.is_active ?? true,
      };
      let scheduleId = schedule?.id;
      if (schedule) {
        await apiPatch(`/api/schedules/${schedule.id}`, payload);
      } else {
        const created = await apiPost<Schedule>("/api/schedules", payload);
        scheduleId = created.id;
      }
      if (scheduleId) {
        await apiPost(`/api/schedules/${scheduleId}/sinks`, {
          sink_ids: Array.from(selectedSinks),
        });
      }
      await onCreated();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setSaving(false);
    }
  }

  const filteredAgents = agents.filter((item) =>
    `${item.id} ${item.name ?? ""}`.toLowerCase().includes(agent.toLowerCase()),
  );
  const selectedRecipe = recipes.find((item) => item.id === recipeId);
  const filteredRecipes = recipes.filter((item) =>
    `${item.id} ${item.name}`.toLowerCase().includes(recipeQuery.toLowerCase()),
  );

  return (
    <Modal
      open={open}
      onClose={onClose}
      title={schedule ? "Editar rotina" : "Nova rotina"}
      description="A rotina pertence ao workspace ativo. Mude de workspace na barra superior para gerenciar as rotinas de outro workspace."
      className="max-w-3xl"
    >
      <div className="space-y-3">
        <div className="grid gap-2 md:grid-cols-2">
          <label>
            <span className={labelClass}>Workspace</span>
            <input value={workspace?.name ?? workspaceId} disabled className={inputClass} />
          </label>
          <label>
            <span className={labelClass}>Nome da rotina</span>
            <input value={name} onChange={(event) => setName(event.target.value)} className={inputClass} />
          </label>
        </div>

        <div className="rounded-xl border border-zinc-800/70 bg-zinc-950/30">
          <button
            type="button"
            className="flex w-full flex-wrap items-center gap-2 px-3 py-2.5 text-left"
            onClick={() => setWhenOpen((current) => !current)}
            aria-expanded={whenOpen}
          >
            <CalendarClock size={14} className="shrink-0 text-blue-300" />
            <span className="text-[11px] font-semibold text-zinc-200">
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
                <span className="mt-1 block text-[10px] leading-snug text-zinc-500">
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
                    <span className={labelClass}>Timezone</span>
                    <input
                      value={timezone}
                      onChange={(event) => setTimezone(event.target.value)}
                      className={cn(inputClass, "font-mono")}
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
                  <span className={labelClass}>Segredo do webhook (opcional)</span>
                  <input
                    value={String(triggerConfig.secret ?? "")}
                    onChange={(event) =>
                      setTriggerConfig((current) => ({
                        ...current,
                        secret: event.target.value,
                      }))
                    }
                    className={inputClass}
                    placeholder="usa o segredo global quando vazio"
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
                      className={cn(inputClass, "font-mono text-[11px]")}
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

        <div className="rounded-xl border border-zinc-800/70 bg-zinc-950/30 p-3">
          <div className="flex flex-wrap items-center gap-2">
            <p className="text-[10px] font-medium uppercase tracking-wide text-zinc-500">
              O que executar
            </p>
            <div className="ml-auto flex items-center gap-1 rounded-lg border border-zinc-800 bg-zinc-950/60 p-0.5">
              <button
                type="button"
                onClick={() => setTargetKind("PROMPT")}
                className={cn(
                  "rounded-md px-2 py-1 text-[10px]",
                  targetKind === "PROMPT"
                    ? "workspace-tab-active"
                    : "text-zinc-500 hover:text-zinc-200",
                )}
              >
                Prompt
              </button>
              <button
                type="button"
                onClick={() => setTargetKind("RECIPE")}
                className={cn(
                  "rounded-md px-2 py-1 text-[10px]",
                  targetKind === "RECIPE"
                    ? "workspace-tab-active"
                    : "text-zinc-500 hover:text-zinc-200",
                )}
              >
                Receita
              </button>
            </div>
          </div>

          {targetKind === "RECIPE" ? (
            <div className="relative mt-2">
              <span className={labelClass}>Receita</span>
              <IconLabel
                icon={<ChevronDown size={12} className="text-zinc-500" />}
                className="pointer-events-none absolute right-2 top-[30px] text-zinc-500"
              />
              <input
                value={recipeOpen ? recipeQuery : (selectedRecipe?.name ?? "")}
                onChange={(event) => {
                  setRecipeQuery(event.target.value);
                  setRecipeOpen(true);
                }}
                onFocus={() => {
                  setRecipeQuery("");
                  setRecipeOpen(true);
                }}
                onBlur={() => window.setTimeout(() => setRecipeOpen(false), 150)}
                placeholder="Digite para buscar a receita…"
                className={inputClass}
              />
              <FieldHint
                hint="A rotina executa o fluxo da receita, com os checkpoints e saídas definidos nela."
                className="absolute right-7 top-[32px]"
              />
              {recipeOpen ? (
                <div className="absolute z-30 mt-1 max-h-56 w-full overflow-y-auto rounded-lg border border-zinc-700 bg-zinc-950 p-1 shadow-xl">
                  {filteredRecipes.length === 0 ? (
                    <p className="px-2 py-1 text-[10px] text-zinc-600">
                      Nenhuma receita encontrada.
                    </p>
                  ) : (
                    filteredRecipes.map((item) => (
                      <button
                        key={item.id}
                        type="button"
                        className="block w-full rounded px-2 py-1.5 text-left text-[10px] hover:bg-zinc-800"
                        onMouseDown={(event) => {
                          event.preventDefault();
                          setRecipeId(item.id);
                          setRecipeQuery("");
                          setRecipeOpen(false);
                        }}
                      >
                        <span className="text-zinc-200">{item.name}</span>
                        <span className="ml-2 font-mono text-[9px] text-zinc-600">
                          {item.id}
                        </span>
                      </button>
                    ))
                  )}
                </div>
              ) : null}
            </div>
          ) : (
            <>
              <div className="mt-2 grid gap-2 md:grid-cols-2">
                <label className="relative">
                  <span className={labelClass}>Agente (opcional)</span>
                  <input
                    value={agent}
                    onChange={(event) => {
                      setAgent(event.target.value);
                      setAgentOpen(true);
                    }}
                    onFocus={() => setAgentOpen(true)}
                    onBlur={() => window.setTimeout(() => setAgentOpen(false), 150)}
                    placeholder="automático (router)"
                    className={inputClass}
                  />
                  {agentOpen ? (
                    <div className="absolute z-30 mt-1 max-h-56 w-full overflow-y-auto rounded-lg border border-zinc-700 bg-zinc-950 p-1 shadow-xl">
                      <button
                        type="button"
                        className="block w-full rounded px-2 py-1 text-left text-[10px] text-zinc-400 hover:bg-zinc-800"
                        onMouseDown={(event) => {
                          event.preventDefault();
                          chooseAgent(null);
                        }}
                      >
                        automático (router)
                      </button>
                      {filteredAgents.slice(0, 60).map((item) => (
                        <button
                          key={item.id}
                          type="button"
                          className="block w-full rounded px-2 py-1 text-left text-[10px] hover:bg-zinc-800"
                          onMouseDown={(event) => {
                            event.preventDefault();
                            chooseAgent(item);
                          }}
                        >
                          <span className="font-mono text-zinc-200">@{item.id}</span>
                          <span className="ml-1 text-zinc-500">{item.name}</span>
                          {item.router_policy ? (
                            <span className="ml-1 text-zinc-600">
                              {item.router_policy}
                            </span>
                          ) : null}
                        </button>
                      ))}
                      {filteredAgents.length === 0 ? (
                        <p className="px-2 py-1 text-[10px] text-zinc-600">
                          Nenhum agente encontrado.
                        </p>
                      ) : null}
                    </div>
                  ) : null}
                </label>
                <label>
                  <span className={labelClass}>Política do roteador</span>
                  <select
                    value={policy}
                    onChange={(event) => {
                      setPolicy(event.target.value);
                      setPolicyTouched(true);
                    }}
                    className={inputClass}
                  >
                    {Object.entries(ROUTER_POLICY_LABELS).map(([value, label]) => (
                      <option key={value} value={value}>
                        {label}
                      </option>
                    ))}
                  </select>
                </label>
                <label>
                  <span className={labelClass}>Template da pasta de saída</span>
                  <input
                    value={template}
                    onChange={(event) => setTemplate(event.target.value)}
                    className={cn(inputClass, "font-mono text-[10px]")}
                  />
                </label>
              </div>

              <label className="relative mt-2 block">
                <span className={labelClass}>Comando ou prompt</span>
                <textarea
                  ref={promptRef}
                  value={prompt}
                  onChange={(event) =>
                    updatePrompt(
                      event.target.value,
                      event.target.selectionStart ?? 0,
                    )
                  }
                  onKeyDown={(event) => {
                    if (event.key === "Escape") setSlash(null);
                  }}
                  rows={5}
                  placeholder="Digite / para comandos e skills, ou descreva a tarefa"
                  className={cn(
                    inputClass,
                    "h-auto resize-y py-2 font-mono text-[11px]",
                  )}
                />
                {slash ? (
                  <div className="absolute z-30 mt-1 max-h-64 w-full overflow-y-auto rounded-lg border border-zinc-700 bg-zinc-950 p-1 shadow-xl">
                    {slashItems.length === 0 ? (
                      <p className="px-2 py-1 text-[10px] text-zinc-600">
                        Nenhum comando/skill encontrado.
                      </p>
                    ) : (
                      slashItems.map((item) => (
                        <button
                          key={`${item.label}:${item.detail}`}
                          type="button"
                          className="block w-full rounded px-2 py-1 text-left hover:bg-zinc-800"
                          onMouseDown={(event) => {
                            event.preventDefault();
                            insertSlash(item.value);
                          }}
                        >
                          <span className="font-mono text-[10px] text-zinc-200">
                            {item.label}
                          </span>
                          <span className="ml-2 text-[9px] text-zinc-600">
                            {item.detail}
                          </span>
                        </button>
                      ))
                    )}
                  </div>
                ) : null}
              </label>
            </>
          )}
        </div>
        <label>
          <span className={labelClass}>Descrição</span>
          <input value={description} onChange={(event) => setDescription(event.target.value)} className={inputClass} />
        </label>

        <div className="rounded-xl border border-zinc-800/70 bg-zinc-950/30 p-3">
          <p className="text-[10px] font-medium uppercase tracking-wide text-zinc-500">Notificações</p>
          <div className="mt-2 grid gap-2 md:grid-cols-2">
            <label>
              <span className={labelClass}>E-mail</span>
              <input value={recipients} onChange={(event) => setRecipients(event.target.value)} placeholder="a@empresa.com, b@empresa.com" className={inputClass} />
            </label>
            <label>
              <span className={labelClass}>Signal (números, opcional)</span>
              <input value={signalRecipients} onChange={(event) => setSignalRecipients(event.target.value)} placeholder="vazio usa a allowlist do Signal" className={inputClass} />
            </label>
          </div>
          <div className="mt-2 flex flex-wrap gap-4 text-[11px] text-zinc-400">
            <label className="flex items-center gap-2">
              <input type="checkbox" checked={signalEnabled} onChange={(event) => setSignalEnabled(event.target.checked)} className="accent-blue-500" />
              notificar por Signal
            </label>
            <label className="flex items-center gap-2">
              <input type="checkbox" checked={onlyOnChange} onChange={(event) => setOnlyOnChange(event.target.checked)} className="accent-blue-500" />
              notificar somente se houver novidade
            </label>
          </div>
        </div>

        {sinks.length > 0 ? (
          <div className="rounded-xl border border-zinc-800/70 bg-zinc-950/30 p-3">
            <p className="text-[10px] font-medium uppercase tracking-wide text-zinc-500">
              Entregas automáticas
            </p>
            <div className="mt-2 flex flex-wrap gap-x-4 gap-y-1">
              {sinks.map((sink) => (
                <label key={sink.id} className="flex items-center gap-1.5 text-[11px] text-zinc-400">
                  <input
                    type="checkbox"
                    checked={selectedSinks.has(sink.id)}
                    onChange={(event) =>
                      setSelectedSinks((current) => {
                        const next = new Set(current);
                        if (event.target.checked) next.add(sink.id);
                        else next.delete(sink.id);
                        return next;
                      })
                    }
                    className="accent-blue-500"
                  />
                  {sink.name}
                </label>
              ))}
            </div>
          </div>
        ) : null}

        {error ? (
          <p className="rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[11px] text-red-300">{error}</p>
        ) : null}
      </div>
      <div className="mt-4 flex justify-end gap-2 border-t border-zinc-800/70 pt-4">
        <Button variant="ghost" size="sm" onClick={onClose}>Cancelar</Button>
        <Button size="sm" onClick={() => void submit()} disabled={saving}>
          {saving ? <Loader2 size={14} className="animate-spin" /> : <Plus size={14} />}
          {schedule ? "Salvar rotina" : "Criar rotina"}
        </Button>
      </div>
    </Modal>
  );
}

const inputClass =
  "h-9 w-full rounded-lg border border-zinc-800 bg-zinc-900/50 px-2.5 text-[12px] text-zinc-100 outline-none placeholder:text-zinc-600 focus:border-blue-500/60";
const labelClass =
  "mb-1.5 block text-[10px] font-medium uppercase tracking-[0.12em] text-zinc-500";

import { useEffect, useMemo, useRef, useState } from "react";
import {
  CalendarClock,
  ChevronDown,
  ChevronRight,
  Loader2,
  Plus,
} from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Modal } from "@/components/ui/modal";
import { FieldHint } from "@/components/ui/field-hint";
import { IconLabel } from "@/components/ui/icon-label";
import { PathInput } from "@/components/ui/path-input";
import { TimezoneSelect } from "@/components/ui/timezone-select";
import type { OutboundSink } from "@/components/integrations/OutboundSinksCard";
import { useAppStore } from "@/store/app-store";
import { PromptAttachments } from "@/components/ui/prompt-attachments";
import { RecurrenceEditor } from "@/components/routines/RecurrenceEditor";
import { apiDelete, apiGet, apiPatch, apiPost } from "@/lib/api";
import { shouldCloseOnBlur } from "@/lib/combobox";
import { ROUTER_POLICY_LABELS } from "@/lib/types";
import type {
  AgentFile,
  Recurrence,
  Schedule,
  SchedulePreview,
  StudioRuntime,
} from "@/lib/types";
import { TRIGGER_TYPES, triggerDescription, triggerLabel } from "@/lib/trigger-types";
import { cn } from "@/lib/utils";
import {
  fieldInputClass as inputClass,
  fieldLabelClass as labelClass,
} from "@/lib/ui-classes";

export interface RecipeOption {
  id: string;
  name: string;
}

export function CreateScheduleModal({
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
  const [attachments, setAttachments] = useState<string[]>([]);
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
    setAttachments([]);
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

  function cancel() {
    const paths = attachments;
    setAttachments([]);
    if (workspaceId) {
      for (const path of paths) {
        void apiDelete(
          `/api/attachments?workspace_id=${encodeURIComponent(workspaceId)}&path=${encodeURIComponent(path)}`,
        ).catch(() => {
          // Best effort; an orphan file is harmless.
        });
      }
    }
    onClose();
  }

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
      const attachmentBlock =
        attachments.length > 0
          ? `\n\nArquivos anexados:\n${attachments
              .map((path) => `- ${path}`)
              .join("\n")}`
          : "";
      const payload = {
        workspace_id: workspaceId,
        name: name.trim(),
        description: description.trim() || null,
        cron_expression:
          triggerType === "CRON" ? (preview?.cron_expression ?? "") : "",
        timezone: timezone.trim(),
        agent_name: agent.trim() || null,
        command_or_prompt:
          targetKind === "PROMPT" ? `${prompt.trim()}${attachmentBlock}` : "",
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
        // New routines always start paused; activation is explicit.
        is_active: schedule?.is_active ?? false,
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
      onClose={cancel}
      title={schedule ? "Editar rotina" : "Nova rotina"}
      description="A rotina pertence ao workspace ativo. Mude de workspace na barra superior para gerenciar as rotinas de outro workspace. Toda rotina nova nasce desligada; ative quando revisar."
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
                onBlur={(event) => {
                  if (shouldCloseOnBlur(event)) setRecipeOpen(false);
                }}
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
                    onBlur={(event) => {
                      if (shouldCloseOnBlur(event)) setAgentOpen(false);
                    }}
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
              {targetKind === "PROMPT" ? (
                <PromptAttachments
                  workspaceId={workspaceId}
                  value={attachments}
                  onChange={setAttachments}
                />
              ) : null}
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
        <Button variant="ghost" size="sm" onClick={cancel}>Cancelar</Button>
        <Button size="sm" onClick={() => void submit()} disabled={saving}>
          {saving ? <Loader2 size={14} className="animate-spin" /> : <Plus size={14} />}
          {schedule ? "Salvar rotina" : "Criar rotina"}
        </Button>
      </div>
    </Modal>
  );
}


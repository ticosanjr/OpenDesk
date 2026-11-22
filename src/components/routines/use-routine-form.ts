import { useEffect, useMemo, useRef, useState } from "react";
import type { OutboundSink } from "@/components/integrations/OutboundSinksCard";
import { useAppStore } from "@/store/app-store";
import { apiDelete, apiGet, apiPatch, apiPost } from "@/lib/api";
import type {
  AgentFile,
  Recurrence,
  Schedule,
  SchedulePreview,
  StudioRuntime,
} from "@/lib/types";

export interface RecipeOption {
  id: string;
  name: string;
}

interface UseRoutineFormInput {
  open: boolean;
  workspaceId: string;
  schedule: Schedule | null;
  onClose: () => void;
  onCreated: () => Promise<void>;
}

export function useRoutineForm({
  open,
  workspaceId,
  schedule,
  onClose,
  onCreated,
}: UseRoutineFormInput) {
  const workspaces = useAppStore((state) => state.workspaces);
  const workspace = workspaces.find((item) => item.id === workspaceId);
  // Workspace settings can arrive after the modal opens. Keep the latest
  // values in a ref so a late load never re-runs the reset effect and wipes
  // what the user is typing.
  const workspaceDefaults = useRef({
    router_policy: workspace?.default_router_policy,
    email: workspace?.default_email_recipient,
  });
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
  const [telegramEnabled, setTelegramEnabled] = useState(false);
  const [telegramRecipients, setTelegramRecipients] = useState("");
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
    workspaceDefaults.current = {
      router_policy: workspace?.default_router_policy,
      email: workspace?.default_email_recipient,
    };
  }, [workspace?.default_router_policy, workspace?.default_email_recipient]);

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
    setPolicy(
      schedule?.router_policy ??
        workspaceDefaults.current.router_policy ??
        "COST_BENEFIT",
    );
    setPolicyTouched(false);
    setTemplate(schedule?.output_folder_template ?? "output/{YYYY}-{MM}/{schedule_slug}");
    setRecipients(
      schedule?.notify_email_recipients ?? workspaceDefaults.current.email ?? "",
    );
    setSignalEnabled(schedule?.notify_signal ?? false);
    setSignalRecipients(schedule?.notify_signal_recipients ?? "");
    setTelegramEnabled(schedule?.notify_telegram ?? false);
    setTelegramRecipients(schedule?.notify_telegram_recipients ?? "");
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
  }, [open, schedule, workspaceId]);

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
      setPolicy(value?.router_policy || workspaceDefaults.current.router_policy || "COST_BENEFIT");
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
        notify_telegram: telegramEnabled,
        notify_telegram_recipients: telegramEnabled
          ? telegramRecipients.trim() || null
          : null,
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

  return {
    workspaces,
    workspace,
    name,
    setName,
    description,
    setDescription,
    triggerType,
    setTriggerType,
    triggerConfig,
    setTriggerConfig,
    targetKind,
    setTargetKind,
    recipeId,
    setRecipeId,
    recipes,
    recipeQuery,
    setRecipeQuery,
    recipeOpen,
    setRecipeOpen,
    whenOpen,
    setWhenOpen,
    prompt,
    setPrompt,
    attachments,
    setAttachments,
    agent,
    setAgent,
    agentOpen,
    setAgentOpen,
    agents,
    policy,
    setPolicy,
    setPolicyTouched,
    template,
    setTemplate,
    recipients,
    setRecipients,
    signalEnabled,
    setSignalEnabled,
    signalRecipients,
    setSignalRecipients,
    telegramEnabled,
    setTelegramEnabled,
    telegramRecipients,
    setTelegramRecipients,
    onlyOnChange,
    setOnlyOnChange,
    timezone,
    setTimezone,
    misfirePolicy,
    setMisfirePolicy,
    safeReadonly,
    setSafeReadonly,
    sinks,
    selectedSinks,
    setSelectedSinks,
    recurrence,
    setRecurrence,
    preview,
    runtime,
    slash,
    setSlash,
    promptRef,
    saving,
    error,
    chooseAgent,
    updatePrompt,
    insertSlash,
    slashItems,
    cancel,
    submit,
    filteredAgents,
    selectedRecipe,
    filteredRecipes,
  };
}

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  ChevronDown,
  ChevronUp,
  GitBranch,
  Loader2,
  Play,
  Plus,
  RefreshCw,
  Save,
  Sparkles,
  Trash2,
  Workflow,
} from "lucide-react";
import { parse as parseYaml, stringify as stringifyYaml } from "yaml";
import { PageBody, PageHeader } from "@/components/layout/PageHeader";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { CountPill } from "@/components/ui/count-pill";
import { EmptyState } from "@/components/ui/empty-state";
import { FieldHint } from "@/components/ui/field-hint";
import { IconLabel } from "@/components/ui/icon-label";
import { LoadingState } from "@/components/ui/loading-state";
import { PathInput } from "@/components/ui/path-input";
import { AgentCombobox } from "@/components/ui/agent-combobox";
import { ModelCombobox } from "@/components/ui/model-combobox";
import { apiGet, apiPost, apiPut } from "@/lib/api";
import { useAppStore } from "@/store/app-store";
import { toast } from "@/store/toast-store";
import { relativeTime } from "@/lib/format";
import { TRIGGER_TYPES, triggerDescription } from "@/lib/trigger-types";
import { cn } from "@/lib/utils";

interface RecipeIndex {
  id: string;
  name: string;
  trigger_type: string;
  is_enabled: boolean;
  last_error?: string | null;
  last_run_at?: string | null;
  total_runs: number;
}

interface RecipeRun {
  id: string;
  recipe_id: string;
  status: string;
  current_step_id?: string | null;
  started_at: string;
  completed_at?: string | null;
  error_message?: string | null;
}

type Dictionary = Record<string, unknown>;
type FlowEntry = Dictionary & { type: "step" | "hitl" | "sink" };

const DEFAULT_YAML = `id: nova-receita
name: Nova receita
enabled: true
triggers: []
flow:
  - type: step
    id: passo-1
    name: Primeiro passo
    mode: build
    agent: build
    effort: low
    prompt: |
      Execute o primeiro passo e salve o resultado em {{run.output_dir}}.
`;

export const HITL_TYPES = [
  {
    value: "BINARY_GATE",
    label: "Aprovação simples (BINARY_GATE)",
    description: "Uma decisão de aprovar ou rejeitar.",
  },
  {
    value: "MULTI_CHOICE",
    label: "Escolha uma opção (MULTI_CHOICE)",
    description: "O usuário escolhe uma das opções informadas.",
  },
  {
    value: "APPROVAL_WITH_FEEDBACK",
    label: "Aprovar com feedback (APPROVAL_WITH_FEEDBACK)",
    description: "Aprova, rejeita ou pede uma correção com texto.",
  },
  {
    value: "ARTIFACT_EDIT_HANDOFF",
    label: "Editar arquivo (ARTIFACT_EDIT_HANDOFF)",
    description: "O usuário edita o artefato e devolve o arquivo.",
  },
  {
    value: "TOOL_PERMISSION_PROXY",
    label: "Permissão de ferramenta (TOOL_PERMISSION_PROXY)",
    description: "Autoriza uma ferramenta externa antes de continuar.",
  },
];

export const SINK_TYPES = [
  { value: "DOC_COMPILER", label: "Compilar documento (DOC_COMPILER)" },
  { value: "EMAIL_SMTP", label: "Enviar e-mail (EMAIL_SMTP)" },
  { value: "SIGNAL_MSG", label: "Enviar Signal (SIGNAL_MSG)" },
  { value: "TELEGRAM_MSG", label: "Enviar Telegram (TELEGRAM_MSG)" },
  { value: "RSYNC_FS", label: "Copiar via Rsync (RSYNC_FS)" },
  { value: "GIT_COMMIT", label: "Commit no Git (GIT_COMMIT)" },
  { value: "SQL_INSERT", label: "Inserir no SQL (SQL_INSERT)" },
];

const CHANNELS = ["DESKTOP_UI", "EMAIL", "SIGNAL"];

const ON_TIMEOUT = [
  { value: "PAUSE_AND_ALERT", label: "Pausar e avisar" },
  { value: "AUTO_APPROVE", label: "Aprovar automaticamente" },
  { value: "ABORT", label: "Abortar a receita" },
];

export function RecipesPage() {
  const workspaceId = useAppStore((state) => state.activeWorkspaceId);
  const [recipes, setRecipes] = useState<RecipeIndex[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [yaml, setYaml] = useState(DEFAULT_YAML);
  const [runs, setRuns] = useState<RecipeRun[]>([]);
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [editorLoading, setEditorLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const recipe = useMemo(() => {
    try {
      const parsed = parseYaml(yaml);
      return parsed && typeof parsed === "object"
        ? (parsed as Dictionary)
        : null;
    } catch {
      return null;
    }
  }, [yaml]);

  const load = useCallback(async () => {
    if (!workspaceId) return;
    setLoading(true);
    try {
      const [recipeResult, runResult] = await Promise.all([
        apiGet<{ recipes: RecipeIndex[] }>(
          `/api/recipes?workspace_id=${encodeURIComponent(workspaceId)}`,
        ),
        apiGet<{ runs: RecipeRun[] }>(
          `/api/recipes/runs?workspace_id=${encodeURIComponent(workspaceId)}`,
        ),
      ]);
      setRecipes(recipeResult.recipes);
      setRuns(runResult.runs);
      setSelected((current) => current ?? recipeResult.recipes[0]?.id ?? null);
      setError(null);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setLoading(false);
    }
  }, [workspaceId]);

  useEffect(() => {
    void load();
  }, [load]);

  useEffect(() => {
    if (!selected) return;
    let cancelled = false;
    setEditorLoading(true);
    void apiGet<{ yaml: string }>(`/api/recipes/${encodeURIComponent(selected)}`)
      .then((value) => {
        if (!cancelled) setYaml(value.yaml);
      })
      .catch((cause) => {
        if (!cancelled) {
          setError(cause instanceof Error ? cause.message : String(cause));
        }
      })
      .finally(() => {
        if (!cancelled) setEditorLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [selected]);

  function update(next: Dictionary) {
    setYaml(stringifyYaml(next));
  }

  async function save() {
    if (!recipe || !selected) return;
    setSaving(true);
    try {
      await apiPut(`/api/recipes/${encodeURIComponent(selected)}`, { yaml });
      await load();
      toast({ tone: "success", title: "Receita salva" });
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setSaving(false);
    }
  }

  async function create() {
    const id = `receita-${Date.now().toString(36)}`;
    const text = DEFAULT_YAML.replace("nova-receita", id);
    try {
      await apiPut(`/api/recipes/${id}`, { yaml: text });
      await load();
      setSelected(id);
      toast({ tone: "success", title: "Receita criada" });
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  }

  async function run(dryRun: boolean) {
    if (!selected) return;
    try {
      const run = await apiPost<RecipeRun>(`/api/recipes/${selected}/run`, {
        dry_run: dryRun,
        trigger_context: { manual: true },
      });
      toast({
        tone: "success",
        title: dryRun ? "Dry-run iniciado" : "Receita iniciada",
        description: `run ${run.id.slice(0, 8)}`,
      });
      await load();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  }

  return (
    <PageBody>
      <PageHeader
        eyebrow="workflows declarativos"
        title="Receitas"
        description="Monte o fluxo visualmente, use zero ou vários gatilhos e combine etapas, checkpoints e saídas na ordem que fizer sentido."
        actions={
          <>
            <Button variant="outline" size="sm" onClick={() => void load()} disabled={loading}>
              <IconLabel icon={<RefreshCw size={13} className={loading ? "animate-spin" : undefined} />}>
                Atualizar
              </IconLabel>
            </Button>
            <Button size="sm" onClick={() => void create()}>
              <IconLabel icon={<Plus size={13} />}>Nova receita</IconLabel>
            </Button>
          </>
        }
      />

      {error ? (
        <p className="rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[11px] text-red-300">
          {error}
        </p>
      ) : null}

      <div className="grid min-h-0 gap-3 lg:grid-cols-[260px_1fr]">
        <Card className="min-h-0">
          <CardHeader className="py-3">
            <CardTitle className="flex items-center gap-2 text-[12px]">
              <Workflow size={14} className="text-zinc-500" />
              Receitas
            </CardTitle>
            <CountPill value={recipes.length} />
          </CardHeader>
          <CardContent className="max-h-[70dvh] space-y-1 overflow-y-auto p-2">
            {loading && recipes.length === 0 ? (
              <LoadingState rows={4} compact />
            ) : recipes.length === 0 ? (
              <EmptyState
                icon={Workflow}
                title="Nenhuma receita"
                description="Crie a primeira receita YAML."
                className="border-0 bg-transparent"
              />
            ) : (
              recipes.map((item) => (
                <button
                  key={item.id}
                  type="button"
                  onClick={() => setSelected(item.id)}
                  className={cn(
                    "w-full rounded-lg px-2.5 py-2 text-left transition-colors",
                    selected === item.id
                      ? "workspace-nav-active text-zinc-100"
                      : "text-zinc-400 hover:bg-zinc-900",
                  )}
                >
                  <p className="truncate text-[11px] font-medium">{item.name}</p>
                  <p className="truncate font-mono text-[9px] text-zinc-600">
                    {item.trigger_type} · {item.total_runs} execuções
                  </p>
                  {item.last_error ? (
                    <p className="truncate text-[9px] text-red-300">{item.last_error}</p>
                  ) : null}
                </button>
              ))
            )}
          </CardContent>
        </Card>

        <div className="grid min-h-0 gap-3 xl:grid-cols-2">
          <Card className="min-h-0">
            <CardHeader className="py-3">
              <CardTitle className="flex items-center gap-2 text-[12px]">
                <GitBranch size={13} className="text-zinc-500" />
                Fluxo visual
              </CardTitle>
              <div className="flex gap-1">
                <Button variant="outline" size="sm" className="h-7" onClick={() => void run(true)} disabled={!selected}>
                  <IconLabel icon={<Sparkles size={12} />}>Dry-run</IconLabel>
                </Button>
                <Button size="sm" className="h-7" onClick={() => void run(false)} disabled={!selected}>
                  <IconLabel icon={<Play size={12} />}>Executar</IconLabel>
                </Button>
              </div>
            </CardHeader>
            <CardContent className="max-h-[70dvh] space-y-2 overflow-y-auto">
              {editorLoading ? (
                <LoadingState label="Carregando receita…" rows={5} />
              ) : recipe ? (
                <RecipeStepper
                  recipe={recipe}
                  workspaceId={workspaceId}
                  onChange={update}
                />
              ) : (
                <p className="rounded-lg border border-red-500/20 bg-red-500/10 p-3 text-[11px] text-red-300">
                  YAML inválido. Corrija no editor ao lado.
                </p>
              )}
            </CardContent>
          </Card>

          <Card className="min-h-0">
            <CardHeader className="py-3">
              <CardTitle className="flex items-center gap-2 text-[12px]">
                YAML
                <FieldHint hint="Editor simples local. Sempre abre offline e nunca fica carregando." />
              </CardTitle>
              <Button size="sm" className="h-7" onClick={() => void save()} disabled={!selected || saving}>
                <IconLabel icon={saving ? <Loader2 size={12} className="animate-spin" /> : <Save size={12} />}>
                  Salvar
                </IconLabel>
              </Button>
            </CardHeader>
            <CardContent className="h-[70dvh] p-0">
              <LocalYamlEditor value={yaml} onChange={setYaml} />
            </CardContent>
          </Card>
        </div>
      </div>

      <Card>
        <CardHeader className="py-3">
          <CardTitle className="text-[12px]">Execuções de receitas</CardTitle>
          <CountPill value={runs.length} />
        </CardHeader>
        <CardContent className="space-y-1">
          {runs.length === 0 ? (
            <p className="p-2 text-[11px] text-zinc-600">Nenhuma execução ainda.</p>
          ) : (
            runs.map((run) => (
              <div key={run.id} className="flex flex-wrap items-center gap-2 rounded border border-zinc-800/70 px-2.5 py-1.5">
                <Badge variant={run.status === "COMPLETED" ? "green" : run.status.includes("WAITING") ? "yellow" : run.status.includes("FAILED") || run.status === "REJECTED" ? "red" : "blue"}>
                  {run.status}
                </Badge>
                <span className="font-mono text-[9px] text-zinc-500">{run.recipe_id}</span>
                <span className="min-w-0 flex-1 truncate font-mono text-[9px] text-zinc-600">{run.current_step_id ?? "—"}</span>
                <span className="text-[9px] text-zinc-600">{relativeTime(run.started_at)}</span>
              </div>
            ))
          )}
        </CardContent>
      </Card>
    </PageBody>
  );
}

function LocalYamlEditor({
  value,
  onChange,
}: {
  value: string;
  onChange: (value: string) => void;
}) {
  const textarea = useRef<HTMLTextAreaElement | null>(null);
  const gutter = useRef<HTMLDivElement | null>(null);
  const lines = value.split("\n").length;

  return (
    <div className="flex h-full min-h-0 bg-zinc-950">
      <div
        ref={gutter}
        aria-hidden
        className="h-full select-none overflow-hidden border-r border-zinc-800/70 px-2 py-3 text-right font-mono text-[10px] leading-[1.6] text-zinc-700"
      >
        {Array.from({ length: lines }).map((_, index) => (
          <div key={index}>{index + 1}</div>
        ))}
      </div>
      <textarea
        ref={textarea}
        value={value}
        onChange={(event) => onChange(event.target.value)}
        onScroll={(event) => {
          if (gutter.current) {
            gutter.current.scrollTop = event.currentTarget.scrollTop;
          }
        }}
        spellCheck={false}
        className="h-full min-h-0 flex-1 resize-none bg-transparent p-3 font-mono text-[11px] leading-[1.6] text-zinc-200 outline-none"
        aria-label="Editor YAML da receita"
      />
    </div>
  );
}

function isFlowEntry(value: unknown): value is FlowEntry {
  if (!value || typeof value !== "object") return false;
  const type = (value as Dictionary).type;
  return type === "step" || type === "hitl" || type === "sink";
}

function normalizeFlow(recipe: Dictionary): FlowEntry[] {
  if (Array.isArray(recipe.flow)) {
    const flow = recipe.flow.filter(isFlowEntry);
    if (flow.length > 0) return flow;
  }
  const steps = Array.isArray(recipe.steps)
    ? (recipe.steps as Dictionary[])
    : [];
  const checkpoints = Array.isArray(recipe.hitl_checkpoints)
    ? (recipe.hitl_checkpoints as Dictionary[])
    : [];
  const sinks = Array.isArray(recipe.sinks)
    ? (recipe.sinks as Dictionary[])
    : [];
  const flow: FlowEntry[] = [];
  for (const step of steps) {
    flow.push({ ...step, type: "step" });
    for (const checkpoint of checkpoints.filter(
      (item) => item.after_step === step.id,
    )) {
      flow.push({ ...checkpoint, type: "hitl" });
    }
  }
  for (const checkpoint of checkpoints.filter(
    (item) => !item.after_step,
  )) {
    flow.push({ ...checkpoint, type: "hitl" });
  }
  for (const sink of sinks) {
    flow.push({
      ...sink,
      type: "sink",
      sink_type: sink.sink_type ?? sink.type,
    });
  }
  return flow;
}

function normalizeTriggers(recipe: Dictionary): Dictionary[] {
  if (Array.isArray(recipe.triggers)) {
    return (recipe.triggers as Dictionary[]).filter(
      (trigger) => trigger && typeof trigger === "object",
    );
  }
  const legacy = recipe.trigger;
  if (legacy && typeof legacy === "object") {
    const trigger = legacy as Dictionary;
    if (typeof trigger.type === "string" && trigger.type) return [trigger];
  }
  return [];
}

function commit(
  recipe: Dictionary,
  flow: FlowEntry[],
  triggers: Dictionary[],
  onChange: (next: Dictionary) => void,
) {
  const next = { ...recipe };
  delete next.steps;
  delete next.hitl_checkpoints;
  delete next.sinks;
  delete next.trigger;
  next.flow = flow;
  next.triggers = triggers;
  onChange(next);
}

function RecipeStepper({
  recipe,
  workspaceId,
  onChange,
}: {
  recipe: Dictionary;
  workspaceId?: string | null;
  onChange: (next: Dictionary) => void;
}) {
  const flow = normalizeFlow(recipe);
  const triggers = normalizeTriggers(recipe);

  function setFlow(next: FlowEntry[]) {
    commit(recipe, next, triggers, onChange);
  }

  function setTriggers(next: Dictionary[]) {
    commit(recipe, flow, next, onChange);
  }

  function move(index: number, direction: -1 | 1) {
    const target = index + direction;
    if (target < 0 || target >= flow.length) return;
    const next = [...flow];
    [next[index], next[target]] = [next[target], next[index]];
    setFlow(next);
  }

  function patch(index: number, value: Dictionary) {
    setFlow(
      flow.map((item, position) =>
        position === index ? ({ ...item, ...value } as FlowEntry) : item,
      ),
    );
  }

  function addStep() {
    const count = flow.filter((item) => item.type === "step").length + 1;
    setFlow([
      ...flow,
      {
        type: "step",
        id: `passo-${count}`,
        name: `Passo ${count}`,
        mode: "build",
        prompt: "Descreva o que esta etapa deve fazer.",
      },
    ]);
  }

  function addCheckpoint() {
    const count = flow.filter((item) => item.type === "hitl").length + 1;
    setFlow([
      ...flow,
      {
        type: "hitl",
        id: `checkpoint-${count}`,
        name: `Checkpoint ${count}`,
        interaction_type: "BINARY_GATE",
        channels: ["DESKTOP_UI"],
        timeout_minutes: 240,
        on_timeout: "PAUSE_AND_ALERT",
        prompt_template: "Aprovar a execução?",
      },
    ]);
  }

  function addSink() {
    const count = flow.filter((item) => item.type === "sink").length + 1;
    setFlow([
      ...flow,
      {
        type: "sink",
        sink_type: "DOC_COMPILER",
        name: `Saída ${count}`,
        config: { format: "pdf" },
      },
    ]);
  }

  return (
    <div className="space-y-3">
      <section className="rounded-lg border border-blue-500/30 bg-blue-500/[0.04] p-2.5">
        <div className="flex flex-wrap items-center gap-2">
          <p className="text-[10px] font-semibold uppercase tracking-wide text-blue-200">
            Gatilhos de entrada
          </p>
          <CountPill value={triggers.length} tone="blue" />
          <FieldHint hint="Uma receita pode ter zero gatilhos (só por rotina/manual) ou vários ao mesmo tempo." />
          <Button
            variant="outline"
            size="sm"
            className="ml-auto h-7"
            onClick={() => setTriggers([...triggers, { type: "MANUAL", config: {} }])}
          >
            <IconLabel icon={<Plus size={11} />}>Adicionar gatilho</IconLabel>
          </Button>
        </div>
        {triggers.length === 0 ? (
          <p className="mt-2 rounded border border-dashed border-zinc-700 px-2.5 py-2 text-[10px] text-zinc-500">
            Sem gatilho. Esta receita só executa manualmente ou quando chamada por uma rotina.
          </p>
        ) : (
          <div className="mt-2 space-y-2">
            {triggers.map((trigger, index) => (
              <TriggerEditor
                key={index}
                trigger={trigger}
                index={index}
                onChange={(value) =>
                  setTriggers(
                    triggers.map((item, position) =>
                      position === index ? value : item,
                    ),
                  )
                }
                onRemove={() =>
                  setTriggers(triggers.filter((_, position) => position !== index))
                }
              />
            ))}
          </div>
        )}
      </section>

      <section className="relative space-y-2 border-l-2 border-zinc-800 pl-4">
        <div className="flex items-center gap-2">
          <p className="text-[10px] font-semibold uppercase tracking-wide text-zinc-400">
            Ordem de execução
          </p>
          <CountPill value={flow.length} />
        </div>

        {flow.map((item, index) => (
          <div
            key={`${item.type}-${index}`}
            className={cn(
              "rounded-lg border p-2.5",
              item.type === "step" && "border-zinc-800 bg-zinc-950/40",
              item.type === "hitl" && "border-amber-500/30 bg-amber-500/[0.04]",
              item.type === "sink" && "border-emerald-500/30 bg-emerald-500/[0.04]",
            )}
          >
            <div className="flex items-center gap-1">
              <Badge
                variant={
                  item.type === "step" ? "default" : item.type === "hitl" ? "yellow" : "green"
                }
              >
                {item.type === "step" ? "Etapa" : item.type === "hitl" ? "Checkpoint" : "Saída"}
              </Badge>
              <input
                value={(item.name as string) ?? ""}
                onChange={(event) => patch(index, { name: event.target.value })}
                className={cn(inputClass, "flex-1")}
                placeholder="Nome exibido"
              />
              <Button
                variant="ghost"
                size="icon"
                className="h-7 w-7"
                onClick={() => move(index, -1)}
                disabled={index === 0}
                title="Mover para cima"
              >
                <ChevronUp size={11} />
              </Button>
              <Button
                variant="ghost"
                size="icon"
                className="h-7 w-7"
                onClick={() => move(index, 1)}
                disabled={index === flow.length - 1}
                title="Mover para baixo"
              >
                <ChevronDown size={11} />
              </Button>
              <Button
                variant="ghost"
                size="icon"
                className="h-7 w-7 text-red-300"
                onClick={() => setFlow(flow.filter((_, position) => position !== index))}
                title="Remover"
              >
                <Trash2 size={11} />
              </Button>
            </div>

            {item.type === "step" ? (
              <StepEditor
                step={item}
                workspaceId={workspaceId}
                onChange={(value) => patch(index, value)}
              />
            ) : item.type === "hitl" ? (
              <HitlEditor
                checkpoint={item}
                onChange={(value) => patch(index, value)}
              />
            ) : (
              <SinkEditor
                sink={item}
                onChange={(value) => patch(index, value)}
              />
            )}
          </div>
        ))}

        {flow.length === 0 ? (
          <p className="rounded-lg border border-dashed border-zinc-700 px-3 py-6 text-center text-[10px] text-zinc-500">
            Adicione a primeira etapa para montar o fluxo.
          </p>
        ) : null}

        <div className="flex flex-wrap gap-1.5">
          <Button variant="outline" size="sm" onClick={addStep}>
            <IconLabel icon={<Plus size={11} />}>Etapa</IconLabel>
          </Button>
          <Button variant="outline" size="sm" onClick={addCheckpoint}>
            <IconLabel icon={<Plus size={11} />}>Checkpoint</IconLabel>
          </Button>
          <Button variant="outline" size="sm" onClick={addSink}>
            <IconLabel icon={<Plus size={11} />}>Saída</IconLabel>
          </Button>
        </div>
      </section>
    </div>
  );
}

function TriggerEditor({
  trigger,
  index,
  onChange,
  onRemove,
}: {
  trigger: Dictionary;
  index: number;
  onChange: (value: Dictionary) => void;
  onRemove: () => void;
}) {
  const type = String(trigger.type ?? "MANUAL");
  const config = (trigger.config ?? {}) as Dictionary;

  function patchConfig(value: Dictionary) {
    onChange({ ...trigger, config: { ...config, ...value } });
  }

  return (
    <div className="rounded-lg border border-blue-500/20 bg-zinc-950/40 p-2">
      <div className="flex items-center gap-1">
        <select
          value={type}
          onChange={(event) =>
            onChange({ ...trigger, type: event.target.value, config: {} })
          }
          className={cn(inputClass, "flex-1")}
          aria-label={`Tipo do gatilho ${index + 1}`}
        >
          {TRIGGER_TYPES.map((item) => (
            <option key={item.value} value={item.value}>
              {item.label}
            </option>
          ))}
        </select>
        <FieldHint hint={triggerDescription(type)} />
        <Button
          variant="ghost"
          size="icon"
          className="h-7 w-7 text-red-300"
          onClick={onRemove}
          title="Remover gatilho"
        >
          <Trash2 size={11} />
        </Button>
      </div>
      <p className="mt-1 text-[9px] leading-snug text-zinc-500">{triggerDescription(type)}</p>
      <div className="mt-1.5 grid gap-1.5 sm:grid-cols-2">
        {type === "FOLDER_WATCH" || type === "RSYNC_FOLDER" ? (
          <PathInput
            value={String(config.path ?? "")}
            onChange={(value) => patchConfig({ path: value })}
            title="Escolher pasta monitorada"
            placeholder="input/dropzone ou caminho absoluto"
            hint="Se o caminho for relativo, ele é resolvido dentro do workspace."
            inputClassName="h-8 text-[10px]"
          />
        ) : null}
        {type === "CRON" ? <CronFields config={config} onChange={patchConfig} /> : null}
        {type === "IMAP_IDLE" ? (
          <>
            <LabeledField label="Caixa de entrada">
              <input
                value={String(config.mailbox ?? "INBOX")}
                onChange={(event) => patchConfig({ mailbox: event.target.value })}
                className={inputClass}
                placeholder="INBOX"
              />
            </LabeledField>
            <LabeledField label="Filtrar remetente">
              <input
                value={String(config.from_filter ?? "")}
                onChange={(event) => patchConfig({ from_filter: event.target.value })}
                className={inputClass}
                placeholder="cliente@exemplo.com"
              />
            </LabeledField>
          </>
        ) : null}
        {type === "SIGNAL_MSG" ? (
          <LabeledField label="Filtrar número">
            <input
              value={String(config.from_filter ?? "")}
              onChange={(event) => patchConfig({ from_filter: event.target.value })}
              className={inputClass}
              placeholder="+5511999999999"
            />
          </LabeledField>
        ) : null}
        {type === "WEBHOOK" ? (
          <>
            <LabeledField label="Segredo do webhook">
              <input
                value={String(config.secret ?? "")}
                onChange={(event) => patchConfig({ secret: event.target.value })}
                className={inputClass}
                placeholder="opcional — usa o segredo global quando vazio"
              />
            </LabeledField>
            <LabeledField label="Formato esperado">
              <input
                value={String(config.format ?? "json")}
                onChange={(event) => patchConfig({ format: event.target.value })}
                className={inputClass}
                placeholder="json"
              />
            </LabeledField>
          </>
        ) : null}
        {type === "SQL_POLL" ? (
          <>
            <LabeledField label="Consulta SQL">
              <input
                value={String(config.query ?? "")}
                onChange={(event) => patchConfig({ query: event.target.value })}
                className={inputClass}
                placeholder="SELECT ..."
              />
            </LabeledField>
            <LabeledField label="Intervalo (segundos)">
              <input
                type="number"
                min={15}
                value={Number(config.interval_seconds ?? 60)}
                onChange={(event) =>
                  patchConfig({ interval_seconds: Number(event.target.value) })
                }
                className={inputClass}
              />
            </LabeledField>
          </>
        ) : null}
      </div>
    </div>
  );
}

function CronFields({
  config,
  onChange,
}: {
  config: Dictionary;
  onChange: (value: Dictionary) => void;
}) {
  const cron = String(config.cron ?? "0 8 * * *");
  const preset = cron === "0 * * * *" ? "hourly" : cron.startsWith("0 ") ? "daily" : cron.startsWith("*/") ? "minutes" : "custom";
  return (
    <>
      <LabeledField label="Frequência">
        <select
          value={preset}
          onChange={(event) => {
            const value = event.target.value;
            if (value === "hourly") onChange({ cron: "0 * * * *" });
            else if (value === "daily") onChange({ cron: "0 8 * * *" });
            else if (value === "minutes") onChange({ cron: "*/15 * * * *" });
          }}
          className={inputClass}
        >
          <option value="hourly">A cada hora</option>
          <option value="daily">Todos os dias às 8h</option>
          <option value="minutes">A cada 15 minutos</option>
          <option value="custom">Personalizado</option>
        </select>
      </LabeledField>
      <LabeledField label="Expressão cron">
        <input
          value={cron}
          onChange={(event) => onChange({ cron: event.target.value })}
          className={inputClass}
          placeholder="0 8 * * *"
        />
      </LabeledField>
      <LabeledField label="Fuso horário">
        <input
          value={String(config.timezone ?? "America/Sao_Paulo")}
          onChange={(event) => onChange({ timezone: event.target.value })}
          className={inputClass}
          placeholder="America/Sao_Paulo"
        />
      </LabeledField>
    </>
  );
}

function StepEditor({
  step,
  workspaceId,
  onChange,
}: {
  step: Dictionary;
  workspaceId?: string | null;
  onChange: (value: Dictionary) => void;
}) {
  return (
    <div className="mt-1.5 space-y-1.5">
      <div className="grid gap-1.5 sm:grid-cols-3">
        <LabeledField label="Modo">
          <select
            value={String(step.mode ?? "build")}
            onChange={(event) => onChange({ mode: event.target.value })}
            className={inputClass}
          >
            <option value="build">build — executar</option>
            <option value="plan">plan — só planejar</option>
            <option value="audit">audit — revisar</option>
          </select>
        </LabeledField>
        <LabeledField label="Agente">
          <AgentCombobox
            value={String(step.agent ?? "")}
            onChange={(value) => onChange({ agent: value || null })}
            workspaceId={workspaceId}
            placeholder="automático (router)"
            className="[&_input]:h-8 [&_input]:text-[10px]"
          />
        </LabeledField>
        <LabeledField label="Modelo">
          <ModelCombobox
            value={String(step.model ?? "")}
            onChange={(value) => onChange({ model: value || null })}
            placeholder="automático"
          />
        </LabeledField>
      </div>
      <LabeledField label="Prompt da etapa">
        <textarea
          value={String(step.prompt ?? "")}
          onChange={(event) => onChange({ prompt: event.target.value })}
          rows={4}
          className="w-full rounded border border-zinc-800 bg-zinc-900/50 p-2 font-mono text-[10px] text-zinc-200 outline-none"
        />
      </LabeledField>
    </div>
  );
}

function HitlEditor({
  checkpoint,
  onChange,
}: {
  checkpoint: Dictionary;
  onChange: (value: Dictionary) => void;
}) {
  const type = String(checkpoint.interaction_type ?? "BINARY_GATE");
  const channels = Array.isArray(checkpoint.channels)
    ? (checkpoint.channels as string[])
    : [];
  return (
    <div className="mt-1.5 space-y-1.5">
      <div className="grid gap-1.5 sm:grid-cols-2">
        <LabeledField
          label="Tipo de interação"
          hint="Decide como o usuário responde ao checkpoint."
        >
          <select
            value={type}
            onChange={(event) => onChange({ interaction_type: event.target.value })}
            className={inputClass}
          >
            {HITL_TYPES.map((item) => (
              <option key={item.value} value={item.value}>
                {item.label}
              </option>
            ))}
          </select>
        </LabeledField>
        <LabeledField
          label="Tempo limite (min)"
          hint="Depois disso, a política de timeout é aplicada."
        >
          <input
            type="number"
            min={1}
            value={Number(checkpoint.timeout_minutes ?? 240)}
            onChange={(event) =>
              onChange({ timeout_minutes: Number(event.target.value) })
            }
            className={inputClass}
          />
        </LabeledField>
        <LabeledField label="Se o tempo acabar">
          <select
            value={String(checkpoint.on_timeout ?? "PAUSE_AND_ALERT")}
            onChange={(event) => onChange({ on_timeout: event.target.value })}
            className={inputClass}
          >
            {ON_TIMEOUT.map((item) => (
              <option key={item.value} value={item.value}>
                {item.label}
              </option>
            ))}
          </select>
        </LabeledField>
        <LabeledField label="Canais de aviso">
          <div className="flex flex-wrap gap-2 pt-1.5">
            {CHANNELS.map((channel) => (
              <label key={channel} className="flex items-center gap-1 text-[10px] text-zinc-400">
                <input
                  type="checkbox"
                  checked={channels.includes(channel)}
                  onChange={(event) =>
                    onChange({
                      channels: event.target.checked
                        ? [...channels, channel]
                        : channels.filter((item) => item !== channel),
                    })
                  }
                  className="accent-blue-500"
                />
                {channel}
              </label>
            ))}
          </div>
        </LabeledField>
      </div>
      {type === "MULTI_CHOICE" ? (
        <LabeledField
          label="Opções (separadas por vírgula)"
          hint="O usuário escolhe uma destas opções."
        >
          <input
            value={
              Array.isArray(checkpoint.options)
                ? (checkpoint.options as string[]).join(", ")
                : ""
            }
            onChange={(event) =>
              onChange({
                options: event.target.value
                  .split(",")
                  .map((value) => value.trim())
                  .filter(Boolean),
              })
            }
            className={inputClass}
            placeholder="aprovar, revisar, cancelar"
          />
        </LabeledField>
      ) : null}
      <LabeledField
        label="Mensagem do checkpoint"
        hint="Aceita variáveis como {{steps.passo-1.summary}}."
      >
        <textarea
          value={String(checkpoint.prompt_template ?? "")}
          onChange={(event) => onChange({ prompt_template: event.target.value })}
          rows={2}
          className="w-full rounded border border-zinc-800 bg-zinc-900/50 p-2 font-mono text-[10px] text-zinc-200 outline-none"
        />
      </LabeledField>
    </div>
  );
}

function SinkEditor({
  sink,
  onChange,
}: {
  sink: Dictionary;
  onChange: (value: Dictionary) => void;
}) {
  const config = (sink.config ?? {}) as Dictionary;
  return (
    <div className="mt-1.5 space-y-1.5">
      <div className="grid gap-1.5 sm:grid-cols-2">
        <LabeledField label="Tipo de saída">
          <select
            value={String(sink.sink_type ?? "DOC_COMPILER")}
            onChange={(event) => onChange({ sink_type: event.target.value })}
            className={inputClass}
          >
            {SINK_TYPES.map((item) => (
              <option key={item.value} value={item.value}>
                {item.label}
              </option>
            ))}
          </select>
        </LabeledField>
        <LabeledField
          label="Configuração (JSON)"
          hint="Caminhos e destinatários aceitam variáveis do fluxo."
        >
          <input
            value={JSON.stringify(config)}
            onChange={(event) => {
              try {
                onChange({ config: JSON.parse(event.target.value) });
              } catch {
                // Keep the last valid JSON while the user types.
              }
            }}
            className={cn(inputClass, "font-mono")}
            placeholder="{}"
          />
        </LabeledField>
      </div>
    </div>
  );
}

function LabeledField({
  label,
  hint,
  children,
}: {
  label: string;
  hint?: string;
  children: React.ReactNode;
}) {
  return (
    <label className="block space-y-1">
      <span className="inline-flex items-center text-[10px] text-zinc-400">
        {label}
        {hint ? <FieldHint hint={hint} side="trailing" /> : null}
      </span>
      {children}
    </label>
  );
}

const inputClass =
  "h-8 w-full rounded border border-zinc-800 bg-zinc-900/50 px-2 text-[10px] text-zinc-200 outline-none focus:border-blue-500/60";

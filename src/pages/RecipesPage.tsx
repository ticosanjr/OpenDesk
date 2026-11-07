import { Suspense, lazy, useCallback, useEffect, useMemo, useState } from "react";
import {
  ChevronDown,
  ChevronUp,
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
import { EmptyState } from "@/components/ui/empty-state";
import { apiGet, apiPost, apiPut } from "@/lib/api";
import { useAppStore } from "@/store/app-store";
import { toast } from "@/store/toast-store";
import { relativeTime } from "@/lib/format";
import { cn } from "@/lib/utils";

const MonacoEditor = lazy(() =>
  import("@monaco-editor/react").then((module) => ({ default: module.Editor })),
);

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

const DEFAULT_YAML = `id: nova-receita
name: Nova receita
enabled: true
trigger:
  type: MANUAL
  config: {}
steps:
  - id: passo-1
    name: Primeiro passo
    mode: build
    agent: build
    effort: low
    prompt: |
      Execute o primeiro passo e salve o resultado em {{run.output_dir}}.
hitl_checkpoints: []
sinks: []
`;

export function RecipesPage() {
  const workspaceId = useAppStore((state) => state.activeWorkspaceId);
  const [recipes, setRecipes] = useState<RecipeIndex[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [yaml, setYaml] = useState(DEFAULT_YAML);
  const [runs, setRuns] = useState<RecipeRun[]>([]);
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const recipe = useMemo(() => {
    try {
      return parseYaml(yaml) as Record<string, unknown> | null;
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
    void apiGet<{ yaml: string }>(`/api/recipes/${encodeURIComponent(selected)}`)
      .then((value) => setYaml(value.yaml))
      .catch((cause) => setError(cause instanceof Error ? cause.message : String(cause)));
  }, [selected]);

  function update(next: Record<string, unknown>) {
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
        description="YAML no workspace, execução pelo OpenCode, HITL por Signal/e-mail/UI e sinks determinísticos."
        actions={
          <>
            <Button variant="outline" size="sm" onClick={() => void load()} disabled={loading}>
              <RefreshCw size={13} className={loading ? "animate-spin" : undefined} />
              Atualizar
            </Button>
            <Button size="sm" onClick={() => void create()}>
              <Plus size={13} />
              Nova receita
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
            <Badge variant="default">{recipes.length}</Badge>
          </CardHeader>
          <CardContent className="max-h-[70dvh] space-y-1 overflow-y-auto p-2">
            {recipes.length === 0 ? (
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
                    "w-full rounded-lg px-2.5 py-2 text-left",
                    selected === item.id
                      ? "bg-zinc-800 text-zinc-100"
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
              <CardTitle className="text-[12px]">Visual Stepper</CardTitle>
              <div className="flex gap-1">
                <Button variant="outline" size="sm" className="h-7" onClick={() => void run(true)} disabled={!selected}>
                  <Sparkles size={12} />
                  Dry-run
                </Button>
                <Button size="sm" className="h-7" onClick={() => void run(false)} disabled={!selected}>
                  <Play size={12} />
                  Executar
                </Button>
              </div>
            </CardHeader>
            <CardContent className="max-h-[70dvh] space-y-2 overflow-y-auto">
              {recipe ? (
                <RecipeStepper recipe={recipe} onChange={update} />
              ) : (
                <p className="rounded-lg border border-red-500/20 bg-red-500/10 p-3 text-[11px] text-red-300">
                  YAML inválido. Corrija no editor ao lado.
                </p>
              )}
            </CardContent>
          </Card>

          <Card className="min-h-0">
            <CardHeader className="py-3">
              <CardTitle className="text-[12px]">YAML</CardTitle>
              <Button size="sm" className="h-7" onClick={() => void save()} disabled={!selected || saving}>
                {saving ? <Loader2 size={12} className="animate-spin" /> : <Save size={12} />}
                Salvar
              </Button>
            </CardHeader>
            <CardContent className="h-[70dvh] p-0">
              <Suspense fallback={<textarea value={yaml} onChange={(event) => setYaml(event.target.value)} className="h-full w-full bg-zinc-950 p-3 font-mono text-[11px] text-zinc-200 outline-none" />}>
                <MonacoEditor
                  height="100%"
                  language="yaml"
                  theme="vs-dark"
                  value={yaml}
                  onChange={(value) => setYaml(value ?? "")}
                  options={{ minimap: { enabled: false }, fontSize: 11, wordWrap: "on" }}
                />
              </Suspense>
            </CardContent>
          </Card>
        </div>
      </div>

      <Card>
        <CardHeader className="py-3">
          <CardTitle className="text-[12px]">Execuções de receitas</CardTitle>
          <Badge variant="default">{runs.length}</Badge>
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

function RecipeStepper({
  recipe,
  onChange,
}: {
  recipe: Record<string, unknown>;
  onChange: (next: Record<string, unknown>) => void;
}) {
  const trigger = (recipe.trigger ?? {}) as Record<string, unknown>;
  const triggerConfig = (trigger.config ?? {}) as Record<string, unknown>;
  const steps = Array.isArray(recipe.steps) ? (recipe.steps as Array<Record<string, unknown>>) : [];
  const checkpoints = Array.isArray(recipe.hitl_checkpoints)
    ? (recipe.hitl_checkpoints as Array<Record<string, unknown>>)
    : [];
  const sinks = Array.isArray(recipe.sinks) ? (recipe.sinks as Array<Record<string, unknown>>) : [];

  function setSteps(next: Array<Record<string, unknown>>) {
    onChange({ ...recipe, steps: next });
  }

  function setCheckpoints(next: Array<Record<string, unknown>>) {
    onChange({ ...recipe, hitl_checkpoints: next });
  }

  function setSinks(next: Array<Record<string, unknown>>) {
    onChange({ ...recipe, sinks: next });
  }

  function move(index: number, direction: -1 | 1) {
    const next = [...steps];
    const target = index + direction;
    if (target < 0 || target >= next.length) return;
    [next[index], next[target]] = [next[target], next[index]];
    setSteps(next);
  }

  return (
    <div className="relative space-y-2 border-l-2 border-zinc-800 pl-4">
      <div className="rounded-lg border border-blue-500/30 bg-blue-500/[0.04] p-2.5">
        <p className="text-[10px] font-semibold uppercase tracking-wide text-blue-200">Gatilho</p>
        <div className="mt-1 grid gap-2 sm:grid-cols-2">
          <input
            value={(recipe.name as string) ?? ""}
            onChange={(event) => onChange({ ...recipe, name: event.target.value })}
            className={inputClass}
            placeholder="nome"
          />
          <select
            value={(trigger.type as string) ?? "MANUAL"}
            onChange={(event) => onChange({ ...recipe, trigger: { ...trigger, type: event.target.value } })}
            className={inputClass}
          >
            {["MANUAL", "FOLDER_WATCH", "RSYNC_FOLDER", "IMAP_IDLE", "WEBHOOK", "SQL_POLL", "CRON", "SIGNAL_MSG"].map((type) => (
              <option key={type} value={type}>{type}</option>
            ))}
          </select>
          {(trigger.type === "FOLDER_WATCH" || trigger.type === "RSYNC_FOLDER") ? (
            <input
              value={(triggerConfig.path as string) ?? "input/dropzone"}
              onChange={(event) => onChange({ ...recipe, trigger: { ...trigger, config: { ...triggerConfig, path: event.target.value } } })}
              className={inputClass}
              placeholder="input/dropzone"
            />
          ) : null}
        </div>
      </div>

      {steps.map((step, index) => (
        <div key={index} className="rounded-lg border border-zinc-800 bg-zinc-950/40 p-2.5">
          <div className="flex items-center gap-1">
            <input
              value={(step.name as string) ?? ""}
              onChange={(event) => setSteps(steps.map((item, itemIndex) => itemIndex === index ? { ...item, name: event.target.value } : item))}
              className={cn(inputClass, "flex-1")}
            />
            <Button variant="ghost" size="icon" className="h-7 w-7" onClick={() => move(index, -1)}><ChevronUp size={11} /></Button>
            <Button variant="ghost" size="icon" className="h-7 w-7" onClick={() => move(index, 1)}><ChevronDown size={11} /></Button>
            <Button variant="ghost" size="icon" className="h-7 w-7 text-red-300" onClick={() => setSteps(steps.filter((_, itemIndex) => itemIndex !== index))}><Trash2 size={11} /></Button>
          </div>
          <div className="mt-1 grid gap-1 sm:grid-cols-3">
            <select value={(step.mode as string) ?? "build"} onChange={(event) => setSteps(steps.map((item, itemIndex) => itemIndex === index ? { ...item, mode: event.target.value } : item))} className={inputClass}>
              <option value="build">build</option>
              <option value="plan">plan</option>
            </select>
            <input value={(step.agent as string) ?? ""} onChange={(event) => setSteps(steps.map((item, itemIndex) => itemIndex === index ? { ...item, agent: event.target.value } : item))} className={inputClass} placeholder="@agente" />
            <input value={(step.model as string) ?? ""} onChange={(event) => setSteps(steps.map((item, itemIndex) => itemIndex === index ? { ...item, model: event.target.value } : item))} className={inputClass} placeholder="modelo" />
          </div>
          <textarea
            value={(step.prompt as string) ?? ""}
            onChange={(event) => setSteps(steps.map((item, itemIndex) => itemIndex === index ? { ...item, prompt: event.target.value } : item))}
            rows={4}
            className="mt-1 w-full rounded border border-zinc-800 bg-zinc-900/50 p-2 font-mono text-[10px] text-zinc-200 outline-none"
          />
        </div>
      ))}
      <Button
        variant="outline"
        size="sm"
        className="w-full"
        onClick={() => setSteps([...steps, { id: `passo-${steps.length + 1}`, name: `Passo ${steps.length + 1}`, mode: "build", prompt: "Descreva o passo." }])}
      >
        <Plus size={12} /> Adicionar step
      </Button>

      {checkpoints.map((checkpoint, index) => (
        <div key={index} className="rounded-lg border border-amber-500/30 bg-amber-500/[0.04] p-2.5">
          <div className="flex items-center gap-1">
            <p className="flex-1 text-[10px] font-semibold uppercase tracking-wide text-amber-200">Checkpoint HITL</p>
            <Button variant="ghost" size="icon" className="h-7 w-7 text-red-300" onClick={() => setCheckpoints(checkpoints.filter((_, itemIndex) => itemIndex !== index))}><Trash2 size={11} /></Button>
          </div>
          <div className="mt-1 grid gap-1 sm:grid-cols-2">
            <select
              value={(checkpoint.interaction_type as string) ?? "BINARY_GATE"}
              onChange={(event) => setCheckpoints(checkpoints.map((item, itemIndex) => itemIndex === index ? { ...item, interaction_type: event.target.value } : item))}
              className={inputClass}
            >
              {["BINARY_GATE", "MULTI_CHOICE", "APPROVAL_WITH_FEEDBACK", "ARTIFACT_EDIT_HANDOFF", "TOOL_PERMISSION_PROXY"].map((type) => <option key={type} value={type}>{type}</option>)}
            </select>
            <select
              value={(checkpoint.after_step as string) ?? steps[0]?.id ?? ""}
              onChange={(event) => setCheckpoints(checkpoints.map((item, itemIndex) => itemIndex === index ? { ...item, after_step: event.target.value } : item))}
              className={inputClass}
            >
              {steps.map((step) => <option key={String(step.id)} value={String(step.id)}>após {String(step.name)}</option>)}
            </select>
            <input
              type="number"
              value={(checkpoint.timeout_minutes as number) ?? 240}
              onChange={(event) => setCheckpoints(checkpoints.map((item, itemIndex) => itemIndex === index ? { ...item, timeout_minutes: Number(event.target.value) } : item))}
              className={inputClass}
            />
          </div>
        </div>
      ))}
      <Button
        variant="outline"
        size="sm"
        className="w-full"
        onClick={() => setCheckpoints([...checkpoints, { id: `hitl-${checkpoints.length + 1}`, after_step: steps[0]?.id ?? "passo-1", interaction_type: "BINARY_GATE", channels: ["DESKTOP_UI"], timeout_minutes: 240, on_timeout: "PAUSE_AND_ALERT", prompt_template: "Aprovar a execução?" }])}
      >
        <Plus size={12} /> Adicionar checkpoint HITL
      </Button>

      {sinks.map((sink, index) => (
        <div key={index} className="rounded-lg border border-emerald-500/30 bg-emerald-500/[0.04] p-2.5">
          <div className="flex items-center gap-1">
            <p className="flex-1 text-[10px] font-semibold uppercase tracking-wide text-emerald-200">Sink</p>
            <Button variant="ghost" size="icon" className="h-7 w-7 text-red-300" onClick={() => setSinks(sinks.filter((_, itemIndex) => itemIndex !== index))}><Trash2 size={11} /></Button>
          </div>
          <div className="mt-1 grid gap-1 sm:grid-cols-2">
            <select
              value={(sink.type as string) ?? "DOC_COMPILER"}
              onChange={(event) => setSinks(sinks.map((item, itemIndex) => itemIndex === index ? { ...item, type: event.target.value } : item))}
              className={inputClass}
            >
              {["DOC_COMPILER", "EMAIL_SMTP", "SIGNAL_MSG", "RSYNC_FS", "GIT_COMMIT", "SQL_INSERT", "TELEGRAM_MSG"].map((type) => <option key={type} value={type}>{type}</option>)}
            </select>
            <input
              value={JSON.stringify(sink.config ?? {})}
              onChange={(event) => {
                try {
                  setSinks(sinks.map((item, itemIndex) => itemIndex === index ? { ...item, config: JSON.parse(event.target.value) } : item));
                } catch {
                  // Keep the last valid JSON while the user types.
                }
              }}
              className={inputClass}
              placeholder="config JSON"
            />
          </div>
        </div>
      ))}
      <Button
        variant="outline"
        size="sm"
        className="w-full"
        onClick={() => setSinks([...sinks, { type: "DOC_COMPILER", config: { format: "pdf" } }])}
      >
        <Plus size={12} /> Adicionar sink
      </Button>
    </div>
  );
}

const inputClass =
  "h-8 w-full rounded border border-zinc-800 bg-zinc-900/50 px-2 text-[10px] text-zinc-200 outline-none focus:border-blue-500/60";

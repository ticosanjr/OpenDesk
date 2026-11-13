import { useCallback, useEffect, useState } from "react";
import {
  GitBranch,
  Loader2,
  Play,
  Plus,
  RefreshCw,
  Save,
  Sparkles,
  Workflow,
} from "lucide-react";
import { useSearchParams } from "react-router-dom";
import { parse as parseYaml, stringify as stringifyYaml } from "yaml";
import { PageBody, PageHeader } from "@/components/layout/PageHeader";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Modal } from "@/components/ui/modal";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { CountPill } from "@/components/ui/count-pill";
import { EmptyState } from "@/components/ui/empty-state";
import { FieldHint } from "@/components/ui/field-hint";
import { IconLabel } from "@/components/ui/icon-label";
import { LoadingState } from "@/components/ui/loading-state";
import { Switch } from "@/components/ui/switch";
import { PromptAttachments } from "@/components/ui/prompt-attachments";
import { apiDelete, apiGet, apiPost, apiPut } from "@/lib/api";
import { useAppStore } from "@/store/app-store";
import { confirmAction } from "@/store/confirm-store";
import { toast } from "@/store/toast-store";
import { relativeTime } from "@/lib/format";
import { cn } from "@/lib/utils";
import {
  LocalYamlEditor,
  RecipeStepper,
  type Dictionary,
} from "@/components/recipes/recipe-editors";

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
enabled: false
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


function recipeRunStatusLabel(status: string): string {
  if (status === "COMPLETED") return "concluída";
  if (status === "FAILED") return "falhou";
  if (status === "REJECTED") return "rejeitada";
  if (status === "RUNNING") return "executando";
  if (status.includes("WAITING")) return "aguardando";
  return status.toLowerCase();
}

export function RecipesPage() {
  const workspaceId = useAppStore((state) => state.activeWorkspaceId);
  const [params] = useSearchParams();
  const workspaceQuery = workspaceId
    ? `?workspace_id=${encodeURIComponent(workspaceId)}`
    : "";
  const focusRecipe = params.get("recipe");
  const [recipes, setRecipes] = useState<RecipeIndex[]>([]);
  const [selected, setSelected] = useState<string | null>(focusRecipe);
  const [yaml, setYaml] = useState(DEFAULT_YAML);
  const [runs, setRuns] = useState<RecipeRun[]>([]);
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [toggling, setToggling] = useState<string | null>(null);
  const [editorLoading, setEditorLoading] = useState(false);
  const [runMode, setRunMode] = useState<"dry" | "run" | null>(null);
  const [runContext, setRunContext] = useState("");
  const [runAttachments, setRunAttachments] = useState<string[]>([]);
  const [running, setRunning] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const [recipe, setRecipe] = useState<Dictionary | null>(null);

  // Parse with a short debounce: typing in the YAML editor no longer
  // re-renders the whole visual stepper on every keystroke.
  useEffect(() => {
    const handle = window.setTimeout(() => {
      try {
        const parsed = parseYaml(yaml);
        setRecipe(
          parsed && typeof parsed === "object" ? (parsed as Dictionary) : null,
        );
      } catch {
        setRecipe(null);
      }
    }, 200);
    return () => window.clearTimeout(handle);
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
    if (focusRecipe) setSelected(focusRecipe);
  }, [focusRecipe]);

  useEffect(() => {
    if (!selected) return;
    let cancelled = false;
    setEditorLoading(true);
    void apiGet<{ yaml: string }>(`/api/recipes/${encodeURIComponent(selected)}${workspaceQuery}`)
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
  }, [selected, workspaceQuery]);

  function update(next: Dictionary) {
    setYaml(stringifyYaml(next));
    // Visual edits are committed immediately; the debounce only protects
    // parsing while the user types raw YAML.
    setRecipe(next);
  }

  async function toggleRecipe(item: RecipeIndex) {
    if (!item.is_enabled) {
      const confirmed = await confirmAction({
        title: `Ativar a receita “${item.name}”?`,
        description:
          "Os gatilhos automáticos desta receita passarão a disparar execuções. Revise o fluxo e os checkpoints antes de continuar.",
        confirmLabel: "Ativar receita",
      });
      if (!confirmed) return;
    }
    setToggling(item.id);
    try {
      await apiPost(`/api/recipes/${encodeURIComponent(item.id)}/enabled${workspaceQuery}`, {
        enabled: !item.is_enabled,
      });
      await load();
      if (selected === item.id) {
        const value = await apiGet<{ yaml: string }>(
          `/api/recipes/${encodeURIComponent(item.id)}${workspaceQuery}`,
        );
        setYaml(value.yaml);
      }
      toast({
        tone: "success",
        title: item.is_enabled ? "Receita desativada" : "Receita ativada",
        description: item.name,
      });
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setToggling(null);
    }
  }

  async function save() {
    if (!recipe || !selected) return;
    setSaving(true);
    try {
      await apiPut(`/api/recipes/${encodeURIComponent(selected)}${workspaceQuery}`, { yaml });
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
      await apiPut(`/api/recipes/${id}${workspaceQuery}`, { yaml: text });
      await load();
      setSelected(id);
      toast({ tone: "success", title: "Receita criada" });
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  }

  function closeRun() {
    if (running) return;
    for (const path of runAttachments) {
      if (!workspaceId) break;
      void apiDelete(
        `/api/attachments?workspace_id=${encodeURIComponent(workspaceId)}&path=${encodeURIComponent(path)}`,
      ).catch(() => {
        // Best effort; an orphan file is harmless.
      });
    }
    setRunAttachments([]);
    setRunMode(null);
  }

  async function run(dryRun: boolean, context: string) {
    if (!selected) return;
    setRunning(true);
    try {
      const attachmentBlock =
        runAttachments.length > 0
          ? `Arquivos anexados:\n${runAttachments
              .map((path) => `- ${path}`)
              .join("\n")}`
          : "";
      const text = [context.trim(), attachmentBlock].filter(Boolean).join("\n\n");
      const run = await apiPost<RecipeRun>(`/api/recipes/${selected}/run${workspaceQuery}`, {
        dry_run: dryRun,
        trigger_context: {
          manual: true,
          context: text,
          prompt: text,
          question: text,
          attachments: runAttachments,
        },
      });
      toast({
        tone: "success",
        title: dryRun ? "Dry-run iniciado" : "Receita iniciada",
        description: `run ${run.id.slice(0, 8)}`,
      });
      setRunMode(null);
      setRunContext("");
      setRunAttachments([]);
      await load();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setRunning(false);
    }
  }

  return (
    <PageBody>
      <PageHeader
        title="Receitas"
        description="Monte o fluxo visualmente, use zero ou vários gatilhos e combine etapas, checkpoints e saídas na ordem que fizer sentido. Novas receitas nascem desligadas; ative depois de revisar."
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
        <p className="rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[13px] text-red-300">
          {error}
        </p>
      ) : null}

      <div className="grid min-h-0 gap-3 lg:grid-cols-[260px_1fr]">
        <Card className="min-h-0">
          <CardHeader className="py-3">
            <CardTitle className="flex items-center gap-2">
              <Workflow size={15} className="text-zinc-400" />
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
                compact
                className="border-0 bg-transparent"
              />
            ) : (
              recipes.map((item) => (
                <div
                  key={item.id}
                  className={cn(
                    "flex items-start gap-2 rounded-lg px-3 py-2.5 transition-colors",
                    selected === item.id
                      ? "workspace-nav-active"
                      : "hover:bg-zinc-900",
                    item.is_enabled ? undefined : "bg-zinc-950/20",
                  )}
                >
                  <button
                    type="button"
                    onClick={() => setSelected(item.id)}
                    className={cn(
                      "min-w-0 flex-1 text-left",
                      selected === item.id ? "text-zinc-100" : "text-zinc-400",
                    )}
                  >
                    <p className="truncate text-[13.5px] font-medium">{item.name}</p>
                    <p className="truncate font-mono text-[13px] text-zinc-400">
                      {item.trigger_type} · {item.total_runs} execuções
                    </p>
                    {item.last_error ? (
                      <p className="truncate text-[13px] text-red-300">{item.last_error}</p>
                    ) : null}
                  </button>
                  <Switch
                    checked={item.is_enabled}
                    onCheckedChange={() => void toggleRecipe(item)}
                    disabled={toggling === item.id}
                    label={
                      item.is_enabled
                        ? `Desativar receita ${item.name}`
                        : `Ativar receita ${item.name}`
                    }
                    className="mt-0.5"
                  />
                </div>
              ))
            )}
          </CardContent>
        </Card>

        <div className="grid min-h-0 gap-3 xl:grid-cols-2">
          <Card className="min-h-0">
            <CardHeader className="py-3">
              <CardTitle className="flex items-center gap-2">
                <GitBranch size={15} className="text-zinc-400" />
                Fluxo visual
              </CardTitle>
              <div className="flex gap-1">
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => setRunMode("dry")}
                  disabled={!selected}
                >
                  <IconLabel icon={<Sparkles size={12} />}>Dry-run</IconLabel>
                </Button>
                <Button
                  size="sm"
                  onClick={() => setRunMode("run")}
                  disabled={!selected}
                >
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
                <p className="rounded-lg border border-red-500/20 bg-red-500/10 p-3 text-[13px] text-red-300">
                  YAML inválido. Corrija no editor ao lado.
                </p>
              )}
            </CardContent>
          </Card>

          <Card className="min-h-0">
            <CardHeader className="py-3">
              <CardTitle className="flex items-center gap-2">
                YAML
                <FieldHint hint="Editor simples local. Sempre abre offline e nunca fica carregando." />
              </CardTitle>
              <Button size="sm" onClick={() => void save()} disabled={!selected || saving}>
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
          <CardTitle>Execuções de receitas</CardTitle>
          <CountPill value={runs.length} />
        </CardHeader>
        <CardContent className="space-y-1">
          {runs.length === 0 ? (
            <p className="p-2 text-[13px] text-zinc-500">Nenhuma execução ainda.</p>
          ) : (
            runs.map((run) => (
              <div key={run.id} className="flex flex-wrap items-center gap-2 rounded-lg border border-zinc-800/70 bg-zinc-950/30 px-3 py-2">
                <Badge variant={run.status === "COMPLETED" ? "green" : run.status.includes("WAITING") ? "yellow" : run.status.includes("FAILED") || run.status === "REJECTED" ? "red" : "blue"}>
                  {recipeRunStatusLabel(run.status)}
                </Badge>
                <span className="font-mono text-[13px] text-zinc-400">{run.recipe_id}</span>
                <span className="min-w-0 flex-1 truncate font-mono text-[13px] text-zinc-500">{run.current_step_id ?? "—"}</span>
                <span className="text-[13px] text-zinc-500">{relativeTime(run.started_at)}</span>
              </div>
            ))
          )}
        </CardContent>
      </Card>

      <Modal
        open={runMode !== null}
        onClose={closeRun}
        title={runMode === "dry" ? "Dry-run da receita" : "Executar receita"}
        description="O contexto abaixo fica disponível como {{trigger.context}}, {{trigger.prompt}} e {{trigger.question}} nos prompts."
        size="md"
      >
        <div className="space-y-3">
          <label className="block">
            <span className="mb-1.5 block text-[12px] font-medium uppercase tracking-[0.12em] text-zinc-500">
              Contexto / pergunta (opcional)
            </span>
            <textarea
              value={runContext}
              onChange={(event) => setRunContext(event.target.value)}
              rows={6}
              placeholder="Ex.: analise o contrato em juridico/contratos/atual.pdf"
              className="w-full resize-y rounded-lg border border-zinc-800 bg-zinc-900/50 p-2.5 font-mono text-[13px] text-zinc-100 outline-none placeholder:text-zinc-500 focus:border-blue-500/60"
            />
            <PromptAttachments
              workspaceId={workspaceId}
              value={runAttachments}
              onChange={setRunAttachments}
              className="mt-2"
            />
          </label>
          <p className="text-[12px] leading-snug text-zinc-500">
            Para anexar arquivos, coloque-os na pasta de entrada do pacote e
            referencie o caminho no contexto. O dry-run não executa ferramentas
            externas.
          </p>
          <div className="flex justify-end gap-2 border-t border-zinc-800/70 pt-3">
            <Button
              variant="ghost"
              size="sm"
              onClick={closeRun}
              disabled={running}
            >
              Cancelar
            </Button>
            <Button
              size="sm"
              onClick={() => void run(runMode === "dry", runContext)}
              disabled={running}
            >
              <IconLabel
                icon={
                  running ? (
                    <Loader2 size={13} className="animate-spin" />
                  ) : (
                    <Play size={13} />
                  )
                }
              >
                {runMode === "dry" ? "Iniciar dry-run" : "Executar agora"}
              </IconLabel>
            </Button>
          </div>
        </div>
      </Modal>
    </PageBody>
  );
}

import { useCallback, useEffect, useState } from "react";
import type { CSSProperties } from "react";
import { History, Plus, RefreshCw, Sparkles, Workflow } from "lucide-react";
import { useNavigate, useSearchParams } from "react-router-dom";
import { parse as parseYaml, stringify as stringifyYaml } from "yaml";
import { PageBody, PageHeader } from "@/components/layout/PageHeader";
import { Card, CardContent } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/ui/empty-state";
import { Modal } from "@/components/ui/modal";
import { IconLabel } from "@/components/ui/icon-label";
import { apiDelete, apiGet, apiPost, apiPut } from "@/lib/api";
import { useAppStore } from "@/store/app-store";
import { confirmAction } from "@/store/confirm-store";
import { toast } from "@/store/toast-store";
import type { Dictionary } from "@/components/recipes/recipe-editors";
import { RecipeListCard } from "./recipes/RecipeListCard";
import { RecipeFlowCard } from "./recipes/RecipeFlowCard";
import { YamlPanel } from "./recipes/YamlPanel";
import { RecipeRunsCard } from "./recipes/RecipeRunsCard";
import { RecipeRunModal } from "./recipes/RecipeRunModal";
import { RecipeVersionsDrawer } from "@/components/recipes/editors/RecipeVersionsDrawer";
import {
  DEFAULT_YAML,
  type RecipeIndex,
  type RecipeRun,
} from "./recipes/recipe-shared";

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
  const [yamlOpen, setYamlOpen] = useState(true);
  const [deleteConfirm, setDeleteConfirm] = useState<string | null>(null);
  const [yamlWidth, setYamlWidth] = useState(() => {
    const saved = Number(window.localStorage.getItem("opendesk.recipe.yamlWidth"));
    return Number.isFinite(saved) && saved >= 320 && saved <= 900 ? saved : 460;
  });
  const [resizing, setResizing] = useState(false);
  const [versionsOpen, setVersionsOpen] = useState(false);
  const [aiOpen, setAiOpen] = useState(false);
  const [aiPrompt, setAiPrompt] = useState("");
  const [aiBusy, setAiBusy] = useState(false);
  const navigate = useNavigate();

  const [recipe, setRecipe] = useState<Dictionary | null>(null);
  const selectedRecipe = recipes.find((item) => item.id === selected) ?? null;

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
      setSelected((current) =>
        current && recipeResult.recipes.some((item) => item.id === current)
          ? current
          : null,
      );
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
    setDeleteConfirm(null);
  }, [selected]);

  useEffect(() => {
    if (!resizing) return;
    function move(event: PointerEvent) {
      const max = Math.min(900, Math.max(320, window.innerWidth - 420));
      setYamlWidth(Math.min(max, Math.max(320, window.innerWidth - event.clientX - 28)));
    }
    function stop() {
      setResizing(false);
    }
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", stop, { once: true });
    return () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", stop);
    };
  }, [resizing]);

  useEffect(() => {
    window.localStorage.setItem("opendesk.recipe.yamlWidth", String(yamlWidth));
  }, [yamlWidth]);

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

  async function removeRecipe(item: RecipeIndex) {
    if (deleteConfirm !== item.id) {
      setDeleteConfirm(item.id);
      window.setTimeout(() => {
        setDeleteConfirm((current) => (current === item.id ? null : current));
      }, 4000);
      return;
    }
    setDeleteConfirm(null);
    try {
      await apiDelete(
        `/api/recipes/${encodeURIComponent(item.id)}${workspaceQuery}`,
      );
      if (selected === item.id) setSelected(null);
      await load();
      toast({ tone: "success", title: "Receita excluída", description: item.name });
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
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

  async function generateWithAi() {
    if (!workspaceId || !aiPrompt.trim()) return;
    setAiBusy(true);
    try {
      const thread = await apiPost<{ id: string }>("/api/chat/threads", {
        workspace_id: workspaceId,
        title: `Receita: ${aiPrompt.trim().slice(0, 48)}`,
        agent: "build",
        model: null,
        source: "desktop",
      });
      void apiPost(`/api/chat/threads/${thread.id}/messages`, {
        content: `/opendesk-receita ${aiPrompt.trim()}`,
      }).catch((cause) => {
        toast({
          tone: "error",
          title: "A conversa abriu, mas o comando falhou",
          description: cause instanceof Error ? cause.message : String(cause),
        });
      });
      setAiOpen(false);
      setAiPrompt("");
      navigate(`/chat?thread=${thread.id}`);
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível iniciar a receita com IA",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setAiBusy(false);
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
            <Button
              variant="outline"
              size="sm"
              disabled={!workspaceId}
              onClick={() => setAiOpen(true)}
            >
              <IconLabel icon={<Sparkles size={13} />}>Gerar com IA</IconLabel>
            </Button>
            <Button
              variant="outline"
              size="sm"
              disabled={!selected}
              onClick={() => setVersionsOpen(true)}
            >
              <IconLabel icon={<History size={13} />}>Versões</IconLabel>
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
        <RecipeListCard
          recipes={recipes}
          selected={selected}
          loading={loading}
          toggling={toggling}
          onSelect={setSelected}
          onToggle={(item) => void toggleRecipe(item)}
        />

        {!selected ? (
          <Card className="min-h-[320px]">
            <CardContent className="flex h-full items-center justify-center p-6">
              <EmptyState
                icon={Workflow}
                title="Crie ou selecione uma receita"
                description="O fluxo visual, o editor YAML e as ações de execução aparecem apenas quando uma receita está aberta."
                action={
                  <Button size="sm" onClick={() => void create()}>
                    <Plus size={13} />
                    Criar nova receita
                  </Button>
                }
              />
            </CardContent>
          </Card>
        ) : (
        <div
          className="flex min-h-0 flex-col gap-3 xl:flex-row"
          style={{ "--yaml-width": `${yamlWidth}px` } as CSSProperties}
        >
          <RecipeFlowCard
            recipe={recipe}
            workspaceId={workspaceId}
            editorLoading={editorLoading}
            deleteConfirm={deleteConfirm}
            selected={selected}
            canDelete={!!selectedRecipe}
            onChange={update}
            onDelete={() => {
              if (selectedRecipe) void removeRecipe(selectedRecipe);
            }}
            onDryRun={() => setRunMode("dry")}
            onRun={() => setRunMode("run")}
          />

          <YamlPanel
            open={yamlOpen}
            yaml={yaml}
            canSave={!!selected}
            saving={saving}
            onChange={setYaml}
            onSave={() => void save()}
            onOpen={() => setYamlOpen(true)}
            onClose={() => setYamlOpen(false)}
            onResizeStart={() => setResizing(true)}
            onResetWidth={() => setYamlWidth(460)}
          />
        </div>
        )}
      </div>

      <RecipeRunsCard runs={runs} />

      <Modal
        open={aiOpen}
        onClose={() => setAiOpen(false)}
        title="Gerar receita com IA"
        description="Descreva o objetivo. O comando /opendesk-receita abre uma conversa no agente build; revise o YAML antes de ligar."
        size="md"
      >
        <div className="space-y-3">
          <textarea
            value={aiPrompt}
            onChange={(event) => setAiPrompt(event.target.value)}
            rows={5}
            placeholder="Ex.: toda segunda, ler o feed de notícias, resumir em 5 itens e enviar no Telegram."
            className="w-full rounded-lg border border-zinc-800 bg-zinc-900/50 p-2 text-[13px] text-zinc-100 outline-none focus:border-blue-500/60"
          />
          <div className="flex justify-end gap-2">
            <Button variant="ghost" size="sm" onClick={() => setAiOpen(false)}>
              Cancelar
            </Button>
            <Button
              size="sm"
              disabled={aiBusy || !aiPrompt.trim()}
              onClick={() => void generateWithAi()}
            >
              {aiBusy ? "Abrindo..." : "Abrir conversa"}
            </Button>
          </div>
        </div>
      </Modal>

      <RecipeVersionsDrawer
        recipeId={selected}
        workspaceId={workspaceId ?? ""}
        open={versionsOpen}
        onClose={() => setVersionsOpen(false)}
        onRestored={() => void load()}
      />

      <RecipeRunModal
        open={runMode !== null}
        mode={runMode}
        context={runContext}
        workspaceId={workspaceId}
        attachments={runAttachments}
        running={running}
        onContextChange={setRunContext}
        onAttachmentsChange={setRunAttachments}
        onClose={closeRun}
        onRun={() => void run(runMode === "dry", runContext)}
      />
    </PageBody>
  );
}

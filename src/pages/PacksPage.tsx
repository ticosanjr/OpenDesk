import { useCallback, useEffect, useMemo, useState } from "react";
import {
  CheckCircle2,
  FolderPlus,
  RefreshCw,
  Sparkles,
} from "lucide-react";
import { PageBody, PageHeader } from "@/components/layout/PageHeader";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { IconLabel } from "@/components/ui/icon-label";
import { ExternalResources } from "@/components/packs/ExternalResources";
import { RecommendedSkills } from "@/components/packs/RecommendedSkills";
import { PackInstallModal } from "@/components/packs/PackInstallModal";
import { PackCatalog } from "@/components/packs/PackCatalog";
import { PacksToolbar } from "@/components/packs/PacksToolbar";
import { WorkspaceTemplateCard } from "@/components/packs/WorkspaceTemplateCard";
import { WorkspaceTemplateModal } from "@/components/packs/WorkspaceTemplateModal";
import type {
  Category,
  InstallPlan,
  InstallResult,
  ItemInstallResult,
  PackItem,
  PackSummary,
  WorkspaceTemplate,
} from "@/components/packs/pack-types";
import { apiDelete, apiGet, apiPost } from "@/lib/api";
import { useFavorites } from "@/lib/favorites";
import type { RecommendedSkill } from "@/lib/recommended-skills";
import { buildInstallTask } from "@/lib/install-task";
import type { Task } from "@/lib/types";
import { links } from "@/lib/links";
import { confirmAction } from "@/store/confirm-store";
import { toast } from "@/store/toast-store";
import { useAppStore } from "@/store/app-store";
import { useUiStore } from "@/store/ui-store";

const ONBOARDING_KEY = "opendesk.starter-packs.onboarding-dismissed";

export function PacksPage() {
  const activeWorkspaceId = useAppStore((state) => state.activeWorkspaceId);
  const workspaces = useAppStore((state) => state.workspaces);
  const refresh = useAppStore((state) => state.refresh);
  const [packs, setPacks] = useState<PackSummary[]>([]);
  const [categories, setCategories] = useState<Category[]>([]);
  const [templates, setTemplates] = useState<WorkspaceTemplate[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [category, setCategory] = useState("all");
  const [installFilter, setInstallFilter] = useState<
    "all" | "installed" | "available" | "favorites"
  >("all");
  const [tab, setTab] = useState<
    "all" | "workspaces" | "packs" | "resources" | "recommended"
  >("all");
  const { toggle: toggleFavorite, isFavorite } = useFavorites();
  const setWorkspaceDialogOpen = useUiStore(
    (state) => state.setWorkspaceDialogOpen,
  );
  const [selected, setSelected] = useState<PackSummary | null>(null);
  const [plan, setPlan] = useState<InstallPlan | null>(null);
  const [planLoading, setPlanLoading] = useState(false);
  const [items, setItems] = useState<PackItem[]>([]);
  const [itemsLoading, setItemsLoading] = useState(false);
  const [installingItem, setInstallingItem] = useState<string | null>(null);
  const [installing, setInstalling] = useState(false);
  const [overwritePaths, setOverwritePaths] = useState<Set<string>>(new Set());
  const [removing, setRemoving] = useState(false);
  const [templateOpen, setTemplateOpen] = useState<WorkspaceTemplate | null>(null);
  const [templateName, setTemplateName] = useState("");
  const [templateRoot, setTemplateRoot] = useState("");
  const [templateColor, setTemplateColor] = useState("#3b82f6");
  const [creatingTemplate, setCreatingTemplate] = useState(false);
  const [templateErrors, setTemplateErrors] = useState<string[]>([]);
  const [managing, setManaging] = useState(false);
  const [selectedPacks, setSelectedPacks] = useState<Set<string>>(new Set());
  const [updatingAll, setUpdatingAll] = useState(false);
  const [onboardingDismissed, setOnboardingDismissed] = useState(() => {
    try {
      return localStorage.getItem(ONBOARDING_KEY) === "1";
    } catch {
      return false;
    }
  });

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const suffix = activeWorkspaceId
        ? `?workspace_id=${encodeURIComponent(activeWorkspaceId)}`
        : "";
      const [catalog, templateList] = await Promise.all([
        apiGet<{ categories: Category[]; packs: PackSummary[] }>(
          `/api/starter-packs${suffix}`,
        ),
        apiGet<WorkspaceTemplate[]>("/api/starter-packs/templates"),
      ]);
      setCategories(catalog.categories);
      setPacks(catalog.packs);
      setTemplates(templateList);
      setError(null);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setLoading(false);
    }
  }, [activeWorkspaceId]);

  useEffect(() => {
    void load();
  }, [load]);

  useEffect(() => {
    // Selections are workspace-scoped: switching the active workspace must
    // never carry pack ids into another workspace's removal request.
    setManaging(false);
    setSelectedPacks(new Set());
  }, [activeWorkspaceId]);

  const installedCount = packs.filter((pack) => pack.installed).length;

  const filtered = useMemo(() => {
    const needle = query.trim().toLowerCase();
    return packs.filter((pack) => {
      if (category !== "all" && pack.category !== category) return false;
      if (installFilter === "installed" && !pack.installed) return false;
      if (installFilter === "available" && pack.installed) return false;
      if (installFilter === "favorites" && !isFavorite("pack", pack.id)) {
        return false;
      }
      if (!needle) return true;
      return `${pack.name} ${pack.description} ${pack.id}`
        .toLowerCase()
        .includes(needle);
    });
  }, [category, installFilter, isFavorite, packs, query]);

  const favoritesOnly = installFilter === "favorites";

  const filteredTemplates = useMemo(() => {
    const needle = query.trim().toLowerCase();
    return templates.filter((template) => {
      if (favoritesOnly && !isFavorite("template", template.id)) return false;
      if (!needle) return true;
      return `${template.name} ${template.description} ${template.packs.join(" ")}`
        .toLowerCase()
        .includes(needle);
    });
  }, [favoritesOnly, isFavorite, query, templates]);

  const searching = query.trim().length > 0;

  async function openPack(pack: PackSummary) {
    setSelected(pack);
    setPlan(null);
    setItems([]);
    setOverwritePaths(new Set());
    setItemsLoading(true);
    try {
      const list = await apiGet<PackItem[]>(
        `/api/starter-packs/${pack.id}/items?workspace_id=${encodeURIComponent(
          activeWorkspaceId ?? "",
        )}`,
      );
      setItems(Array.isArray(list) ? list : []);
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível carregar os itens",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setItemsLoading(false);
    }
    if (!activeWorkspaceId) return;
    setPlanLoading(true);
    try {
      const preview = await apiPost<InstallPlan>(
        `/api/starter-packs/${pack.id}/preview`,
        { workspace_id: activeWorkspaceId },
      );
      setPlan(preview);
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível preparar a instalação",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setPlanLoading(false);
    }
  }

  async function installItem(kind: string, id: string) {
    if (!selected || !activeWorkspaceId) return;
    const key = `${kind}:${id}`;
    setInstallingItem(key);
    try {
      const result = await apiPost<ItemInstallResult>(
        `/api/starter-packs/${selected.id}/items/install`,
        {
          workspace_id: activeWorkspaceId,
          kind,
          id,
          overwrite: false,
        },
      );
      toast({
        tone: "success",
        title: `${result.kind} ${result.id} instalado`,
        description: `${result.written} arquivo(s) gravado(s)${
          result.skipped_conflicts ? `, ${result.skipped_conflicts} conflito(s)` : ""
        }.`,
      });
      await refresh();
      await load();
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível instalar o item",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setInstallingItem(null);
    }
  }

  async function installPack() {
    if (!selected || !activeWorkspaceId) return;
    setInstalling(true);
    try {
      const overwrite = plan
        ? plan.files
            .filter(
              (file) =>
                file.status === "conflict" && overwritePaths.has(file.path),
            )
            .map((file) => file.path)
        : [];
      const result = await apiPost<InstallResult>(
        `/api/starter-packs/${selected.id}/install`,
        {
          workspace_id: activeWorkspaceId,
          overwrite,
          skip_conflicts: true,
        },
      );
      toast({
        tone: "success",
        title: `${selected.name} adicionado`,
        description: `${result.written} arquivo(s), ${result.routines.length} rotina(s)${
          result.skipped_conflicts
            ? `, ${result.skipped_conflicts} conflito(s) mantido(s)`
            : ""
        }.`,
      });
      setSelected(null);
      setPlan(null);
      setItems([]);
      await load();
      await refresh();
    } catch (cause) {
      toast({
        tone: "error",
        title: "Falha ao adicionar o pacote",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setInstalling(false);
    }
  }

  async function removePack() {
    if (!selected || !activeWorkspaceId) return;
    const confirmed = await confirmAction({
      title: `Remover ${selected.name}?`,
      description:
        "Arquivos que você modificou são preservados. Rotinas já executadas também são mantidas.",
      confirmLabel: "Remover pacote",
      tone: "destructive",
    });
    if (!confirmed) return;
    setRemoving(true);
    try {
      const result = await apiDelete<{
        removed: number;
        kept: string[];
        routines_removed: number;
        routines_kept: string[];
      }>(
        `/api/starter-packs/${selected.id}?workspace_id=${encodeURIComponent(activeWorkspaceId)}`,
      );
      toast({
        tone: "success",
        title: `${selected.name} removido`,
        description: `${result.removed} arquivo(s) removido(s)${
          result.kept.length ? `, ${result.kept.length} preservado(s)` : ""
        }.`,
      });
      setSelected(null);
      setPlan(null);
      setItems([]);
      await load();
      await refresh();
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível remover",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setRemoving(false);
    }
  }

  function openTemplate(template: WorkspaceTemplate) {
    setTemplateErrors([]);
    setTemplateOpen(template);
    setTemplateName(template.name);
    setTemplateRoot("");
    setTemplateColor(template.color);
  }

  async function createTemplate() {
    if (!templateOpen) return;
    setCreatingTemplate(true);
    try {
      const result = await apiPost<{
        workspace: { id: string; name: string };
        installed: string[];
        errors: string[];
      }>("/api/starter-packs/templates/create", {
        template_id: templateOpen.id,
        name: templateName.trim() || templateOpen.name,
        root_path: templateRoot.trim() || null,
        color: templateColor,
      });
      setTemplateErrors(result.errors);
      toast({
        tone: result.errors.length ? "warning" : "success",
        title: "Workspace criado",
        description: `${result.workspace.name} · ${result.installed.length} pacote(s)${
          result.errors.length ? ` · ${result.errors.length} pendência(s)` : ""
        }.`,
      });
      if (result.errors.length === 0) setTemplateOpen(null);
      await refresh();
      await load();
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível criar o workspace",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setCreatingTemplate(false);
    }
  }

  async function updateAll() {
    if (!activeWorkspaceId || updatingAll) return;
    const targets = packs.filter((pack) => pack.installed && pack.update_available);
    if (targets.length === 0) return;
    setUpdatingAll(true);
    let written = 0;
    let skipped = 0;
    let failed = 0;
    let lastError: string | null = null;
    for (const pack of targets) {
      try {
        const result = await apiPost<InstallResult>(
          `/api/starter-packs/${pack.id}/install`,
          {
            workspace_id: activeWorkspaceId,
            overwrite: [],
            skip_conflicts: true,
          },
        );
        written += result.written;
        skipped += result.skipped_conflicts;
      } catch (cause) {
        failed += 1;
        lastError = cause instanceof Error ? cause.message : String(cause);
      }
    }
    toast({
      tone: failed ? "warning" : "success",
      title: failed ? "Atualização parcial" : "Pacotes atualizados",
      description: `${targets.length - failed}/${targets.length} pacote(s) · ${written} arquivo(s)${
        skipped ? ` · ${skipped} conflito(s) preservado(s)` : ""
      }${lastError ? ` · ${lastError}` : ""}.`,
    });
    setUpdatingAll(false);
    await load();
    await refresh();
  }

  async function removeSelected() {
    if (!activeWorkspaceId || selectedPacks.size === 0) return;
    const names = packs
      .filter((pack) => selectedPacks.has(pack.id))
      .map((pack) => pack.name);
    if (names.length === 0) {
      toast({
        tone: "warning",
        title: "Nenhum pacote selecionado neste workspace",
        description: "A seleção foi limpa para evitar remover arquivos de outro workspace.",
      });
      setSelectedPacks(new Set());
      setManaging(false);
      return;
    }
    const confirmed = await confirmAction({
      title: `Remover ${names.length} pacote(s)?`,
      description: `${names.slice(0, 4).join(", ")}${
        names.length > 4 ? ` e mais ${names.length - 4}` : ""
      }. Arquivos que você modificou e rotinas já executadas são preservados.`,
      confirmLabel: "Remover pacotes",
      tone: "destructive",
    });
    if (!confirmed) return;
    let removed = 0;
    let kept = 0;
    let failed = 0;
    let lastError: string | null = null;
    for (const id of selectedPacks) {
      try {
        const result = await apiDelete<{ removed: number; kept: string[] }>(
          `/api/starter-packs/${id}?workspace_id=${encodeURIComponent(activeWorkspaceId)}`,
        );
        removed += result.removed;
        kept += result.kept.length;
      } catch (cause) {
        failed += 1;
        lastError = cause instanceof Error ? cause.message : String(cause);
      }
    }
    toast({
      tone: failed ? "warning" : "success",
      title: failed ? "Remoção parcial" : "Pacotes removidos",
      description: `${removed} arquivo(s) removido(s)${
        kept ? ` · ${kept} preservado(s)` : ""
      }${failed ? ` · ${failed} falha(s)` : ""}${lastError ? ` · ${lastError}` : ""}.`,
    });
    setSelectedPacks(new Set());
    setManaging(false);
    await load();
    await refresh();
  }

  async function installRecommended(skill: RecommendedSkill) {
    if (!activeWorkspaceId) {
      toast({
        tone: "warning",
        title: "Selecione um workspace",
        description: "A skill precisa de um workspace ativo para ser instalada.",
      });
      setWorkspaceDialogOpen(true);
      return;
    }
    const workspaceName =
      workspaces.find((workspace) => workspace.id === activeWorkspaceId)?.name ??
      null;
    const payload = buildInstallTask({
      kind: "skill",
      name: skill.name,
      source: skill.source,
      sourceType: skill.sourceType,
      url: skill.url,
      installCommand: skill.installCommand,
      skillId: skill.skillId,
      note: skill.note,
      workspaceName,
    });
    try {
      const task = await apiPost<Task>(
        `/api/tasks?workspace_id=${encodeURIComponent(activeWorkspaceId)}`,
        {
          title: payload.title,
          description: payload.description,
          execution_mode: "AUTO_DSH",
          priority: 2,
          care_level: "standard",
          tags: payload.tags,
          origin: "MANUAL",
        },
      );
      if (!task.active_job_status) {
        await apiPost(`/api/tasks/${task.id}/dispatch`, {}).catch(() => {
          // The task is queued for manual execution when the engine is offline.
        });
      }
      toast({
        tone: "success",
        title: "Instalação enviada para execução",
        description: `${skill.name} está na tarefa ${task.title}.`,
        href: links.task(task.id),
      });
      await refresh();
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível criar a tarefa de instalação",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    }
  }

  function dismissOnboarding() {
    setOnboardingDismissed(true);
    try {
      localStorage.setItem(ONBOARDING_KEY, "1");
    } catch {
      // storage can be unavailable; dismiss for this session only
    }
  }

  return (
    <PageBody>
      <PageHeader
        title="Pacotes"
        description="Comece com um pacote pronto ou adicione capacidades ao workspace atual. Tudo é opcional e removível."
        actions={
          <>
            <Button variant="outline" size="sm" onClick={() => void load()} disabled={loading}>
              <IconLabel icon={<RefreshCw size={13} className={loading ? "animate-spin" : undefined} />}>
                Atualizar
              </IconLabel>
            </Button>
            <Badge variant="default">
              {installedCount} instalado(s) neste workspace
            </Badge>
          </>
        }
      />

      {error ? (
        <p className="rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[13px] text-red-300">
          {error}
        </p>
      ) : null}

      {!searching &&
      (tab === "all" || tab === "workspaces") &&
      !onboardingDismissed &&
      installedCount === 0 &&
      templates.length > 0 ? (
        <Card className="border-blue-500/25 bg-blue-500/[0.04]">
          <CardHeader className="py-3">
            <CardTitle className="flex items-center gap-2 text-[14px]">
              <Sparkles size={15} className="text-blue-300" />
              Comece com um ponto de partida
            </CardTitle>
            <button
              type="button"
              className="text-[12px] text-zinc-500 hover:text-zinc-200"
              onClick={dismissOnboarding}
            >
              não mostrar novamente
            </button>
          </CardHeader>
          <CardContent className="grid gap-2 sm:grid-cols-2 xl:grid-cols-3">
            {templates.map((template) => (
              <WorkspaceTemplateCard
                key={template.id}
                template={template}
                action="criar workspace"
                favorite={isFavorite("template", template.id)}
                onClick={() => openTemplate(template)}
                onToggleFavorite={() =>
                  toggleFavorite("template", template.id)
                }
              />
            ))}
          </CardContent>
        </Card>
      ) : null}

      <PacksToolbar
        tab={tab}
        query={query}
        counts={{ templates: templates.length, packs: packs.length }}
        searching={searching}
        filteredTemplates={filteredTemplates.length}
        filteredPacks={filtered.length}
        favoritesOnly={favoritesOnly}
        onTabChange={setTab}
        onQueryChange={setQuery}
        onToggleFavorites={() =>
          setInstallFilter((current) =>
            current === "favorites" ? "all" : "favorites",
          )
        }
      />

      {((searching && filteredTemplates.length > 0) ||
      (!searching &&
        (tab === "all" || tab === "workspaces") &&
        (onboardingDismissed || installedCount > 0))) ? (
        <Card>
          <CardHeader className="py-3">
            <div>
              <CardTitle className="flex items-center gap-2 text-[13.5px]">
                <FolderPlus size={14} className="text-zinc-500" />
                Workspaces prontos
              </CardTitle>
              <p className="mt-1 text-[12px] text-zinc-500">
                Crie um workspace com cor, pastas e pacotes já organizados. Você pode
                editar tudo depois.
              </p>
            </div>
          </CardHeader>
          <CardContent className="grid gap-2 sm:grid-cols-2 xl:grid-cols-3">
            {filteredTemplates.map((template) => (
              <WorkspaceTemplateCard
                key={template.id}
                template={template}
                favorite={isFavorite("template", template.id)}
                onClick={() => openTemplate(template)}
                onToggleFavorite={() =>
                  toggleFavorite("template", template.id)
                }
              />
            ))}
          </CardContent>
        </Card>
      ) : null}

      {tab === "recommended" || tab === "all" || searching ? (
        <RecommendedSkills
          query={query}
          isFavorite={(id) => isFavorite("skill", id)}
          onToggleFavorite={(id) => toggleFavorite("skill", id)}
          onInstall={(skill) => void installRecommended(skill)}
          favoritesOnly={favoritesOnly}
        />
      ) : null}

      {(!searching && (tab === "all" || tab === "resources")) || searching ? (
        <ExternalResources
          query={query}
          availableTemplateIds={templates.map((template) => template.id)}
          onCreateWorkspace={(templateId) => {
            const template = templates.find((item) => item.id === templateId);
            if (template) openTemplate(template);
          }}
          favoritesOnly={favoritesOnly}
          isFavorite={(id) => isFavorite("resource", id)}
          onToggleFavorite={(id) => toggleFavorite("resource", id)}
        />
      ) : null}

      {((!searching && (tab === "all" || tab === "packs")) ||
      (searching && filtered.length > 0)) ? (
        <PackCatalog
          packs={packs}
          filtered={filtered}
          categories={categories}
          category={category}
          installFilter={installFilter}
          installedCount={installedCount}
          loading={loading}
          isFavorite={(id) => isFavorite("pack", id)}
          onCategoryChange={setCategory}
          onInstallFilterChange={setInstallFilter}
          onOpenPack={(pack) => void openPack(pack)}
          onToggleFavorite={(id) => toggleFavorite("pack", id)}
          managing={managing}
          selectedPacks={selectedPacks}
          updatableCount={
            packs.filter((pack) => pack.installed && pack.update_available).length
          }
          updating={updatingAll}
          onToggleManage={() => {
            setManaging((current) => !current);
            setSelectedPacks(new Set());
          }}
          onTogglePack={(id) =>
            setSelectedPacks((current) => {
              const next = new Set(current);
              if (next.has(id)) next.delete(id);
              else next.add(id);
              return next;
            })
          }
          onSelectInstalled={() =>
            setSelectedPacks(
              new Set(
                packs.filter((pack) => pack.installed).map((pack) => pack.id),
              ),
            )
          }
          onClearSelection={() => setSelectedPacks(new Set())}
          onUpdateAll={() => void updateAll()}
          onRemoveSelected={() => void removeSelected()}
          onClearFilters={() => setInstallFilter("all")}
        />
      ) : null}

      <PackInstallModal
        selected={selected}
        plan={plan}
        planLoading={planLoading}
        installing={installing}
        removing={removing}
        overwritePaths={overwritePaths}
        activeWorkspaceId={activeWorkspaceId}
        items={items}
        itemsLoading={itemsLoading}
        installingItem={installingItem}
        onClose={() => {
          setSelected(null);
          setPlan(null);
          setItems([]);
        }}
        onToggleOverwrite={(path, checked) =>
          setOverwritePaths((current) => {
            const next = new Set(current);
            if (checked) next.add(path);
            else next.delete(path);
            return next;
          })
        }
        onInstall={() => void installPack()}
        onRemove={() => void removePack()}
        onInstallItem={(kind, id) => void installItem(kind, id)}
      />

      <WorkspaceTemplateModal
        template={templateOpen}
        name={templateName}
        root={templateRoot}
        color={templateColor}
        creating={creatingTemplate}
        errors={templateErrors}
        onName={setTemplateName}
        onRoot={setTemplateRoot}
        onColor={setTemplateColor}
        onCreate={() => void createTemplate()}
        onClose={() => setTemplateOpen(null)}
      />

      <p className="flex items-center gap-1.5 text-[12px] text-zinc-500">
        <CheckCircle2 size={11} />
        O pacote Controle OpenDesk é instalado automaticamente em novos workspaces;
        os demais são opcionais. Remover um pacote não apaga arquivos que você
        modificou. Recomendações como o Impeccable seguem externas por design: o
        instalador e o engine são do próprio projeto.
      </p>
    </PageBody>
  );
}

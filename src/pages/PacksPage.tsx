import { useCallback, useEffect, useMemo, useState } from "react";
import type {
  Category,
  InstallPlan,
  PackItem,
  PackSummary,
  WorkspaceTemplate,
} from "@/components/packs/pack-types";
import { useFavorites } from "@/lib/favorites";
import type { RecommendedSkill } from "@/lib/recommended-skills";
import { links } from "@/lib/links";
import { confirmAction } from "@/store/confirm-store";
import { toast } from "@/store/toast-store";
import { useAppStore } from "@/store/app-store";
import { useUiStore } from "@/store/ui-store";
import {
  PacksPageContent,
  type PacksInstallFilter,
  type PacksTab,
} from "@/pages/packs/PacksPageContent";
import {
  createSkillInstallTask,
  createTemplateWorkspaceRequest,
  fetchPackItems,
  fetchStarterPackCatalog,
  installPackItemRequest,
  installPackRequest,
  installPackDescription,
  previewPackInstall,
  removeInstalledPacks,
  removePackRequest,
  removePacksDescription,
  updateInstalledPacks,
  updatePacksDescription,
} from "@/pages/packs/packs-api";
import {
  filterPacks,
  filterTemplates,
  readOnboardingDismissed,
  writeOnboardingDismissed,
} from "@/pages/packs/packs-filters";

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
  const [installFilter, setInstallFilter] = useState<PacksInstallFilter>("all");
  const [tab, setTab] = useState<PacksTab>("all");
  const { toggle: toggleFavorite, isFavorite } = useFavorites();
  const setWorkspaceDialogOpen = useUiStore((state) => state.setWorkspaceDialogOpen);
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
  const [onboardingDismissed, setOnboardingDismissed] = useState(readOnboardingDismissed);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const { catalog, templates: templateList } =
        await fetchStarterPackCatalog(activeWorkspaceId);
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

  const filtered = useMemo(
    () => filterPacks(packs, { query, category, installFilter, isFavorite }),
    [category, installFilter, isFavorite, packs, query],
  );

  const favoritesOnly = installFilter === "favorites";

  const filteredTemplates = useMemo(
    () => filterTemplates(templates, { query, favoritesOnly, isFavorite }),
    [favoritesOnly, isFavorite, query, templates],
  );

  const searching = query.trim().length > 0;

  async function openPack(pack: PackSummary) {
    setSelected(pack);
    setPlan(null);
    setItems([]);
    setOverwritePaths(new Set());
    setItemsLoading(true);
    try {
      setItems(await fetchPackItems(pack.id, activeWorkspaceId));
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
      setPlan(await previewPackInstall(pack.id, activeWorkspaceId));
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
      const result = await installPackItemRequest(
        selected.id,
        activeWorkspaceId,
        kind,
        id,
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
      const result = await installPackRequest(
        selected.id,
        activeWorkspaceId,
        overwrite,
      );
      toast({
        tone: "success",
        title: `${selected.name} adicionado`,
        description: installPackDescription(result),
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
      const result = await removePackRequest(selected.id, activeWorkspaceId);
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
      const result = await createTemplateWorkspaceRequest({
        templateId: templateOpen.id,
        name: templateName.trim() || templateOpen.name,
        rootPath: templateRoot.trim() || null,
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
    const summary = await updateInstalledPacks(targets, activeWorkspaceId);
    toast({
      tone: summary.failed ? "warning" : "success",
      title: summary.failed ? "Atualização parcial" : "Pacotes atualizados",
      description: updatePacksDescription(summary),
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
    const summary = await removeInstalledPacks(selectedPacks, activeWorkspaceId);
    toast({
      tone: summary.failed ? "warning" : "success",
      title: summary.failed ? "Remoção parcial" : "Pacotes removidos",
      description: removePacksDescription(summary),
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
    try {
      const task = await createSkillInstallTask(
        skill,
        activeWorkspaceId,
        workspaceName,
      );
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
    writeOnboardingDismissed();
  }

  return (
    <PacksPageContent
      installedCount={installedCount}
      error={error}
      loading={loading}
      searching={searching}
      tab={tab}
      templates={templates}
      onboardingDismissed={onboardingDismissed}
      query={query}
      filteredTemplates={filteredTemplates}
      packs={packs}
      filtered={filtered}
      categories={categories}
      category={category}
      installFilter={installFilter}
      managing={managing}
      selectedPacks={selectedPacks}
      updatableCount={
        packs.filter((pack) => pack.installed && pack.update_available).length
      }
      updatingAll={updatingAll}
      favoritesOnly={favoritesOnly}
      isFavorite={isFavorite}
      onToggleFavorite={toggleFavorite}
      onDismissOnboarding={dismissOnboarding}
      onRefresh={() => void load()}
      onTabChange={setTab}
      onQueryChange={setQuery}
      onToggleFavorites={() =>
        setInstallFilter((current) =>
          current === "favorites" ? "all" : "favorites",
        )
      }
      onCategoryChange={setCategory}
      onInstallFilterChange={setInstallFilter}
      onOpenPack={(pack) => void openPack(pack)}
      onOpenTemplate={openTemplate}
      onInstallRecommended={(skill) => void installRecommended(skill)}
      onCreateWorkspace={(templateId) => {
        const template = templates.find((item) => item.id === templateId);
        if (template) openTemplate(template);
      }}
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
      installModal={{
        selected,
        plan,
        planLoading,
        installing,
        removing,
        overwritePaths,
        activeWorkspaceId,
        items,
        itemsLoading,
        installingItem,
        onClose: () => {
          setSelected(null);
          setPlan(null);
          setItems([]);
        },
        onToggleOverwrite: (path, checked) =>
          setOverwritePaths((current) => {
            const next = new Set(current);
            if (checked) next.add(path);
            else next.delete(path);
            return next;
          }),
        onInstall: () => void installPack(),
        onRemove: () => void removePack(),
        onInstallItem: (kind, id) => void installItem(kind, id),
      }}
      templateModal={{
        template: templateOpen,
        name: templateName,
        root: templateRoot,
        color: templateColor,
        creating: creatingTemplate,
        errors: templateErrors,
        onName: setTemplateName,
        onRoot: setTemplateRoot,
        onColor: setTemplateColor,
        onCreate: () => void createTemplate(),
        onClose: () => setTemplateOpen(null),
      }}
    />
  );
}

import { useCallback, useEffect, useMemo, useState } from "react";
import {
  Boxes,
  CheckCircle2,
  FolderPlus,
  RefreshCw,
  Search,
  Sparkles,
  Star,
} from "lucide-react";
import { PageBody, PageHeader } from "@/components/layout/PageHeader";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { EmptyState } from "@/components/ui/empty-state";
import { IconLabel } from "@/components/ui/icon-label";
import { LoadingState } from "@/components/ui/loading-state";
import { ExternalResources } from "@/components/packs/ExternalResources";
import { RecommendedSkills } from "@/components/packs/RecommendedSkills";
import { StudioInstallModal } from "@/components/studio/StudioInstallModal";
import { PackInstallModal } from "@/components/packs/PackInstallModal";
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
import { confirmAction } from "@/store/confirm-store";
import { toast } from "@/store/toast-store";
import { useAppStore } from "@/store/app-store";
import { useUiStore } from "@/store/ui-store";
import { cn } from "@/lib/utils";

const ONBOARDING_KEY = "opendesk.starter-packs.onboarding-dismissed";

export function PacksPage() {
  const activeWorkspaceId = useAppStore((state) => state.activeWorkspaceId);
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
  const [installRecommendation, setInstallRecommendation] =
    useState<RecommendedSkill | null>(null);
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

  const filteredTemplates = useMemo(() => {
    const needle = query.trim().toLowerCase();
    if (!needle) return templates;
    return templates.filter((template) =>
      `${template.name} ${template.description} ${template.packs.join(" ")}`
        .toLowerCase()
        .includes(needle),
    );
  }, [query, templates]);

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

  function installRecommended(skill: RecommendedSkill) {
    if (!activeWorkspaceId) {
      toast({
        tone: "warning",
        title: "Selecione um workspace",
        description: "A skill precisa de um workspace ativo para ser instalada.",
      });
      setWorkspaceDialogOpen(true);
      return;
    }
    setInstallRecommendation(skill);
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
              <TemplateCard
                key={template.id}
                template={template}
                action="criar workspace"
                onClick={() => openTemplate(template)}
              />
            ))}
          </CardContent>
        </Card>
      ) : null}

      <div className="flex flex-col gap-2 rounded-xl border border-zinc-800/70 bg-zinc-900/20 p-2">
        <div className="flex flex-wrap items-center gap-1">
          {[
            { id: "all", label: "Visão geral" },
            { id: "workspaces", label: `Workspaces (${templates.length})` },
            { id: "packs", label: `Pacotes (${packs.length})` },
            { id: "resources", label: "Capacidades externas" },
            { id: "recommended", label: "Recomendados" },
          ].map((item) => (
            <button
              key={item.id}
              type="button"
              aria-pressed={tab === item.id}
              onClick={() => setTab(item.id as typeof tab)}
              className={
                tab === item.id
                  ? "inline-flex h-8 items-center rounded-lg bg-zinc-800 px-3 text-[13.5px] font-medium text-zinc-100"
                  : "inline-flex h-8 items-center rounded-lg px-3 text-[13.5px] font-medium text-zinc-500 transition-colors hover:bg-zinc-900 hover:text-zinc-200"
              }
            >
              {item.label}
            </button>
          ))}
          <div className="ml-auto flex h-9 min-w-[240px] flex-1 items-center gap-2 rounded-lg border border-zinc-800 bg-zinc-950/40 px-2.5 focus-within:border-zinc-600 sm:max-w-md">
            <Search size={13} className="shrink-0 text-zinc-500" />
            <input
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder="Buscar nos workspaces, pacotes e capacidades externas…"
              className="h-full min-w-0 flex-1 bg-transparent text-[14px] outline-none placeholder:text-zinc-500"
              aria-label="Buscar workspaces, pacotes e capacidades externas"
            />
          </div>
        </div>
        {searching ? (
          <p className="text-[11.5px] text-zinc-500">
            Resultados em {filteredTemplates.length} workspace(s), {filtered.length} pacote(s),
            capacidades externas e recomendações.
          </p>
        ) : null}
      </div>

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
              <TemplateCard
                key={template.id}
                template={template}
                onClick={() => openTemplate(template)}
              />
            ))}
          </CardContent>
        </Card>
      ) : null}

      {!searching && tab === "recommended" ? (
        <RecommendedSkills
          query={query}
          isFavorite={(id) => isFavorite("skill", id)}
          onToggleFavorite={(id) => toggleFavorite("skill", id)}
          onInstall={installRecommended}
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
        />
      ) : null}

      {((!searching && (tab === "all" || tab === "packs")) ||
      (searching && filtered.length > 0)) ? (
        <>
      <div className="flex flex-wrap items-end justify-between gap-3 pt-1">
        <div>
          <h2 className="text-[14px] font-semibold text-zinc-100">Adicionar capacidades</h2>
          <p className="mt-0.5 text-[12px] text-zinc-500">
            Instale pacotes no workspace ativo ou use um workspace pronto como base.
          </p>
        </div>
      </div>

      <div className="flex flex-wrap items-center gap-2 rounded-xl border border-zinc-800/70 bg-zinc-900/20 px-3 py-2">
        <select
          value={category}
          onChange={(event) => setCategory(event.target.value)}
          className="h-8 rounded-lg border border-zinc-800 bg-zinc-900/50 px-2 text-[12px] text-zinc-300"
          aria-label="Filtrar por categoria"
        >
          <option value="all">todas as categorias</option>
          {categories.map((item) => (
            <option key={item.id} value={item.id}>
              {item.name}
            </option>
          ))}
        </select>
        <div className="flex rounded-lg border border-zinc-800 bg-zinc-950/40 p-0.5">
          {([
            ["all", "Todos"],
            ["installed", `Instalados (${installedCount})`],
            ["available", "Disponíveis"],
            ["favorites", "Favoritos"],
          ] as const).map(([value, label]) => (
            <button
              key={value}
              type="button"
              aria-pressed={installFilter === value}
              onClick={() => setInstallFilter(value)}
              className={cn(
                "inline-flex h-8 items-center rounded-md px-2.5 text-[13.5px] transition-colors",
                installFilter === value
                  ? "bg-zinc-800 text-zinc-100"
                  : "text-zinc-500 hover:text-zinc-200",
              )}
            >
              {label}
            </button>
          ))}
        </div>
        <span className="text-[12px] text-zinc-500">
          {filtered.length} de {packs.length}
        </span>
      </div>

      {loading ? (
        <LoadingState label="Carregando pacotes…" rows={6} />
      ) : filtered.length === 0 ? (
        <EmptyState
          icon={Boxes}
          title="Nenhum pacote encontrado"
          description="Ajuste o filtro de categoria ou limpe a busca."
          compact
        />
      ) : (
        <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-3">
          {filtered.map((pack) => (
            <article
              key={pack.id}
              className={cn(
                "relative rounded-xl border bg-zinc-950/40 transition-colors hover:border-zinc-600",
                pack.installed ? "border-emerald-500/30" : "border-zinc-800/70",
              )}
            >
              <button
                type="button"
                onClick={() => void openPack(pack)}
                className="block w-full rounded-xl p-3 text-left outline-none focus-visible:ring-2 focus-visible:ring-blue-500/50"
              >
              <div className="flex items-start gap-2 pr-7">
                <span
                  className="mt-0.5 h-2.5 w-2.5 shrink-0 rounded-full"
                  style={{ background: pack.color }}
                />
                <div className="min-w-0 flex-1">
                  <div className="flex flex-wrap items-center gap-1.5">
                    <span className="truncate text-[13.5px] font-medium text-zinc-100">
                      {pack.name}
                    </span>
                    {pack.recommended ? <Badge variant="blue">recomendado</Badge> : null}
                    {pack.installed ? (
                      <Badge
                        variant={
                          pack.health.status === "partial"
                            ? "red"
                            : pack.update_available
                              ? "yellow"
                              : pack.health.status === "modified"
                                ? "purple"
                                : "green"
                        }
                        title="Este pacote está presente no workspace ativo"
                      >
                        <CheckCircle2 size={9} />
                        {pack.health.status === "partial"
                          ? "parcial"
                          : pack.update_available
                            ? "atualização"
                            : pack.health.status === "modified"
                              ? "personalizado"
                              : "instalado"}
                      </Badge>
                    ) : null}
                  </div>
                  <p className="mt-1 line-clamp-2 text-[12px] leading-snug text-zinc-500">
                    {pack.description}
                  </p>
                  <div className="mt-2 flex flex-wrap items-center gap-1 text-[11.5px] text-zinc-500">
                    <span>{pack.counts.agents} agentes</span>
                    <span>·</span>
                    <span>{pack.counts.recipes} receitas</span>
                    <span>·</span>
                    <span>{pack.counts.skills} skills</span>
                    {pack.counts.routines > 0 ? (
                      <>
                        <span>·</span>
                        <span>{pack.counts.routines} rotinas</span>
                      </>
                    ) : null}
                  </div>
                  {pack.requires.tools.length || pack.requires.integrations.length ? (
                    <div className="mt-1 flex flex-wrap gap-1">
                      {pack.requires.tools.map((tool) => (
                        <Badge key={tool} variant="yellow">
                          {tool}
                        </Badge>
                      ))}
                      {pack.requires.integrations.map((integration) => (
                        <Badge key={integration} variant="purple">
                          {integration}
                        </Badge>
                      ))}
                    </div>
                  ) : null}
                </div>
              </div>
              </button>
              <button
                type="button"
                aria-label={
                  isFavorite("pack", pack.id)
                    ? `Remover ${pack.name} dos favoritos`
                    : `Favoritar ${pack.name}`
                }
                title={
                  isFavorite("pack", pack.id)
                    ? "Remover dos favoritos"
                    : "Favoritar"
                }
                onClick={() => toggleFavorite("pack", pack.id)}
                className={cn(
                  "absolute right-2 top-2 rounded-md p-1.5 transition-colors",
                  isFavorite("pack", pack.id)
                    ? "text-amber-300 hover:bg-amber-500/10"
                    : "text-zinc-500 hover:bg-zinc-800/60 hover:text-zinc-200",
                )}
              >
                <Star
                  size={15}
                  fill={isFavorite("pack", pack.id) ? "currentColor" : "none"}
                />
              </button>
            </article>
          ))}
        </div>
      )}

        </>
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

      <StudioInstallModal
        open={installRecommendation !== null}
        workspaceId={activeWorkspaceId}
        defaultScope="workspace"
        initialSource={
          installRecommendation
            ? {
                sourceType: installRecommendation.sourceType,
                value: installRecommendation.source,
                kind: "skill",
              }
            : undefined
        }
        preselectIds={
          installRecommendation ? [installRecommendation.skillId] : undefined
        }
        lockSource
        onClose={() => setInstallRecommendation(null)}
        onInstalled={async () => {
          const name = installRecommendation?.name ?? "Skill";
          setInstallRecommendation(null);
          toast({
            tone: "success",
            title: `${name} instalada`,
            description: "A skill foi gravada no workspace ativo.",
          });
          await refresh();
        }}
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
        modificou.
      </p>
    </PageBody>
  );
}


function TemplateCard({
  template,
  action,
  onClick,
}: {
  template: WorkspaceTemplate;
  action?: string;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="rounded-lg border border-zinc-800/70 bg-zinc-950/40 p-3 text-left transition-colors hover:border-zinc-600"
    >
      <div className="flex items-center gap-2">
        <span
          className="h-2.5 w-2.5 rounded-full"
          style={{ background: template.color }}
        />
        <span className="text-[13.5px] font-medium text-zinc-200">
          {template.name}
        </span>
      </div>
      <p className="mt-1 text-[12px] leading-snug text-zinc-500">
        {template.description}
      </p>
      <p className="mt-2 text-[12px] text-zinc-500">
        {template.packs.length} pacotes · {template.folders.length} pastas
        {action ? ` · ${action}` : ""}
      </p>
      <p className="mt-1 truncate font-mono text-[11px] text-zinc-500">
        {template.packs.slice(0, 4).join(" · ")}
        {template.packs.length > 4 ? ` +${template.packs.length - 4}` : ""}
      </p>
    </button>
  );
}

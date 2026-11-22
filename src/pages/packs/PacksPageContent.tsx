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
  PackItem,
  PackSummary,
  WorkspaceTemplate,
} from "@/components/packs/pack-types";
import type { FavoriteKind } from "@/lib/favorites";
import type { RecommendedSkill } from "@/lib/recommended-skills";

export type PacksTab = "all" | "workspaces" | "packs" | "resources" | "recommended";
export type PacksInstallFilter = "all" | "installed" | "available" | "favorites";

export interface PacksInstallModalProps {
  selected: PackSummary | null;
  plan: InstallPlan | null;
  planLoading: boolean;
  installing: boolean;
  removing: boolean;
  overwritePaths: Set<string>;
  activeWorkspaceId: string | null;
  items: PackItem[];
  itemsLoading: boolean;
  installingItem: string | null;
  onClose: () => void;
  onToggleOverwrite: (path: string, checked: boolean) => void;
  onInstall: () => void;
  onRemove: () => void;
  onInstallItem: (kind: string, id: string) => void;
}

export interface PacksTemplateModalProps {
  template: WorkspaceTemplate | null;
  name: string;
  root: string;
  color: string;
  creating: boolean;
  errors: string[];
  onName: (value: string) => void;
  onRoot: (value: string) => void;
  onColor: (value: string) => void;
  onCreate: () => void;
  onClose: () => void;
}

export interface PacksPageContentProps {
  installedCount: number;
  error: string | null;
  loading: boolean;
  searching: boolean;
  tab: PacksTab;
  templates: WorkspaceTemplate[];
  onboardingDismissed: boolean;
  query: string;
  filteredTemplates: WorkspaceTemplate[];
  packs: PackSummary[];
  filtered: PackSummary[];
  categories: Category[];
  category: string;
  installFilter: PacksInstallFilter;
  managing: boolean;
  selectedPacks: Set<string>;
  updatableCount: number;
  updatingAll: boolean;
  favoritesOnly: boolean;
  isFavorite: (kind: FavoriteKind, id: string) => boolean;
  onToggleFavorite: (kind: FavoriteKind, id: string) => void;
  onDismissOnboarding: () => void;
  onRefresh: () => void;
  onTabChange: (tab: PacksTab) => void;
  onQueryChange: (query: string) => void;
  onToggleFavorites: () => void;
  onCategoryChange: (category: string) => void;
  onInstallFilterChange: (filter: PacksInstallFilter) => void;
  onOpenPack: (pack: PackSummary) => void;
  onOpenTemplate: (template: WorkspaceTemplate) => void;
  onInstallRecommended: (skill: RecommendedSkill) => void;
  onCreateWorkspace: (templateId: string) => void;
  onToggleManage: () => void;
  onTogglePack: (id: string) => void;
  onSelectInstalled: () => void;
  onClearSelection: () => void;
  onUpdateAll: () => void;
  onRemoveSelected: () => void;
  onClearFilters: () => void;
  installModal: PacksInstallModalProps;
  templateModal: PacksTemplateModalProps;
}

export function PacksPageContent({
  installedCount,
  error,
  loading,
  searching,
  tab,
  templates,
  onboardingDismissed,
  query,
  filteredTemplates,
  packs,
  filtered,
  categories,
  category,
  installFilter,
  managing,
  selectedPacks,
  updatableCount,
  updatingAll,
  favoritesOnly,
  isFavorite,
  onToggleFavorite,
  onDismissOnboarding,
  onRefresh,
  onTabChange,
  onQueryChange,
  onToggleFavorites,
  onCategoryChange,
  onInstallFilterChange,
  onOpenPack,
  onOpenTemplate,
  onInstallRecommended,
  onCreateWorkspace,
  onToggleManage,
  onTogglePack,
  onSelectInstalled,
  onClearSelection,
  onUpdateAll,
  onRemoveSelected,
  onClearFilters,
  installModal,
  templateModal,
}: PacksPageContentProps) {
  return (
    <PageBody>
      <PageHeader
        title="Pacotes"
        description="Comece com um pacote pronto ou adicione capacidades ao workspace atual. Tudo é opcional e removível."
        actions={
          <>
            <Button variant="outline" size="sm" onClick={onRefresh} disabled={loading}>
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
              onClick={onDismissOnboarding}
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
                onClick={() => onOpenTemplate(template)}
                onToggleFavorite={() =>
                  onToggleFavorite("template", template.id)
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
        onTabChange={onTabChange}
        onQueryChange={onQueryChange}
        onToggleFavorites={onToggleFavorites}
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
                onClick={() => onOpenTemplate(template)}
                onToggleFavorite={() =>
                  onToggleFavorite("template", template.id)
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
          onToggleFavorite={(id) => onToggleFavorite("skill", id)}
          onInstall={(skill) => onInstallRecommended(skill)}
          favoritesOnly={favoritesOnly}
        />
      ) : null}

      {(!searching && (tab === "all" || tab === "resources")) || searching ? (
        <ExternalResources
          query={query}
          availableTemplateIds={templates.map((template) => template.id)}
          onCreateWorkspace={onCreateWorkspace}
          favoritesOnly={favoritesOnly}
          isFavorite={(id) => isFavorite("resource", id)}
          onToggleFavorite={(id) => onToggleFavorite("resource", id)}
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
          onCategoryChange={onCategoryChange}
          onInstallFilterChange={onInstallFilterChange}
          onOpenPack={onOpenPack}
          onToggleFavorite={(id) => onToggleFavorite("pack", id)}
          managing={managing}
          selectedPacks={selectedPacks}
          updatableCount={updatableCount}
          updating={updatingAll}
          onToggleManage={onToggleManage}
          onTogglePack={onTogglePack}
          onSelectInstalled={onSelectInstalled}
          onClearSelection={onClearSelection}
          onUpdateAll={onUpdateAll}
          onRemoveSelected={onRemoveSelected}
          onClearFilters={onClearFilters}
        />
      ) : null}

      <PackInstallModal {...installModal} />

      <WorkspaceTemplateModal {...templateModal} />

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

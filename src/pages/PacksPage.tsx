import { useCallback, useEffect, useMemo, useState } from "react";
import { Link } from "react-router-dom";
import {
  Boxes,
  CheckCircle2,
  Download,
  FolderPlus,
  Package,
  RefreshCw,
  Search,
  Sparkles,
  Trash2,
  TriangleAlert,
} from "lucide-react";
import { PageBody, PageHeader } from "@/components/layout/PageHeader";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { CountPill } from "@/components/ui/count-pill";
import { EmptyState } from "@/components/ui/empty-state";
import { IconLabel } from "@/components/ui/icon-label";
import { LoadingState } from "@/components/ui/loading-state";
import { Modal } from "@/components/ui/modal";
import { PathInput } from "@/components/ui/path-input";
import { apiDelete, apiGet, apiPost } from "@/lib/api";
import { confirmAction } from "@/store/confirm-store";
import { toast } from "@/store/toast-store";
import { useAppStore } from "@/store/app-store";
import { cn } from "@/lib/utils";
import {
  fieldInputClass as inputClass,
  fieldLabelClass as labelClass,
} from "@/lib/ui-classes";

interface PackCounts {
  agents: number;
  skills: number;
  commands: number;
  recipes: number;
  routines: number;
}

interface PackRequires {
  tools: string[];
  integrations: string[];
}

interface PackHealth {
  status: "installed" | "modified" | "partial" | string;
  missing_files: number;
  modified_files: number;
  missing_routines: number;
}

interface PackSummary {
  id: string;
  version: string;
  name: string;
  category: string;
  description: string;
  icon: string;
  color: string;
  recommended: boolean;
  requires: PackRequires;
  counts: PackCounts;
  installed: boolean;
  installed_version?: string | null;
  installed_at?: string | null;
  update_available: boolean;
  health: PackHealth;
}

interface Category {
  id: string;
  name: string;
  description: string;
}

interface PlanFile {
  path: string;
  kind: string;
  status: "create" | "identical" | "conflict" | string;
  sha256: string;
}

interface PlanRoutine {
  file: string;
  name: string;
  trigger_type: string;
  recipe_id?: string | null;
}

interface InstallPlan {
  pack_id: string;
  version: string;
  files: PlanFile[];
  routines: PlanRoutine[];
  folders: string[];
  requires: PackRequires;
  conflicts: number;
}

interface WorkspaceTemplate {
  schema: number;
  id: string;
  name: string;
  description: string;
  color: string;
  router_policy: string;
  packs: string[];
  folders: string[];
}

interface InstallResult {
  pack_id: string;
  version: string;
  written: number;
  skipped: number;
  skipped_conflicts: number;
  routines: string[];
  folders: string[];
}

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
  const [selected, setSelected] = useState<PackSummary | null>(null);
  const [plan, setPlan] = useState<InstallPlan | null>(null);
  const [planLoading, setPlanLoading] = useState(false);
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
      if (!needle) return true;
      return `${pack.name} ${pack.description} ${pack.id}`
        .toLowerCase()
        .includes(needle);
    });
  }, [category, packs, query]);

  async function openPack(pack: PackSummary) {
    setSelected(pack);
    setPlan(null);
    setOverwritePaths(new Set());
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
        eyebrow="biblioteca"
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
        <p className="rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[11px] text-red-300">
          {error}
        </p>
      ) : null}

      {!onboardingDismissed && installedCount === 0 && templates.length > 0 ? (
        <Card className="border-blue-500/25 bg-blue-500/[0.04]">
          <CardHeader className="py-3">
            <CardTitle className="flex items-center gap-2 text-[13px]">
              <Sparkles size={15} className="text-blue-300" />
              Comece com um ponto de partida
            </CardTitle>
            <button
              type="button"
              className="text-[10px] text-zinc-500 hover:text-zinc-200"
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

      <div className="flex flex-wrap items-center gap-2 rounded-xl border border-zinc-800/70 bg-zinc-900/20 px-3 py-2">
        <div className="flex h-8 min-w-[220px] flex-1 items-center gap-2 rounded-lg border border-zinc-800 bg-zinc-950/40 px-2.5">
          <Search size={13} className="shrink-0 text-zinc-600" />
          <input
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="Buscar pacote por nome, descrição ou id…"
            className="h-full min-w-0 flex-1 bg-transparent text-[11px] outline-none placeholder:text-zinc-600"
          />
        </div>
        <select
          value={category}
          onChange={(event) => setCategory(event.target.value)}
          className="h-8 rounded-lg border border-zinc-800 bg-zinc-900/50 px-2 text-[10px] text-zinc-300"
          aria-label="Filtrar por categoria"
        >
          <option value="all">todas as categorias</option>
          {categories.map((item) => (
            <option key={item.id} value={item.id}>
              {item.name}
            </option>
          ))}
        </select>
        <span className="text-[10px] text-zinc-500">
          {filtered.length} de {packs.length}
        </span>
      </div>

      {loading ? (
        <LoadingState label="Carregando pacotes…" rows={6} />
      ) : filtered.length === 0 ? (
        <EmptyState
          icon={Boxes}
          title="Nenhum pacote encontrado"
          description="Ajuste a busca ou escolha outra categoria."
        />
      ) : (
        <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-3">
          {filtered.map((pack) => (
            <button
              key={pack.id}
              type="button"
              onClick={() => void openPack(pack)}
              className={cn(
                "rounded-xl border bg-zinc-950/40 p-3 text-left transition-colors hover:border-zinc-600",
                pack.installed ? "border-emerald-500/30" : "border-zinc-800/70",
              )}
            >
              <div className="flex items-start gap-2">
                <span
                  className="mt-0.5 h-2.5 w-2.5 shrink-0 rounded-full"
                  style={{ background: pack.color }}
                />
                <div className="min-w-0 flex-1">
                  <div className="flex flex-wrap items-center gap-1.5">
                    <span className="truncate text-[12px] font-medium text-zinc-100">
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
                      >
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
                  <p className="mt-1 line-clamp-2 text-[10px] leading-snug text-zinc-500">
                    {pack.description}
                  </p>
                  <div className="mt-2 flex flex-wrap items-center gap-1 text-[9px] text-zinc-600">
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
                <Package size={15} className="shrink-0 text-zinc-600" />
              </div>
            </button>
          ))}
        </div>
      )}

      <Card>
        <CardHeader className="py-3">
          <div>
            <CardTitle className="flex items-center gap-2 text-[12px]">
              <FolderPlus size={14} className="text-zinc-500" />
              Workspaces prontos
            </CardTitle>
            <p className="mt-1 text-[10px] text-zinc-500">
              Crie um workspace com cor, pastas e pacotes já organizados. Você pode
              editar tudo depois.
            </p>
          </div>
        </CardHeader>
        <CardContent className="grid gap-2 sm:grid-cols-2 xl:grid-cols-3">
          {templates.map((template) => (
            <TemplateCard
              key={template.id}
              template={template}
              onClick={() => openTemplate(template)}
            />
          ))}
        </CardContent>
      </Card>

      <Modal
        open={selected !== null}
        onClose={() => {
          setSelected(null);
          setPlan(null);
        }}
        title={selected?.name ?? "Pacote"}
        description={selected?.description}
        className="max-w-2xl"
      >
        {selected ? (
          <div className="space-y-3">
            <div className="flex flex-wrap items-center gap-2">
              <Badge variant="default">{selected.counts.agents} agentes</Badge>
              <Badge variant="default">{selected.counts.skills} skills</Badge>
              <Badge variant="default">{selected.counts.commands} comandos</Badge>
              <Badge variant="default">{selected.counts.recipes} receitas</Badge>
              <Badge variant="default">{selected.counts.routines} rotinas</Badge>
              {selected.installed ? (
                <Badge
                  variant={
                    selected.health.status === "partial"
                      ? "red"
                      : selected.update_available
                        ? "yellow"
                        : selected.health.status === "modified"
                          ? "purple"
                          : "green"
                  }
                >
                  {selected.health.status === "partial"
                    ? "parcial"
                    : selected.update_available
                      ? "atualização"
                      : selected.health.status === "modified"
                        ? "personalizado"
                        : "instalado"}
                </Badge>
              ) : null}
            </div>
            {selected.installed && selected.update_available ? (
              <p className="text-[10px] text-amber-200">
                Versão instalada {selected.installed_version ?? "?"}; disponível{" "}
                {selected.version}.
              </p>
            ) : null}
            {selected.health.status === "partial" ? (
              <p className="rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[10px] text-red-200">
                Faltam {selected.health.missing_files} arquivo(s) e{" "}
                {selected.health.missing_routines} rotina(s). Use “Reparar” para
                recriar o que falta; arquivos personalizados aparecem como
                conflito e exigem confirmação.
              </p>
            ) : null}

            {selected.requires.tools.length || selected.requires.integrations.length ? (
              <div className="rounded-lg border border-amber-500/20 bg-amber-500/[0.05] px-3 py-2 text-[10px] text-amber-200">
                <strong>Requisitos:</strong>{" "}
                {[...selected.requires.tools, ...selected.requires.integrations].join(", ")}
              </div>
            ) : null}

            {!activeWorkspaceId ? (
              <p className="rounded-lg border border-zinc-800 px-3 py-2 text-[10px] text-zinc-500">
                Selecione um workspace ativo para adicionar este pacote.
              </p>
            ) : planLoading ? (
              <LoadingState compact label="Calculando arquivos e conflitos…" />
            ) : plan ? (
              <>
                <div className="grid grid-cols-3 gap-2 text-center">
                  <div className="rounded-lg border border-zinc-800 px-2 py-1.5">
                    <CountPill value={plan.files.filter((file) => file.status === "create").length} tone="green" />
                    <p className="mt-1 text-[9px] text-zinc-500">novos</p>
                  </div>
                  <div className="rounded-lg border border-zinc-800 px-2 py-1.5">
                    <CountPill value={plan.files.filter((file) => file.status === "identical").length} />
                    <p className="mt-1 text-[9px] text-zinc-500">iguais</p>
                  </div>
                  <div className="rounded-lg border border-zinc-800 px-2 py-1.5">
                    <CountPill value={plan.conflicts} tone={plan.conflicts ? "red" : "default"} />
                    <p className="mt-1 text-[9px] text-zinc-500">conflitos</p>
                  </div>
                </div>
                {plan.conflicts > 0 ? (
                  <>
                    <p className="flex items-start gap-1.5 rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[10px] text-red-200">
                      <TriangleAlert size={12} className="mt-0.5 shrink-0" />
                      Marque os arquivos que podem ser substituídos. Os não marcados
                      serão mantidos como estão; substituições fazem backup.
                    </p>
                    <div className="max-h-32 space-y-1 overflow-y-auto rounded-lg border border-zinc-800/70 p-2">
                      {plan.files
                        .filter((file) => file.status === "conflict")
                        .map((file) => (
                          <label
                            key={file.path}
                            className="flex items-center gap-2 text-[10px] text-zinc-300"
                          >
                            <input
                              type="checkbox"
                              checked={overwritePaths.has(file.path)}
                              onChange={(event) =>
                                setOverwritePaths((current) => {
                                  const next = new Set(current);
                                  if (event.target.checked) next.add(file.path);
                                  else next.delete(file.path);
                                  return next;
                                })
                              }
                              className="accent-blue-500"
                            />
                            <span className="truncate font-mono text-[9px]">
                              {file.path}
                            </span>
                          </label>
                        ))}
                    </div>
                  </>
                ) : null}
                {plan.routines.length > 0 ? (
                  <p className="text-[10px] text-zinc-500">
                    {plan.routines.length} rotina(s) serão criadas <strong>inativas</strong>.
                  </p>
                ) : null}
                <div className="max-h-40 overflow-y-auto rounded-lg border border-zinc-800/70">
                  {plan.files.slice(0, 80).map((file) => (
                    <div
                      key={file.path}
                      className="flex items-center gap-2 border-b border-zinc-800/50 px-2.5 py-1 last:border-0"
                    >
                      <span className="min-w-0 flex-1 truncate font-mono text-[9px] text-zinc-400">
                        {file.path}
                      </span>
                      <Badge
                        variant={
                          file.status === "conflict"
                            ? "red"
                            : file.status === "identical"
                              ? "default"
                              : "green"
                        }
                      >
                        {file.status}
                      </Badge>
                    </div>
                  ))}
                </div>
              </>
            ) : null}

            <div className="flex flex-wrap justify-end gap-2 border-t border-zinc-800/70 pt-3">
              {selected.installed ? (
                <Button
                  variant="outline"
                  size="sm"
                  className="border-red-500/30 text-red-300"
                  onClick={() => void removePack()}
                  disabled={removing}
                >
                  <IconLabel icon={<Trash2 size={13} />}>Remover</IconLabel>
                </Button>
              ) : null}
              <Button
                size="sm"
                onClick={() => void installPack()}
                disabled={!activeWorkspaceId || installing || !plan}
              >
                <IconLabel
                  icon={
                    installing ? (
                      <RefreshCw size={13} className="animate-spin" />
                    ) : (
                      <Download size={13} />
                    )
                  }
                >
                  {selected.health.status === "partial"
                    ? "Reparar"
                    : selected.update_available
                      ? "Atualizar"
                      : selected.installed
                        ? "Reinstalar"
                        : "Adicionar ao workspace"}
                </IconLabel>
              </Button>
            </div>
            {selected.installed ? (
              <div className="flex flex-wrap gap-2 text-[10px] text-zinc-500">
                <Link to="/studio" className="hover:text-zinc-200">
                  ver agentes
                </Link>
                <Link to="/recipes" className="hover:text-zinc-200">
                  ver receitas
                </Link>
                <Link to="/routines" className="hover:text-zinc-200">
                  ver rotinas
                </Link>
              </div>
            ) : null}
          </div>
        ) : null}
      </Modal>

      <Modal
        open={templateOpen !== null}
        onClose={() => setTemplateOpen(null)}
        title={templateOpen ? `Criar workspace: ${templateOpen.name}` : "Novo workspace"}
        description={templateOpen?.description}
        className="max-w-lg"
      >
        {templateOpen ? (
          <div className="space-y-3">
            <label className="block">
              <span className={labelClass}>Nome do workspace</span>
              <input
                value={templateName}
                onChange={(event) => setTemplateName(event.target.value)}
                className={inputClass}
              />
            </label>
            <label className="block">
              <span className={labelClass}>Pasta</span>
              <PathInput
                value={templateRoot}
                onChange={setTemplateRoot}
                title="Escolher pasta do novo workspace"
                placeholder="vazio usa OpenDesk Workareas/<nome>"
                hint="A pasta precisa estar fora de /tmp. O OpenDesk cria a estrutura .opencode nela."
              />
            </label>
            <label className="block">
              <span className={labelClass}>Cor</span>
              <input
                type="color"
                value={templateColor}
                onChange={(event) => setTemplateColor(event.target.value)}
                className="h-9 w-full rounded-lg border border-zinc-800 bg-zinc-900/50 px-1"
              />
            </label>
            <p className="text-[10px] leading-snug text-zinc-500">
              Pacotes: {templateOpen.packs.join(", ")}. Rotinas entram inativas.
            </p>
            {templateErrors.length > 0 ? (
              <div className="rounded-lg border border-amber-500/25 bg-amber-500/10 px-3 py-2 text-[10px] text-amber-200">
                <p className="font-semibold">Instalação parcial</p>
                <ul className="mt-1 list-disc space-y-0.5 pl-4">
                  {templateErrors.map((error) => (
                    <li key={error}>{error}</li>
                  ))}
                </ul>
              </div>
            ) : null}
            <div className="flex justify-end gap-2 border-t border-zinc-800/70 pt-3">
              <Button variant="ghost" size="sm" onClick={() => setTemplateOpen(null)}>
                Cancelar
              </Button>
              <Button size="sm" onClick={() => void createTemplate()} disabled={creatingTemplate}>
                <IconLabel
                  icon={
                    creatingTemplate ? (
                      <RefreshCw size={13} className="animate-spin" />
                    ) : (
                      <FolderPlus size={13} />
                    )
                  }
                >
                  Criar workspace
                </IconLabel>
              </Button>
            </div>
          </div>
        ) : null}
      </Modal>

      <p className="flex items-center gap-1.5 text-[10px] text-zinc-600">
        <CheckCircle2 size={11} />
        Nada é instalado automaticamente. Remover um pacote não apaga arquivos que
        você modificou.
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
        <span className="text-[12px] font-medium text-zinc-200">
          {template.name}
        </span>
      </div>
      <p className="mt-1 text-[10px] leading-snug text-zinc-500">
        {template.description}
      </p>
      <p className="mt-2 text-[10px] text-zinc-600">
        {template.packs.length} pacotes
        {action ? ` · ${action}` : ""}
      </p>
    </button>
  );
}

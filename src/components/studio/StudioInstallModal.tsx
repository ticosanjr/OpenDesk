import { useEffect, useState } from "react";
import {
  Check,
  FolderOpen,
  Github,
  Globe,
  Loader2,
  Package,
  Plug,
  Terminal,
} from "lucide-react";
import { apiGet, apiPost } from "@/lib/api";
import type {
  StudioCatalog,
  StudioInstallItem,
  StudioInstallPreview,
} from "@/lib/types";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Modal } from "@/components/ui/modal";
import { pickFile, pickFolder } from "@/lib/folder";
import { useAppStore } from "@/store/app-store";
import { cn } from "@/lib/utils";

type SourceType = "catalog" | "github" | "url" | "local" | "mcp_json" | "npm";

const SOURCES: Array<{
  id: SourceType;
  label: string;
  icon: typeof Github;
  hint: string;
}> = [
  {
    id: "catalog",
    label: "Catálogo MCP",
    icon: Plug,
    hint: "Presets prontos: filesystem, git, fetch, memory, thinking e Playwright.",
  },
  {
    id: "github",
    label: "GitHub",
    icon: Github,
    hint: "owner/repo, owner/repo@ref ou github:owner/repo.",
  },
  {
    id: "url",
    label: "URL",
    icon: Globe,
    hint: "Link direto para SKILL.md, agente, comando, zip ou JSON de MCP.",
  },
  {
    id: "local",
    label: "Pasta/arquivo",
    icon: FolderOpen,
    hint: "Selecione um arquivo .md ou uma pasta com .opencode.",
  },
  {
    id: "mcp_json",
    label: "MCP JSON",
    icon: Terminal,
    hint: "Cole o objeto do servidor MCP.",
  },
  {
    id: "npm",
    label: "npm",
    icon: Package,
    hint: "Nome do pacote npm que expõe um servidor MCP.",
  },
];

export function StudioInstallModal({
  open,
  workspaceId,
  defaultScope,
  onClose,
  onInstalled,
  initialSource,
  preselectIds,
  lockSource = false,
}: {
  open: boolean;
  workspaceId: string | null;
  defaultScope: "workspace" | "global";
  onClose: () => void;
  onInstalled: (preview: StudioInstallPreview) => Promise<void>;
  /** Prefill the source and optionally lock the chooser (recommended skills). */
  initialSource?: { sourceType: SourceType; value: string; kind?: string };
  /** Keep only the matching IDs after preview (one installed skill). */
  preselectIds?: string[];
  lockSource?: boolean;
}) {
  const workspaces = useAppStore((state) => state.workspaces);
  const [sourceType, setSourceType] = useState<SourceType>("catalog");
  const [value, setValue] = useState("");
  const [name, setName] = useState("");
  const [kind, setKind] = useState("");
  const [catalog, setCatalog] = useState<StudioCatalog | null>(null);
  const [preview, setPreview] = useState<StudioInstallPreview | null>(null);
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const [scope, setScope] = useState<"workspace" | "global">(defaultScope);
  const [targetWorkspace, setTargetWorkspace] = useState(workspaceId ?? "");
  const [overwrite, setOverwrite] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    setPreview(null);
    setSelected(new Set());
    setError(null);
    setScope(defaultScope);
    setTargetWorkspace(workspaceId ?? "");
    if (initialSource) {
      setSourceType(initialSource.sourceType);
      setValue(initialSource.value);
      setKind(initialSource.kind ?? "skill");
      setName("");
      void previewSource({
        sourceType: initialSource.sourceType,
        value: initialSource.value,
        kind: initialSource.kind ?? "skill",
        name: "",
      });
    }
    // The source fields are intentionally read once per open.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, defaultScope, workspaceId, initialSource?.sourceType, initialSource?.value, initialSource?.kind]);

  useEffect(() => {
    if (!open || catalog) return;
    apiGet<StudioCatalog>("/api/studio/catalog")
      .then(setCatalog)
      .catch(() => setCatalog({ mcps: [] }));
  }, [open, catalog]);

  function setPreviewItems(items: StudioInstallItem[], warnings: string[] = []) {
    const seen = new Set<string>();
    const unique = items.filter((item) => {
      const key = `${item.kind}:${item.id}`;
      if (seen.has(key)) return false;
      seen.add(key);
      return true;
    });
    const visible = preselectIds?.length
      ? unique.filter((item) => preselectIds.includes(item.id))
      : unique;
    setPreview({ items: visible, warnings });
    setSelected(new Set(visible.map((_, index) => index)));
  }

  async function previewSource(override?: {
    sourceType: SourceType;
    value: string;
    kind?: string;
    name?: string;
  }) {
    const next = override ?? { sourceType, value, kind, name };
    if (!next.value.trim()) {
      setError("Informe a fonte.");
      return;
    }
    setBusy("preview");
    setError(null);
    try {
      const response = await apiPost<StudioInstallPreview>(
        "/api/studio/install/preview",
        {
          source_type: next.sourceType,
          value: next.value.trim(),
          kind: next.kind || null,
          name: next.name?.trim() || null,
        },
      );
      setPreviewItems(response.items, response.warnings);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(null);
    }
  }

  async function chooseLocal(directory: boolean) {
    const selectedPath = directory
      ? await pickFolder("Pasta para importar")
      : await pickFile("Arquivo para importar");
    if (selectedPath) setValue(selectedPath);
  }

  async function apply() {
    if (!preview) return;
    const items = preview.items.filter((_, index) => selected.has(index));
    if (items.length === 0) {
      setError("Selecione ao menos um item.");
      return;
    }
    setBusy("apply");
    setError(null);
    try {
      await apiPost("/api/studio/install/apply", {
        items,
        target_scope: scope,
        target_workspace_id: scope === "workspace" ? targetWorkspace : null,
        overwrite,
      });
      await onInstalled({ ...preview, items });
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(null);
    }
  }

  if (!open) return null;

  return (
    <Modal
      open={open}
      onClose={onClose}
      title="Instalar no OpenCode Studio"
      description="Importe agentes, skills, comandos e MCPs de fontes comuns. Nada é executado durante a instalação."
      size="lg"
    >
      <div className="space-y-3">
        {lockSource && initialSource ? (
          <div className="flex items-center gap-2 rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2.5">
            {initialSource.sourceType === "url" ? (
              <Globe size={15} className="shrink-0 text-zinc-400" />
            ) : (
              <Github size={15} className="shrink-0 text-zinc-400" />
            )}
            <div className="min-w-0 flex-1">
              <p className="text-[13.5px] font-medium text-zinc-200">
                {initialSource.sourceType === "url"
                  ? "URL direta"
                  : "Repositório GitHub"}
              </p>
              <p className="truncate font-mono text-[13px] text-zinc-500">
                {initialSource.value}
              </p>
            </div>
            <Badge variant="blue">skill</Badge>
          </div>
        ) : (
        <div className="grid gap-1 sm:grid-cols-3">
          {SOURCES.map((source) => (
            <button
              key={source.id}
              type="button"
              onClick={() => {
                setSourceType(source.id);
                setPreview(null);
                setError(null);
              }}
              className={cn(
                "flex items-start gap-2 rounded-lg border px-2.5 py-2 text-left",
                sourceType === source.id
                  ? "border-blue-500/50 bg-blue-500/10"
                  : "border-zinc-800 bg-zinc-950/40 hover:border-zinc-700",
              )}
            >
              <source.icon size={14} className="mt-0.5 text-zinc-400" />
              <div>
                <p className="text-[13.5px] font-medium text-zinc-200">{source.label}</p>
                <p className="mt-0.5 text-[13px] leading-relaxed text-zinc-500">
                  {source.hint}
                </p>
              </div>
            </button>
          ))}
        </div>
        )}

        {!lockSource && sourceType === "catalog" ? (
          <div className="space-y-3">
            {catalog?.repos?.length ? (
              <div className="space-y-1.5">
                <p className="text-[13px] font-semibold uppercase tracking-wide text-zinc-500">
                  Repositórios recomendados
                </p>
                {catalog.repos.map((repo) => (
                  <button
                    key={repo.id}
                    type="button"
                    onClick={() => {
                      setSourceType(repo.source_type);
                      setValue(repo.source);
                      setKind(repo.kind);
                      setError(null);
                    }}
                    className="flex w-full items-center gap-2 rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2 text-left transition-colors hover:border-blue-500/30 hover:bg-zinc-900/50"
                    title={`${repo.source_type}: ${repo.source}`}
                  >
                    <div className="min-w-0 flex-1">
                      <p className="text-[14px] font-medium text-zinc-200">
                        {repo.name}
                      </p>
                      <p className="truncate text-[11.5px] text-zinc-500">
                        {repo.description}
                      </p>
                    </div>
                    <Badge variant="blue">{repo.kind}</Badge>
                    <span className="font-mono text-[12px] text-zinc-500">
                      {repo.source}
                    </span>
                  </button>
                ))}
              </div>
            ) : null}

            <p className="text-[13px] font-semibold uppercase tracking-wide text-zinc-500">
              Presets MCP
            </p>
            {(catalog?.mcps ?? []).map((preset) => (
              <div
                key={preset.id}
                className="flex flex-wrap items-center gap-2 rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2"
              >
                <div className="min-w-0 flex-1">
                  <p className="text-[13px] text-zinc-200">{preset.name}</p>
                  <p className="truncate text-[11.5px] text-zinc-500">{preset.description}</p>
                </div>
                <Button
                  size="sm"
                  variant="outline"
                  onClick={() =>
                    setPreviewItems([
                      {
                        kind: "mcp",
                        id: preset.id,
                        name: preset.name,
                        description: preset.description,
                        body: "",
                        server: preset.server,
                      },
                    ])
                  }
                >
                  selecionar
                </Button>
              </div>
            ))}
          </div>
        ) : !lockSource ? (
          <div className="space-y-2">
            <div className="flex gap-2">
              <input
                value={value}
                onChange={(event) => setValue(event.target.value)}
                placeholder={
                  sourceType === "github"
                    ? "owner/repo ou github:owner/repo@main"
                    : sourceType === "url"
                      ? "https://…"
                      : sourceType === "local"
                        ? "/caminho/para/pasta"
                        : sourceType === "npm"
                          ? "@modelcontextprotocol/server-filesystem"
                          : '{ "type": "local", "command": [...] }'
                }
                className={inputClass}
              />
              {sourceType === "local" ? (
                <>
                  <Button variant="outline" size="sm" onClick={() => void chooseLocal(true)}>
                    <FolderOpen size={12} />
                    pasta
                  </Button>
                  <Button variant="outline" size="sm" onClick={() => void chooseLocal(false)}>
                    arquivo
                  </Button>
                </>
              ) : null}
            </div>
            <div className="grid gap-2 sm:grid-cols-3">
              <input
                value={name}
                onChange={(event) => setName(event.target.value)}
                placeholder="ID/nome (opcional)"
                className={inputClass}
              />
              <select
                value={kind}
                onChange={(event) => setKind(event.target.value)}
                className={inputClass}
                aria-label="Tipo do item"
              >
                <option value="">tipo automático</option>
                <option value="agent">agente</option>
                <option value="skill">skill</option>
                <option value="command">comando</option>
                <option value="mcp">MCP</option>
              </select>
              <Button
                size="sm"
                onClick={() => void previewSource()}
                disabled={busy !== null}
              >
                {busy === "preview" ? (
                  <Loader2 size={13} className="animate-spin" />
                ) : (
                  <Package size={13} />
                )}
                Analisar
              </Button>
            </div>
          </div>
        ) : null}

        {preview ? (
          <div className="space-y-2 rounded-lg border border-zinc-800/70 bg-zinc-950/40 p-3">
            <div className="flex items-center gap-2">
              <p className="text-[13px] font-medium text-zinc-200">
                Itens detectados
              </p>
              <Badge variant="blue">{preview.items.length}</Badge>
            </div>
            {preview.items.map((item, index) => (
              <label
                key={`${item.kind}:${item.id}:${index}`}
                className="flex items-center gap-2 text-[13.5px] text-zinc-400"
              >
                <input
                  type="checkbox"
                  checked={selected.has(index)}
                  onChange={(event) => {
                    const next = new Set(selected);
                    if (event.target.checked) next.add(index);
                    else next.delete(index);
                    setSelected(next);
                  }}
                  className="accent-blue-500"
                />
                <Badge variant="default">{item.kind}</Badge>
                <span className="font-mono text-zinc-300">{item.id}</span>
                {item.name ? <span className="text-zinc-500">{item.name}</span> : null}
              </label>
            ))}
            {preview.warnings.length > 0 ? (
              <div className="text-[13px] text-amber-300/80">
                {preview.warnings.slice(0, 5).map((warning) => (
                  <p key={warning}>⚠ {warning}</p>
                ))}
              </div>
            ) : null}
            <div className="grid gap-2 sm:grid-cols-3">
              <select
                value={scope}
                onChange={(event) => setScope(event.target.value as "workspace" | "global")}
                className={inputClass}
                aria-label="Escopo da instalação"
              >
                <option value="workspace">workspace</option>
                <option value="global">global</option>
              </select>
              {scope === "workspace" ? (
                <select
                  value={targetWorkspace}
                  onChange={(event) => setTargetWorkspace(event.target.value)}
                  className={inputClass}
                  aria-label="Workspace de destino"
                >
                  {workspaces.map((workspace) => (
                    <option key={workspace.id} value={workspace.id}>
                      {workspace.name}
                    </option>
                  ))}
                </select>
              ) : (
                <div />
              )}
              <label className="flex items-center gap-2 text-[13.5px] text-zinc-400">
                <input
                  type="checkbox"
                  checked={overwrite}
                  onChange={(event) => setOverwrite(event.target.checked)}
                  className="accent-blue-500"
                />
                sobrescrever
              </label>
            </div>
          </div>
        ) : null}

        {error ? (
          <p className="rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[13.5px] text-red-300">
            {error}
          </p>
        ) : null}

        <div className="sticky bottom-0 z-10 -mx-6 -mb-6 mt-4 flex justify-end gap-2 border-t border-zinc-800/70 bg-zinc-950/95 px-6 py-3 backdrop-blur">
          <Button variant="ghost" size="sm" onClick={onClose}>
            Cancelar
          </Button>
          <Button
            size="sm"
            onClick={() => void apply()}
            disabled={!preview || selected.size === 0 || busy !== null}
          >
            {busy === "apply" ? (
              <Loader2 size={13} className="animate-spin" />
            ) : (
              <Check size={13} />
            )}
            Instalar selecionados
          </Button>
        </div>
      </div>
    </Modal>
  );
}

const inputClass =
  "h-9 w-full rounded-lg border border-zinc-800 bg-zinc-900/50 px-2.5 text-[14px] text-zinc-100 outline-none placeholder:text-zinc-500 transition-colors hover:border-zinc-700 focus:border-blue-500/60";

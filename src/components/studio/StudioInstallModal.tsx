import { useEffect, useState } from "react";
import { Check, FolderOpen, Loader2, Package } from "lucide-react";
import { apiGet, apiPost } from "@/lib/api";
import type {
  StudioCatalog,
  StudioInstallItem,
  StudioInstallPreview,
} from "@/lib/types";
import { Button } from "@/components/ui/button";
import { Modal } from "@/components/ui/modal";
import { pickFile, pickFolder } from "@/lib/folder";
import { useAppStore } from "@/store/app-store";
import { InstallCatalogPane } from "./install/InstallCatalogPane";
import { InstallPreviewPane } from "./install/InstallPreviewPane";
import { InstallSourcePicker } from "./install/InstallSourcePicker";
import { inputClass, type SourceType } from "./install/install-sources";

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
        <InstallSourcePicker
          lockSource={lockSource}
          initialSource={initialSource}
          sourceType={sourceType}
          onSelect={(next) => {
            setSourceType(next);
            setPreview(null);
            setError(null);
          }}
        />

        {!lockSource && sourceType === "catalog" ? (
          <InstallCatalogPane
            catalog={catalog}
            onPickRepo={(repo) => {
              setSourceType(repo.source_type);
              setValue(repo.source);
              setKind(repo.kind);
              setError(null);
            }}
            onSelectPreset={(preset) =>
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
          />
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

        <InstallPreviewPane
          preview={preview}
          selected={selected}
          onToggle={(index, checked) => {
            const next = new Set(selected);
            if (checked) next.add(index);
            else next.delete(index);
            setSelected(next);
          }}
          scope={scope}
          onScopeChange={setScope}
          workspaces={workspaces}
          targetWorkspace={targetWorkspace}
          onTargetWorkspaceChange={setTargetWorkspace}
          overwrite={overwrite}
          onOverwriteChange={setOverwrite}
        />

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

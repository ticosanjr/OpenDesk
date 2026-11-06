import { useCallback, useEffect, useMemo, useState } from "react";
import { useSearchParams } from "react-router-dom";
import {
  ChevronRight,
  Copy,
  ExternalLink,
  FileText,
  Folder,
  FolderOpen,
  FolderTree,
  Home,
  RefreshCw,
  Search,
} from "lucide-react";
import { openPath } from "@tauri-apps/plugin-opener";
import { PageBody, PageHeader } from "@/components/layout/PageHeader";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { EmptyState } from "@/components/ui/empty-state";
import { useAppStore } from "@/store/app-store";
import { useUiStore } from "@/store/ui-store";
import { apiGet } from "@/lib/api";
import type { FileContent, FileEntry } from "@/lib/types";
import { formatNumber, relativeTime, shortPath } from "@/lib/format";
import { cn } from "@/lib/utils";
import { toast } from "@/store/toast-store";
import { FileViewer } from "@/components/viewer/FileViewer";

export function FilesPage() {
  const workspaces = useAppStore((state) => state.workspaces);
  const activeWorkspaceId = useAppStore((state) => state.activeWorkspaceId);
  const setWorkspaceDialogOpen = useUiStore(
    (state) => state.setWorkspaceDialogOpen,
  );
  const workspace = workspaces.find((item) => item.id === activeWorkspaceId);

  const [directory, setDirectory] = useState("");
  const [entries, setEntries] = useState<FileEntry[]>([]);
  const [selected, setSelected] = useState<FileEntry | null>(null);
  const [content, setContent] = useState<FileContent | null>(null);
  const [filter, setFilter] = useState("");
  const [loading, setLoading] = useState(false);
  const [loadingFile, setLoadingFile] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [params, setParams] = useSearchParams();
  const deepPath = params.get("path");

  const loadTree = useCallback(
    async (path: string) => {
      if (!activeWorkspaceId) return;
      setLoading(true);
      try {
        const list = await apiGet<FileEntry[]>(
          `/api/files/tree?workspace_id=${encodeURIComponent(activeWorkspaceId)}&path=${encodeURIComponent(path)}`,
        );
        setEntries(list);
        setDirectory(path);
        setError(null);
      } catch (cause) {
        setError(cause instanceof Error ? cause.message : String(cause));
        setEntries([]);
      } finally {
        setLoading(false);
      }
    },
    [activeWorkspaceId],
  );

  useEffect(() => {
    if (!activeWorkspaceId) return;
    setSelected(null);
    setContent(null);
    void loadTree("");
  }, [activeWorkspaceId, loadTree]);

  useEffect(() => {
    if (!activeWorkspaceId || !deepPath) return;
    const parent = deepPath.includes("/")
      ? deepPath.slice(0, deepPath.lastIndexOf("/"))
      : "";
    setSelected({
      name: deepPath.split("/").pop() ?? deepPath,
      path: deepPath,
      is_dir: false,
      size: 0,
    });
    setLoadingFile(true);
    setContent(null);
    void Promise.all([
      loadTree(parent),
      apiGet<FileContent>(
        `/api/files/content?workspace_id=${encodeURIComponent(activeWorkspaceId)}&path=${encodeURIComponent(deepPath)}`,
      ),
    ])
      .then(([, value]) => {
        setContent(value);
        setError(null);
      })
      .catch((cause) => {
        setError(cause instanceof Error ? cause.message : String(cause));
      })
      .finally(() => {
        setLoadingFile(false);
        setParams({}, { replace: true });
      });
  }, [activeWorkspaceId, deepPath, loadTree, setParams]);

  async function openEntry(entry: FileEntry) {
    if (entry.is_dir) {
      setSelected(null);
      setContent(null);
      setFilter("");
      await loadTree(entry.path);
      return;
    }
    setSelected(entry);
    setLoadingFile(true);
    setContent(null);
    try {
      const value = await apiGet<FileContent>(
        `/api/files/content?workspace_id=${encodeURIComponent(activeWorkspaceId ?? "")}&path=${encodeURIComponent(entry.path)}`,
      );
      setContent(value);
      setError(null);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setLoadingFile(false);
    }
  }

  async function reveal(path?: string) {
    if (!workspace) return;
    const target = path ? `${workspace.root_path}/${path}` : workspace.root_path;
    try {
      await openPath(target);
    } catch (cause) {
      console.warn("OpenDesk: could not open path", cause);
      toast({
        tone: "error",
        title: "Não foi possível abrir a pasta",
        description: String(cause),
      });
    }
  }

  async function copyPath(value: string) {
    try {
      await navigator.clipboard.writeText(value);
      toast({ tone: "success", title: "Caminho copiado", description: value });
    } catch {
      toast({ tone: "error", title: "Não foi possível copiar o caminho" });
    }
  }

  const crumbs = useMemo(() => {
    const parts = directory.split("/").filter(Boolean);
    return [
      { label: "workspace", path: "" },
      ...parts.map((part, index) => ({
        label: part,
        path: parts.slice(0, index + 1).join("/"),
      })),
    ];
  }, [directory]);

  const filtered = useMemo(() => {
    const needle = filter.trim().toLowerCase();
    if (!needle) return entries;
    return entries.filter((entry) => entry.name.toLowerCase().includes(needle));
  }, [entries, filter]);

  if (!activeWorkspaceId || !workspace) {
    return (
      <PageBody>
        <PageHeader eyebrow="explorador" title="Arquivos" />
        <EmptyState
          icon={FolderTree}
          title="Nenhum workspace ativo"
          description="Selecione ou crie um workspace para navegar por input/, output/ e .opencode/."
          action={
            <Button size="sm" onClick={() => setWorkspaceDialogOpen(true)}>
              Criar workspace
            </Button>
          }
        />
      </PageBody>
    );
  }

  return (
    <PageBody>
      <PageHeader
        eyebrow="explorador"
        title="Arquivos"
        description={`${workspace.name} · ${shortPath(workspace.root_path, 64)}`}
        actions={
          <>
            <Button variant="outline" size="sm" onClick={() => void reveal(directory)}>
              <ExternalLink size={13} />
              Abrir pasta
            </Button>
            <Button
              variant="outline"
              size="sm"
              onClick={() => void loadTree(directory)}
              disabled={loading}
            >
              <RefreshCw size={13} className={loading ? "animate-spin" : undefined} />
              Atualizar
            </Button>
          </>
        }
      />

      {error ? (
        <p className="rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[11px] text-red-300">
          {error}
        </p>
      ) : null}

      <div className="grid gap-3 lg:grid-cols-[minmax(280px,380px)_1fr]">
        <Card className="flex max-h-[70vh] min-h-[420px] flex-col">
          <CardHeader className="gap-2 px-3.5 py-3">
            <div className="flex w-full flex-wrap items-center gap-1 text-[10px] text-zinc-500">
              {crumbs.map((crumb, index) => (
                <span key={crumb.path || "root"} className="flex items-center gap-1">
                  {index > 0 ? <ChevronRight size={10} className="text-zinc-700" /> : null}
                  <button
                    type="button"
                    onClick={() => void loadTree(crumb.path)}
                    className={cn(
                      "flex items-center gap-1 rounded px-1 py-0.5 hover:bg-zinc-800 hover:text-zinc-200",
                      index === crumbs.length - 1 && "text-zinc-200",
                    )}
                  >
                    {index === 0 ? <Home size={10} /> : null}
                    {crumb.label}
                  </button>
                </span>
              ))}
            </div>
            <div className="flex h-8 w-full items-center gap-2 rounded-lg border border-zinc-800 bg-zinc-950/40 px-2.5">
              <Search size={13} className="text-zinc-600" />
              <input
                value={filter}
                onChange={(event) => setFilter(event.target.value)}
                placeholder="Filtrar nesta pasta"
                className="h-full flex-1 bg-transparent text-[11px] outline-none placeholder:text-zinc-600"
              />
            </div>
          </CardHeader>
          <CardContent className="min-h-0 flex-1 overflow-y-auto p-1.5">
            {loading ? (
              <p className="p-3 text-[10px] text-zinc-600">Carregando…</p>
            ) : filtered.length === 0 ? (
              <EmptyState
                icon={FolderOpen}
                title="Pasta vazia"
                description="Nenhum arquivo visível nesta pasta."
                className="border-0 bg-transparent py-8"
              />
            ) : (
              filtered.map((entry) => (
                <button
                  key={entry.path}
                  type="button"
                  onClick={() => void openEntry(entry)}
                  className={cn(
                    "flex w-full items-center gap-2 rounded-lg px-2 py-1.5 text-left transition-colors hover:bg-zinc-900",
                    selected?.path === entry.path && "bg-zinc-800/80",
                  )}
                >
                  {entry.is_dir ? (
                    <Folder size={14} className="shrink-0 text-amber-400/70" />
                  ) : (
                    <FileText size={14} className="shrink-0 text-zinc-500" />
                  )}
                  <span className="min-w-0 flex-1">
                    <span className="block truncate text-[11px] text-zinc-300">
                      {entry.name}
                    </span>
                    <span className="block truncate font-mono text-[9px] text-zinc-600">
                      {entry.is_dir
                        ? "pasta"
                        : `${formatNumber(entry.size)} B · ${relativeTime(entry.modified)}`}
                    </span>
                  </span>
                  {entry.is_dir ? (
                    <ChevronRight size={12} className="text-zinc-700" />
                  ) : null}
                </button>
              ))
            )}
          </CardContent>
        </Card>

        <Card className="flex max-h-[70vh] min-h-[420px] flex-col">
          <CardHeader className="px-3.5 py-3">
            <div className="min-w-0">
              <CardTitle className="flex items-center gap-2 text-[12px]">
                <FileText size={14} className="text-zinc-500" />
                {selected?.name ?? "Pré-visualização"}
              </CardTitle>
              <p className="mt-0.5 truncate font-mono text-[9px] text-zinc-600">
                {content?.path ?? selected?.path ?? "selecione um arquivo"}
              </p>
            </div>
            {content ? (
              <div className="flex shrink-0 items-center gap-1">
                <Badge variant="default">{formatNumber(content.size)} B</Badge>
                <Button
                  variant="ghost"
                  size="icon"
                  className="h-7 w-7"
                  title="Copiar caminho"
                  onClick={() => void copyPath(content.path)}
                >
                  <Copy size={12} />
                </Button>
                <Button
                  variant="ghost"
                  size="icon"
                  className="h-7 w-7"
                  title="Abrir arquivo"
                  onClick={() => void reveal(content.path)}
                >
                  <ExternalLink size={12} />
                </Button>
              </div>
            ) : null}
          </CardHeader>
          <CardContent className="min-h-0 flex-1 overflow-hidden p-0">
            {loadingFile ? (
              <p className="p-3 text-[11px] text-zinc-600">Carregando arquivo…</p>
            ) : !selected ? (
              <EmptyState
                icon={FileText}
                title="Escolha um arquivo"
                description="Navegue pela árvore à esquerda para visualizar Markdown, HTML, imagens, PDF, vídeo, áudio, JSON e código."
                className="border-0 bg-transparent"
              />
            ) : content ? (
              <FileViewer
                name={selected.name}
                path={content.path}
                content={content.content}
                binary={content.binary}
                mime={content.mime}
                truncated={content.truncated}
                size={content.size}
                rawUrl={`/api/files/raw?workspace_id=${encodeURIComponent(activeWorkspaceId)}&path=${encodeURIComponent(content.path)}`}
                onOpenExternal={() => void reveal(content.path)}
              />
            ) : (
              <p className="p-3 text-[11px] text-zinc-600">Sem conteúdo.</p>
            )}
          </CardContent>
        </Card>
      </div>
    </PageBody>
  );
}

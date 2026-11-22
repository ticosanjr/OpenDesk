import { useCallback, useEffect, useMemo, useState } from "react";
import { useSearchParams } from "react-router-dom";
import { ExternalLink, FolderTree, RefreshCw } from "lucide-react";
import { openWorkspacePath } from "@/lib/open-path";
import { PageBody, PageBodyFill, PageHeader } from "@/components/layout/PageHeader";
import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/ui/empty-state";
import { useAppStore } from "@/store/app-store";
import { useUiStore } from "@/store/ui-store";
import { apiGet } from "@/lib/api";
import type { FileContent, FileEntry } from "@/lib/types";
import { shortPath } from "@/lib/format";
import { toast } from "@/store/toast-store";
import type { ArchiveEntry } from "@/components/viewer/FileViewer";
import { saveLocalFile } from "@/lib/save-file";
import { FileListCard } from "./files/FileListCard";
import { FilePreviewCard } from "./files/FilePreviewCard";
import type { Crumb, FileScope, MediaInfo } from "./files/types";

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
  const [scope, setScope] = useState<FileScope>("folder");
  const [searchResults, setSearchResults] = useState<FileEntry[]>([]);
  const [searching, setSearching] = useState(false);
  const [loading, setLoading] = useState(false);
  const [loadingFile, setLoadingFile] = useState(false);
  const [archive, setArchive] = useState<ArchiveEntry[] | null>(null);
  const [archiveLoading, setArchiveLoading] = useState(false);
  const [archiveError, setArchiveError] = useState<string | null>(null);
  const [media, setMedia] = useState<MediaInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [params, setParams] = useSearchParams();
  const deepPath = params.get("path");
  const deepDir = params.get("dir") === "1";

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
    if (deepDir) {
      setSelected(null);
      setContent(null);
      setLoadingFile(false);
      void loadTree(deepPath).finally(() =>
        setParams({}, { replace: true }),
      );
      return;
    }
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
        void loadMedia(deepPath);
        setParams({}, { replace: true });
      });
    // loadMedia only depends on activeWorkspaceId, which is already listed.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [activeWorkspaceId, deepPath, deepDir, loadTree, setParams]);

  const MEDIA_EXTENSIONS = /\.(mp4|mkv|webm|mov|avi|mp3|wav|flac|ogg|m4a|aac)$/i;

  useEffect(() => {
    const path =
      content?.binary && /\.zip$/i.test(content.path) ? content.path : null;
    setArchive(null);
    setArchiveError(null);
    if (!path || !activeWorkspaceId) {
      setArchiveLoading(false);
      return;
    }
    let cancelled = false;
    setArchiveLoading(true);
    void apiGet<ArchiveEntry[]>(
      `/api/files/archive?workspace_id=${encodeURIComponent(activeWorkspaceId)}&path=${encodeURIComponent(path)}`,
    )
      .then((list) => {
        if (!cancelled) setArchive(Array.isArray(list) ? list : []);
      })
      .catch((cause) => {
        if (!cancelled) {
          setArchiveError(cause instanceof Error ? cause.message : String(cause));
        }
      })
      .finally(() => {
        if (!cancelled) setArchiveLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [activeWorkspaceId, content]);

  async function loadMedia(path: string) {
    if (!MEDIA_EXTENSIONS.test(path)) {
      setMedia(null);
      return;
    }
    try {
      setMedia(
        await apiGet(
          `/api/files/media?workspace_id=${encodeURIComponent(activeWorkspaceId ?? "")}&path=${encodeURIComponent(path)}`,
        ),
      );
    } catch {
      setMedia(null);
    }
  }

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
    void loadMedia(entry.path);
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

  async function saveCurrentFile() {
    if (!content || !activeWorkspaceId) return;
    try {
      const saved = await saveLocalFile({
        filename: selected?.name ?? content.path.split("/").pop() ?? "arquivo",
        workspaceId: activeWorkspaceId,
        path: content.path,
        fallbackUrl: `/api/files/raw?workspace_id=${encodeURIComponent(activeWorkspaceId)}&path=${encodeURIComponent(content.path)}`,
      });
      if (saved) {
        toast({
          tone: "success",
          title: "Download salvo",
          description: content.path,
        });
      }
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível baixar",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    }
  }

  async function reveal(path?: string) {
    if (!workspace) return;
    const target = path ? `${workspace.root_path}/${path}` : workspace.root_path;
    try {
      await openWorkspacePath(target);
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

  const crumbs = useMemo<Crumb[]>(() => {
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
    if (scope === "workspace") return searchResults;
    const needle = filter.trim().toLowerCase();
    if (!needle) return entries;
    return entries.filter((entry) => entry.name.toLowerCase().includes(needle));
  }, [entries, filter, scope, searchResults]);

  useEffect(() => {
    if (scope !== "workspace" || !activeWorkspaceId) {
      setSearchResults([]);
      return;
    }
    const needle = filter.trim();
    if (needle.length < 2) {
      setSearchResults([]);
      return;
    }
    let cancelled = false;
    const handle = window.setTimeout(() => {
      setSearching(true);
      void apiGet<FileEntry[]>(
        `/api/files/search?workspace_id=${encodeURIComponent(activeWorkspaceId)}&q=${encodeURIComponent(needle)}`,
      )
        .then((value) => {
          if (!cancelled) setSearchResults(value);
        })
        .catch((cause) => {
          if (!cancelled) {
            setSearchResults([]);
            setError(cause instanceof Error ? cause.message : String(cause));
          }
        })
        .finally(() => {
          if (!cancelled) setSearching(false);
        });
    }, 250);
    return () => {
      cancelled = true;
      window.clearTimeout(handle);
    };
  }, [activeWorkspaceId, filter, scope]);

  async function openSearchResult(entry: FileEntry) {
    if (entry.is_dir) {
      await loadTree(entry.path);
      setDirectory(entry.path);
      return;
    }
    const parent = entry.path.includes("/")
      ? entry.path.slice(0, entry.path.lastIndexOf("/"))
      : "";
    await loadTree(parent);
    setDirectory(parent);
    await openEntry(entry);
  }

  if (!activeWorkspaceId || !workspace) {
    return (
      <PageBody>
        <PageHeader title="Arquivos" />
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
    <PageBodyFill>
      <div className="shrink-0">
      <PageHeader
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

      </div>

      {error ? (
        <p className="shrink-0 rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[13px] text-red-300">
          {error}
        </p>
      ) : null}

      <div className="grid min-h-0 flex-1 gap-3 overflow-hidden lg:grid-cols-[minmax(280px,380px)_1fr]">
        <FileListCard
          crumbs={crumbs}
          filter={filter}
          onFilterChange={setFilter}
          scope={scope}
          onScopeChange={setScope}
          onLoadTree={loadTree}
          loading={loading}
          searching={searching}
          filtered={filtered}
          selected={selected}
          onEntryClick={(entry) =>
            void (scope === "workspace"
              ? openSearchResult(entry)
              : openEntry(entry))
          }
        />

        <FilePreviewCard
          selected={selected}
          content={content}
          media={media}
          loadingFile={loadingFile}
          workspaceId={activeWorkspaceId}
          archive={archive}
          archiveLoading={archiveLoading}
          archiveError={archiveError}
          onCopyPath={(path) => void copyPath(path)}
          onReveal={(path) => void reveal(path)}
          onSave={() => void saveCurrentFile()}
        />
      </div>
    </PageBodyFill>
  );
}

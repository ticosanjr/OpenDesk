import { useEffect, useMemo, useState } from "react";
import {
  Download,
  ExternalLink,
  FileText,
  FolderOpen,
  Loader2,
  Music,
  RefreshCw,
  ShieldCheck,
  Video,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { formatNumber } from "@/lib/format";
import { downloadUrl } from "@/lib/download";
import {
  loadFilePreferences,
  saveFilePreferences,
  type FilePreferences,
} from "@/lib/file-preferences";
import { formatJsonContent } from "@/lib/json-format";
import { toast } from "@/store/toast-store";
import { cn } from "@/lib/utils";
import { FilePreviewBody } from "@/components/viewer/FilePreviewBody";
import { kindFor, useAuthBlob } from "@/components/viewer/preview-helpers";
import type { ArchiveEntry } from "@/components/viewer/preview-helpers";

export type { ArchiveEntry } from "@/components/viewer/preview-helpers";

export function FileViewer({
  name,
  path,
  content,
  mime,
  binary,
  truncated,
  size,
  rawUrl,
  onOpenExternal,
  onOpenFolder,
  onDownload,
  archive = null,
  archiveLoading = false,
  archiveError = null,
  embedded = false,
}: {
  name: string;
  path: string;
  content: string;
  mime?: string;
  binary?: boolean;
  truncated?: boolean;
  size?: number;
  rawUrl?: string;
  onOpenExternal?: () => void;
  onOpenFolder?: () => void;
  onDownload?: () => void | Promise<void>;
  /** Entries of a ZIP archive, listed without extracting it. */
  archive?: ArchiveEntry[] | null;
  archiveLoading?: boolean;
  archiveError?: string | null;
  /** Parent already renders the file path, size and actions. */
  embedded?: boolean;
}) {
  const kind = useMemo(() => kindFor(name, mime), [mime, name]);
  const needsBlob = ["image", "pdf", "video", "audio"].includes(kind);
  const blobUrl = useAuthBlob(needsBlob ? rawUrl : null);
  const [tab, setTab] = useState<"preview" | "source">("preview");
  const [downloading, setDownloading] = useState(false);
  const [allowScripts, setAllowScripts] = useState(false);
  const [reloadKey, setReloadKey] = useState(0);
  const [preferences, setPreferences] = useState<FilePreferences | null>(null);
  const textSource =
    kind === "text" || kind === "markdown" || kind === "html" || kind === "json";
  const isArchive =
    kind === "binary" &&
    (/\.zip$/i.test(name) || mime === "application/zip" || mime === "application/x-zip-compressed");

  useEffect(() => {
    let cancelled = false;
    void loadFilePreferences().then((loaded) => {
      if (!cancelled) {
        setPreferences(loaded);
        setAllowScripts(loaded.htmlAllowScripts);
      }
    });
    return () => {
      cancelled = true;
    };
  }, [path]);

  const jsonPreview = useMemo(
    () => (kind === "json" ? formatJsonContent(content) : null),
    [content, kind],
  );

  async function toggleScripts() {
    const next = !allowScripts;
    setAllowScripts(next);
    setReloadKey((current) => current + 1);
    if (preferences) {
      const updated = { ...preferences, htmlAllowScripts: next };
      setPreferences(updated);
      try {
        await saveFilePreferences(updated);
      } catch {
        // The preference is best-effort; the preview already unblocked.
      }
    }
  }

  async function saveFile() {
    if (onDownload) {
      setDownloading(true);
      try {
        await onDownload();
      } finally {
        setDownloading(false);
      }
      return;
    }
    if (!rawUrl) return;
    setDownloading(true);
    try {
      await downloadUrl(rawUrl, name);
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível baixar",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setDownloading(false);
    }
  }

  const showToolbar =
    !embedded ||
    truncated ||
    kind === "html" ||
    kind === "csv" ||
    textSource ||
    Boolean(blobUrl);

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      {showToolbar ? (
        <div className="flex flex-wrap items-center gap-2 border-b border-zinc-800/70 px-3 py-2">
          {embedded ? null : (
            <>
              <FileText size={13} className="text-zinc-500" />
              <span className="min-w-0 flex-1 truncate font-mono text-[13px] text-zinc-300">
                {path || name}
              </span>
              <Badge variant="default">{kind}</Badge>
              {size !== undefined ? (
                <Badge variant="default">{formatNumber(size)} B</Badge>
              ) : null}
            </>
          )}
          {truncated ? <Badge variant="yellow">truncado</Badge> : null}
          {kind === "html" ? (
            <>
              <button
                type="button"
                onClick={() => void toggleScripts()}
                className="inline-flex items-center gap-1 rounded-md border border-zinc-800 px-1.5 py-0.5 text-[11.5px] text-zinc-400 hover:border-zinc-600 hover:text-zinc-200"
                title={
                  allowScripts
                    ? "Bloquear scripts nesta visualização"
                    : "Permitir scripts nesta visualização"
                }
              >
                <ShieldCheck size={10} />
                {allowScripts ? "scripts liberados" : "scripts bloqueados"}
              </button>
              <button
                type="button"
                onClick={() => setReloadKey((current) => current + 1)}
                className="inline-flex items-center gap-1 rounded-md border border-zinc-800 px-1.5 py-0.5 text-[11.5px] text-zinc-400 hover:border-zinc-600 hover:text-zinc-200"
                title="Recarregar a prévia HTML"
              >
                <RefreshCw size={10} />
                recarregar
              </button>
            </>
          ) : null}
          {textSource ? (
            <span className={cn("flex items-center gap-1 rounded-md border border-zinc-800 p-0.5", embedded && "ml-auto")}>
              {(["preview", "source"] as const).map((value) => (
                <button
                  key={value}
                  type="button"
                  onClick={() => setTab(value)}
                  className={cn(
                    "rounded px-1.5 py-0.5 text-[12px]",
                    tab === value
                      ? "bg-zinc-800 text-zinc-100"
                      : "text-zinc-500 hover:text-zinc-200",
                  )}
                >
                  {value === "preview" ? "visualização" : "fonte"}
                </button>
              ))}
            </span>
          ) : null}
          {!embedded && onOpenExternal ? (
            <Button variant="ghost" size="sm" className="h-7" onClick={onOpenExternal}>
              <ExternalLink size={11} />
              abrir
            </Button>
          ) : null}
          {onOpenFolder ? (
            <Button
              variant="ghost"
              size="sm"
              className="h-7"
              onClick={onOpenFolder}
              title="Abrir a pasta do arquivo"
              aria-label="Abrir a pasta do arquivo"
            >
              <FolderOpen size={11} />
            </Button>
          ) : null}
          {onDownload ? (
            <Button
              variant="ghost"
              size="sm"
              className="h-7"
              onClick={() => void saveFile()}
              disabled={downloading}
              title={`Baixar ${name}`}
            >
              {downloading ? (
                <Loader2 size={11} className="animate-spin" />
              ) : (
                <Download size={11} />
              )}
              baixar
            </Button>
          ) : blobUrl ? (
            <a
              href={blobUrl}
              download={name}
              className="inline-flex h-7 items-center gap-1 rounded-lg px-2 text-[13px] text-zinc-400 hover:bg-zinc-900 hover:text-zinc-100"
            >
              <Download size={11} />
              baixar
            </a>
          ) : rawUrl ? (
            <Button
              variant="ghost"
              size="sm"
              className="h-7"
              onClick={() => void saveFile()}
              disabled={downloading}
              title={`Baixar ${name}`}
            >
              {downloading ? (
                <Loader2 size={11} className="animate-spin" />
              ) : (
                <Download size={11} />
              )}
              baixar
            </Button>
          ) : null}
        </div>
      ) : null}

      <FilePreviewBody
        kind={kind}
        tab={tab}
        content={content}
        allowScripts={allowScripts}
        reloadKey={reloadKey}
        name={name}
        jsonPreview={jsonPreview}
        blobUrl={blobUrl}
        binary={binary}
        isArchive={isArchive}
        archive={archive}
        archiveLoading={archiveLoading}
        archiveError={archiveError}
        rawUrl={rawUrl}
        downloading={downloading}
        onDownload={() => void saveFile()}
      />

      {kind === "video" || kind === "audio" ? (
        <div className="flex items-center gap-1 border-t border-zinc-800/70 px-3 py-2 text-[12px] text-zinc-500">
          {kind === "video" ? <Video size={11} /> : <Music size={11} />}
          use os controles nativos para reproduzir
        </div>
      ) : null}
    </div>
  );
}

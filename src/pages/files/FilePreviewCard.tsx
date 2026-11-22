import { Copy, ExternalLink, FileText } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { EmptyState } from "@/components/ui/empty-state";
import { LoadingState } from "@/components/ui/loading-state";
import { LazyFileViewer } from "@/components/viewer/LazyFileViewer";
import type { ArchiveEntry } from "@/components/viewer/FileViewer";
import type { FileContent, FileEntry } from "@/lib/types";
import { formatNumber } from "@/lib/format";
import type { MediaInfo } from "./types";

function containingFolder(path: string): string {
  const index = path.lastIndexOf("/");
  return index > 0 ? path.slice(0, index) : "";
}

export function FilePreviewCard({
  selected,
  content,
  media,
  loadingFile,
  workspaceId,
  archive,
  archiveLoading,
  archiveError,
  onCopyPath,
  onReveal,
  onSave,
}: {
  selected: FileEntry | null;
  content: FileContent | null;
  media: MediaInfo | null;
  loadingFile: boolean;
  workspaceId: string;
  archive: ArchiveEntry[] | null;
  archiveLoading: boolean;
  archiveError: string | null;
  onCopyPath: (path: string) => void;
  onReveal: (path: string) => void;
  onSave: () => void;
}) {
  return (
    <Card className="flex h-full min-h-0 flex-col">
      <CardHeader className="px-3.5 py-3">
        <div className="min-w-0">
          <CardTitle className="flex items-center gap-2 text-[13.5px]">
            <FileText size={14} className="text-zinc-500" />
            {selected?.name ?? "Pré-visualização"}
          </CardTitle>
          {content || selected ? (
            (content?.path ?? selected?.path) !== selected?.name ? (
              <p className="mt-0.5 truncate font-mono text-[13px] text-zinc-500">
                {content?.path ?? selected?.path}
              </p>
            ) : null
          ) : (
            <p className="mt-0.5 font-mono text-[13px] text-zinc-500">
              selecione um arquivo
            </p>
          )}
          {media ? (
            <p className="mt-1 flex flex-wrap items-center gap-1 text-[11.5px] text-zinc-500">
              {media.duration_secs ? (
                <Badge variant="default">
                  {Math.floor(media.duration_secs / 60)}:
                  {String(Math.floor(media.duration_secs % 60)).padStart(2, "0")}
                </Badge>
              ) : null}
              {media.format ? <Badge variant="blue">{media.format}</Badge> : null}
              {media.streams
                .filter((stream) => stream.type === "video")
                .slice(0, 2)
                .map((stream, index) => (
                  <Badge key={index} variant="purple">
                    {stream.codec}
                    {stream.width && stream.height
                      ? ` ${stream.width}x${stream.height}`
                      : ""}
                  </Badge>
                ))}
              {media.streams
                .filter((stream) => stream.type === "audio")
                .slice(0, 1)
                .map((stream, index) => (
                  <Badge key={index} variant="green">
                    {stream.codec ?? "audio"}
                    {stream.channels ? ` ${stream.channels}ch` : ""}
                  </Badge>
                ))}
            </p>
          ) : null}
        </div>
        {content ? (
          <div className="flex shrink-0 items-center gap-1">
            <Badge variant="default">{formatNumber(content.size)} B</Badge>
            <Button
              variant="ghost"
              size="icon"
              className="h-8 w-8"
              aria-label="Copiar caminho"
              title="Copiar caminho"
              onClick={() => onCopyPath(content.path)}
            >
              <Copy size={12} />
            </Button>
            <Button
              variant="ghost"
              size="icon"
              className="h-8 w-8"
              aria-label="Abrir arquivo"
              title="Abrir arquivo"
              onClick={() => onReveal(content.path)}
            >
              <ExternalLink size={12} />
            </Button>
          </div>
        ) : null}
      </CardHeader>
      <CardContent className="flex min-h-0 flex-1 flex-col overflow-hidden p-0">
        {loadingFile ? (
          <LoadingState compact label="Carregando arquivo…" rows={3} />
        ) : !selected ? (
          <EmptyState
            icon={FileText}
            title="Escolha um arquivo"
            description="Navegue pela árvore à esquerda para visualizar Markdown, HTML, imagens, PDF, vídeo, áudio, JSON e código."
            compact
            className="border-0 bg-transparent"
          />
        ) : content ? (
          <LazyFileViewer
            embedded
            name={selected.name}
            path={content.path}
            content={content.content}
            binary={content.binary}
            mime={content.mime}
            truncated={content.truncated}
            size={content.size}
            rawUrl={`/api/files/raw?workspace_id=${encodeURIComponent(workspaceId)}&path=${encodeURIComponent(content.path)}`}
            onOpenExternal={() => onReveal(content.path)}
            onOpenFolder={() =>
              onReveal(containingFolder(content.path) || ".")
            }
            onDownload={onSave}
            archive={archive}
            archiveLoading={archiveLoading}
            archiveError={archiveError}
          />
        ) : (
          <p className="p-3 text-[13px] text-zinc-500">Sem conteúdo.</p>
        )}
      </CardContent>
    </Card>
  );
}

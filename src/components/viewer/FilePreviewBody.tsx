import { Download, FileArchive, Loader2 } from "lucide-react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { formatNumber } from "@/lib/format";
import type { ArchiveEntry, Kind } from "@/components/viewer/preview-helpers";

export function FilePreviewBody({
  kind,
  tab,
  content,
  allowScripts,
  reloadKey,
  name,
  jsonPreview,
  blobUrl,
  zoom,
  binary,
  isArchive,
  archive,
  archiveLoading,
  archiveError,
  rawUrl,
  downloading,
  onDownload,
}: {
  kind: Kind;
  tab: "preview" | "source";
  content: string;
  allowScripts: boolean;
  reloadKey: number;
  name: string;
  jsonPreview: { formatted: boolean; text: string } | null;
  blobUrl: string | null;
  zoom: number;
  binary?: boolean;
  isArchive: boolean;
  archive: ArchiveEntry[] | null;
  archiveLoading: boolean;
  archiveError: string | null;
  rawUrl?: string;
  downloading: boolean;
  onDownload: () => void;
}) {
  return (
    <div className="min-h-0 flex-1 overflow-auto bg-zinc-950/40 p-3">
      {tab === "source" ? (
        <pre className="whitespace-pre-wrap break-words font-mono text-[13px] leading-relaxed text-zinc-300">
          {content || "Conteúdo não disponível."}
        </pre>
      ) : kind === "markdown" ? (
        <div className="markdown-body max-w-none">
          <ReactMarkdown remarkPlugins={[remarkGfm]}>{content}</ReactMarkdown>
        </div>
      ) : kind === "html" ? (
        <iframe
          key={`${allowScripts ? "scripts" : "blocked"}-${reloadKey}`}
          title={name}
          sandbox={allowScripts ? "allow-scripts" : ""}
          srcDoc={content}
          className="h-full min-h-[420px] w-full rounded-lg border border-zinc-800 bg-white"
        />
      ) : kind === "json" ? (
        <div className="space-y-2">
          {jsonPreview && !jsonPreview.formatted ? (
            <p className="rounded-lg border border-amber-500/20 bg-amber-500/10 px-3 py-2 text-[12px] text-amber-200">
              JSON inválido ou com comentários; exibindo o texto original.
            </p>
          ) : null}
          <pre className="whitespace-pre-wrap break-words font-mono text-[13px] leading-relaxed text-zinc-300">
            {jsonPreview?.text ?? content}
          </pre>
        </div>
      ) : kind === "image" ? (
        <div className="flex min-h-[320px] items-center justify-center">
          {blobUrl ? (
            <img
              src={blobUrl}
              alt={name}
              style={{ maxWidth: `${zoom}%` }}
              className="max-h-[70vh] rounded-lg object-contain"
            />
          ) : (
            <p className="text-[13px] text-zinc-500">Carregando imagem…</p>
          )}
        </div>
      ) : kind === "pdf" ? (
        blobUrl ? (
          <iframe
            title={name}
            src={blobUrl}
            className="h-full min-h-[520px] w-full rounded-lg border border-zinc-800 bg-white"
          />
        ) : (
          <p className="text-[13px] text-zinc-500">Carregando PDF…</p>
        )
      ) : kind === "video" ? (
        blobUrl ? (
          <video
            src={blobUrl}
            controls
            className="max-h-[70vh] w-full rounded-lg bg-black"
          />
        ) : (
          <p className="text-[13px] text-zinc-500">Carregando vídeo…</p>
        )
      ) : kind === "audio" ? (
        blobUrl ? (
          <audio src={blobUrl} controls className="w-full" />
        ) : (
          <p className="text-[13px] text-zinc-500">Carregando áudio…</p>
        )
      ) : binary ? (
        isArchive ? (
          <div className="space-y-2">
            <p className="flex items-center gap-2 text-[13px] text-zinc-400">
              <FileArchive size={14} className="text-amber-300" />
              Conteúdo do ZIP
              {archive ? (
                <Badge variant="default">{archive.length} entradas</Badge>
              ) : null}
            </p>
            {archiveLoading ? (
              <p className="flex items-center gap-2 py-6 text-[13px] text-zinc-500">
                <Loader2 size={13} className="animate-spin" />
                Lendo o arquivo…
              </p>
            ) : archiveError ? (
              <p className="rounded-lg border border-amber-500/20 bg-amber-500/10 px-3 py-2 text-[13px] text-amber-200">
                Não foi possível listar o ZIP: {archiveError}
              </p>
            ) : archive && archive.length > 0 ? (
              <div className="scroll-area max-h-[420px] overflow-auto rounded-lg border border-zinc-800/70">
                <table className="w-full text-left text-[13px]">
                  <thead className="sticky top-0 bg-zinc-950/95 text-[11.5px] uppercase tracking-wide text-zinc-500">
                    <tr>
                      <th className="px-3 py-2">Entrada</th>
                      <th className="px-3 py-2 text-right">Tamanho</th>
                      <th className="px-3 py-2 text-right">Comprimido</th>
                    </tr>
                  </thead>
                  <tbody>
                    {archive.map((entry, index) => (
                      <tr
                        key={`${index}-${entry.name}`}
                        className="border-t border-zinc-800/60 font-mono"
                      >
                        <td className="max-w-[520px] truncate px-3 py-1.5 text-zinc-300">
                          {entry.name}
                        </td>
                        <td className="px-3 py-1.5 text-right text-zinc-500">
                          {entry.is_dir ? "—" : formatNumber(entry.size)}
                        </td>
                        <td className="px-3 py-1.5 text-right text-zinc-500">
                          {entry.is_dir ? "—" : formatNumber(entry.compressed)}
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
                {/* Keep this cap in sync with MAX_ARCHIVE_ENTRIES in api/files.rs. */}
                {archive.length >= 400 ? (
                  <p className="border-t border-zinc-800/70 px-3 py-1.5 text-[11.5px] text-zinc-500">
                    Mostrando as primeiras 400 entradas do ZIP.
                  </p>
                ) : null}
              </div>
            ) : archive ? (
              <p className="rounded-lg border border-dashed border-zinc-800 px-3 py-6 text-center text-[13px] text-zinc-500">
                ZIP vazio.
              </p>
            ) : (
              <p className="text-[13px] text-zinc-500">
                Listando as entradas do ZIP…
              </p>
            )}
          </div>
        ) : (
          <div className="flex flex-col items-center justify-center gap-3 py-16 text-center">
            <Download size={24} className="text-zinc-500" />
            <p className="text-[13.5px] text-zinc-400">
              Este formato não tem prévia interna. Baixe ou abra externamente.
            </p>
            {rawUrl ? (
              <Button
                variant="outline"
                size="sm"
                onClick={onDownload}
                disabled={downloading}
              >
                <Download size={12} />
                Baixar arquivo
              </Button>
            ) : null}
          </div>
        )
      ) : (
        <pre className="whitespace-pre-wrap break-words font-mono text-[13px] leading-relaxed text-zinc-300">
          {content || "Sem conteúdo textual."}
        </pre>
      )}
    </div>
  );
}

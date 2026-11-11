import { useEffect, useMemo, useState } from "react";
import {
  Download,
  ExternalLink,
  FileText,
  Image as ImageIcon,
  Music,
  Video,
} from "lucide-react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { getEndpoint } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { formatNumber } from "@/lib/format";
import { cn } from "@/lib/utils";

type Kind =
  | "markdown"
  | "html"
  | "image"
  | "pdf"
  | "video"
  | "audio"
  | "text"
  | "binary";

function kindFor(name: string, mime?: string): Kind {
  const ext = name.split(".").pop()?.toLowerCase() ?? "";
  if (
    mime?.startsWith("image/") ||
    ["png", "jpg", "jpeg", "gif", "webp", "avif", "svg", "ico"].includes(ext)
  )
    return "image";
  if (mime === "application/pdf" || ext === "pdf") return "pdf";
  if (mime?.startsWith("video/") || ["mp4", "webm", "mov", "m4v"].includes(ext))
    return "video";
  if (mime?.startsWith("audio/") || ["mp3", "wav", "ogg", "m4a"].includes(ext))
    return "audio";
  if (ext === "md" || ext === "markdown" || ext === "mdx") return "markdown";
  if (ext === "html" || ext === "htm") return "html";
  if (
    [
      "json",
      "jsonc",
      "jsonl",
      "yaml",
      "yml",
      "toml",
      "csv",
      "tsv",
      "js",
      "ts",
      "tsx",
      "jsx",
      "rs",
      "py",
      "sh",
      "css",
      "sql",
      "log",
      "txt",
    ].includes(ext)
  )
    return "text";
  return "binary";
}

function useAuthBlob(url?: string | null) {
  const [blobUrl, setBlobUrl] = useState<string | null>(null);
  useEffect(() => {
    if (!url) {
      setBlobUrl(null);
      return;
    }
    let cancelled = false;
    let objectUrl: string | null = null;
    void getEndpoint()
      .then(({ port, token }) =>
        fetch(`http://127.0.0.1:${port}${url}`, {
          headers: { Authorization: `Bearer ${token}` },
        }),
      )
      .then((response) => {
        if (!response.ok) throw new Error(`preview failed (${response.status})`);
        return response.blob();
      })
      .then((blob) => {
        if (cancelled) return;
        objectUrl = URL.createObjectURL(blob);
        setBlobUrl(objectUrl);
      })
      .catch(() => {
        if (!cancelled) setBlobUrl(null);
      });
    return () => {
      cancelled = true;
      if (objectUrl) URL.revokeObjectURL(objectUrl);
    };
  }, [url]);
  return blobUrl;
}

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
}) {
  const kind = useMemo(() => kindFor(name, mime), [mime, name]);
  const needsBlob = ["image", "pdf", "video", "audio"].includes(kind);
  const blobUrl = useAuthBlob(needsBlob ? rawUrl : null);
  const [tab, setTab] = useState<"preview" | "source">("preview");
  const [zoom, setZoom] = useState(100);
  const textSource = kind === "text" || kind === "markdown" || kind === "html";

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex flex-wrap items-center gap-2 border-b border-zinc-800/70 px-3 py-2">
        <FileText size={13} className="text-zinc-500" />
        <span className="min-w-0 flex-1 truncate font-mono text-[11px] text-zinc-300">
          {path || name}
        </span>
        <Badge variant="default">{kind}</Badge>
        {size !== undefined ? (
          <Badge variant="default">{formatNumber(size)} B</Badge>
        ) : null}
        {truncated ? <Badge variant="yellow">truncado</Badge> : null}
        {kind === "html" ? <Badge variant="yellow">scripts bloqueados</Badge> : null}
        {textSource ? (
          <span className="flex items-center gap-1 rounded-md border border-zinc-800 p-0.5">
            {(["preview", "source"] as const).map((value) => (
              <button
                key={value}
                type="button"
                onClick={() => setTab(value)}
                className={cn(
                  "rounded px-1.5 py-0.5 text-[10px]",
                  tab === value
                    ? "bg-zinc-800 text-zinc-100"
                    : "text-zinc-500 hover:text-zinc-200",
                )}
              >
                {value === "preview" ? "prévia" : "fonte"}
              </button>
            ))}
          </span>
        ) : null}
        {onOpenExternal ? (
          <Button variant="ghost" size="sm" className="h-7" onClick={onOpenExternal}>
            <ExternalLink size={11} />
            abrir
          </Button>
        ) : null}
        {blobUrl ? (
          <a
            href={blobUrl}
            download={name}
            className="inline-flex h-7 items-center gap-1 rounded-lg px-2 text-[11px] text-zinc-400 hover:bg-zinc-900 hover:text-zinc-100"
          >
            <Download size={11} />
            baixar
          </a>
        ) : null}
      </div>

      <div className="min-h-0 flex-1 overflow-auto bg-zinc-950/40 p-3">
        {tab === "source" ? (
          <pre className="whitespace-pre-wrap break-words font-mono text-[11px] leading-relaxed text-zinc-300">
            {content || "Conteúdo não disponível."}
          </pre>
        ) : kind === "markdown" ? (
          <div className="markdown-body max-w-none">
            <ReactMarkdown remarkPlugins={[remarkGfm]}>{content}</ReactMarkdown>
          </div>
        ) : kind === "html" ? (
          <iframe
            title={name}
            sandbox=""
            srcDoc={content}
            className="h-full min-h-[420px] w-full rounded-lg border border-zinc-800 bg-white"
          />
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
              <p className="text-[11px] text-zinc-500">Carregando imagem…</p>
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
            <p className="text-[11px] text-zinc-500">Carregando PDF…</p>
          )
        ) : kind === "video" ? (
          blobUrl ? (
            <video
              src={blobUrl}
              controls
              className="max-h-[70vh] w-full rounded-lg bg-black"
            />
          ) : (
            <p className="text-[11px] text-zinc-500">Carregando vídeo…</p>
          )
        ) : kind === "audio" ? (
          blobUrl ? (
            <audio src={blobUrl} controls className="w-full" />
          ) : (
            <p className="text-[11px] text-zinc-500">Carregando áudio…</p>
          )
        ) : binary ? (
          <div className="flex flex-col items-center justify-center gap-3 py-16 text-center">
            <Download size={24} className="text-zinc-500" />
            <p className="text-[12px] text-zinc-400">
              Este formato não tem prévia interna. Baixe ou abra externamente.
            </p>
          </div>
        ) : (
          <pre className="whitespace-pre-wrap break-words font-mono text-[11px] leading-relaxed text-zinc-300">
            {content || "Sem conteúdo textual."}
          </pre>
        )}
      </div>
      {kind === "image" && blobUrl ? (
        <div className="flex items-center gap-2 border-t border-zinc-800/70 px-3 py-2">
          <ImageIcon size={11} className="text-zinc-500" />
          <input
            type="range"
            min={25}
            max={200}
            value={zoom}
            onChange={(event) => setZoom(Number(event.target.value))}
            className="h-1 flex-1 accent-blue-500"
          />
          <span className="font-mono text-[10px] text-zinc-500">{zoom}%</span>
        </div>
      ) : null}
      {kind === "video" || kind === "audio" ? (
        <div className="flex items-center gap-1 border-t border-zinc-800/70 px-3 py-2 text-[10px] text-zinc-500">
          {kind === "video" ? <Video size={11} /> : <Music size={11} />}
          use os controles nativos para reproduzir
        </div>
      ) : null}
    </div>
  );
}

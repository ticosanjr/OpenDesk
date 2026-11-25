import { useEffect, useState } from "react";
import { getEndpoint } from "@/lib/api";

export interface ArchiveEntry {
  name: string;
  size: number;
  compressed: number;
  is_dir: boolean;
}

export type Kind =
  | "markdown"
  | "html"
  | "json"
  | "csv"
  | "image"
  | "pdf"
  | "video"
  | "audio"
  | "text"
  | "binary";

export function kindFor(name: string, mime?: string): Kind {
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
  if (mime === "application/json" || ["json", "jsonc", "jsonl"].includes(ext))
    return "json";
  if (ext === "csv" || ext === "tsv") return "csv";
  if (
    [
      "yaml",
      "yml",
      "toml",
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

/** Best-effort language label for the code viewer header. */
export function languageFor(name: string): string {
  const ext = name.split(".").pop()?.toLowerCase() ?? "";
  const languages: Record<string, string> = {
    ts: "TypeScript",
    tsx: "TSX",
    js: "JavaScript",
    jsx: "JSX",
    rs: "Rust",
    py: "Python",
    sh: "Shell",
    css: "CSS",
    sql: "SQL",
    yaml: "YAML",
    yml: "YAML",
    toml: "TOML",
    log: "Log",
    txt: "Texto",
  };
  return languages[ext] ?? (ext ? ext.toUpperCase() : "Texto");
}

export function useAuthBlob(url?: string | null) {
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

import { useRef, useState } from "react";
import { Loader2, Paperclip, X } from "lucide-react";
import { apiDelete, apiPost } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { basename } from "@/lib/path";
import { cn } from "@/lib/utils";

export function PromptAttachments({
  workspaceId,
  value,
  onChange,
  label = "Anexar arquivos",
  className,
}: {
  workspaceId?: string | null;
  value: string[];
  onChange: (paths: string[]) => void;
  label?: string;
  className?: string;
}) {
  const input = useRef<HTMLInputElement | null>(null);
  const [uploading, setUploading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function upload(files: File[]) {
    if (!workspaceId || files.length === 0) return;
    setUploading(true);
    setError(null);
    try {
      const uploaded: string[] = [];
      for (const file of files) {
        if (file.size > 20 * 1024 * 1024) {
          setError(`${file.name} é maior que 20 MB.`);
          continue;
        }
        const data = await new Promise<string>((resolve, reject) => {
          const reader = new FileReader();
          reader.onload = () => resolve(String(reader.result ?? ""));
          reader.onerror = () => reject(reader.error);
          reader.readAsDataURL(file);
        });
        const saved = await apiPost<{ path: string }>(
          `/api/attachments?workspace_id=${encodeURIComponent(workspaceId)}`,
          { filename: file.name, base64: data },
        );
        uploaded.push(saved.path);
      }
      if (uploaded.length > 0) onChange([...value, ...uploaded]);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setUploading(false);
      if (input.current) input.current.value = "";
    }
  }

  function remove(path: string) {
    onChange(value.filter((item) => item !== path));
    if (!workspaceId) return;
    void apiDelete(
      `/api/attachments?workspace_id=${encodeURIComponent(workspaceId)}&path=${encodeURIComponent(path)}`,
    ).catch(() => {
      // The prompt already lost the reference; an orphan file is harmless.
    });
  }

  return (
    <div className={cn("space-y-1.5", className)}>
      <input
        ref={input}
        type="file"
        multiple
        className="hidden"
        onChange={(event) => void upload(Array.from(event.target.files ?? []))}
      />
      <div className="flex flex-wrap items-center gap-2">
        <Button
          type="button"
          variant="outline"
          size="sm"
          onClick={() => input.current?.click()}
          disabled={!workspaceId || uploading}
        >
          {uploading ? (
            <Loader2 size={12} className="animate-spin" />
          ) : (
            <Paperclip size={12} />
          )}
          {label}
        </Button>
        <span className="text-[9px] text-zinc-500">
          {value.length > 0
            ? `${value.length} anexo(s) — caminhos adicionados ao prompt`
            : "opcional, até 20 MB por arquivo"}
        </span>
      </div>
      {value.length > 0 ? (
        <div className="flex flex-wrap gap-1.5">
          {value.map((path) => (
            <span
              key={path}
              className="inline-flex max-w-full items-center gap-1 rounded-md border border-zinc-700 bg-zinc-900 px-1.5 py-1 text-[9px] text-zinc-400"
            >
              <Paperclip size={9} />
              <span className="max-w-48 truncate">{basename(path)}</span>
              <button
                type="button"
                aria-label={`Remover ${basename(path)}`}
                onClick={() => remove(path)}
                className="text-zinc-500 hover:text-zinc-200"
              >
                <X size={9} />
              </button>
            </span>
          ))}
        </div>
      ) : null}
      {error ? <p className="text-[10px] text-red-300">{error}</p> : null}
    </div>
  );
}

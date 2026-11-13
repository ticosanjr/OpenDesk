import { useCallback, useEffect, useMemo, useState } from "react";
import { FileJson, Loader2, RefreshCw, Save } from "lucide-react";
import { apiGet, apiPut } from "@/lib/api";
import type { ConfigDocument, StudioScope } from "@/lib/types";
import { useAppStore } from "@/store/app-store";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { StudioError } from "@/components/studio/StudioList";

export function ConfigPanel({
  scope,
  workspaceId,
}: {
  scope: StudioScope;
  workspaceId: string | null;
}) {
  const lastFsEvent = useAppStore((state) => state.lastFsEvent);
  const query = useMemo(
    () => `scope=${scope}&workspace_id=${encodeURIComponent(workspaceId ?? "")}`,
    [scope, workspaceId],
  );

  const [document, setDocument] = useState<ConfigDocument | null>(null);
  const [content, setContent] = useState("");
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const response = await apiGet<ConfigDocument>(`/api/studio/config?${query}`);
      setDocument(response);
      setContent(response.content);
      setError(null);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setLoading(false);
    }
  }, [query]);

  useEffect(() => {
    void load();
  }, [load, lastFsEvent?.path]);

  async function save() {
    setSaving(true);
    setError(null);
    try {
      const response = await apiPut<ConfigDocument>(`/api/studio/config?${query}`, {
        content,
        expected_sha256: document?.exists ? document.sha256 : null,
      });
      setDocument(response);
      setContent(response.content);
    } catch (cause) {
      const message = cause instanceof Error ? cause.message : String(cause);
      setError(
        message.includes("changed on disk")
          ? "O opencode.jsonc mudou no disco. Recarregue para não sobrescrever a edição externa."
          : message,
      );
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col overflow-hidden p-5">
      <div className="mb-3 flex flex-wrap items-start justify-between gap-3">
        <div>
          <div className="flex items-center gap-2">
            <FileJson size={16} className="text-zinc-500" />
            <h2 className="text-sm font-semibold text-zinc-100">opencode.jsonc</h2>
            <Badge variant="blue">{scope}</Badge>
            {document ? (
              <Badge variant={document.exists ? "green" : "default"}>
                {document.exists ? "existente" : "será criado"}
              </Badge>
            ) : null}
          </div>
          <p className="mt-1 font-mono text-[11.5px] text-zinc-500">
            {document?.path ?? "carregando…"}
          </p>
        </div>
        <div className="flex gap-2">
          <Button variant="outline" size="sm" onClick={() => void load()} disabled={loading}>
            <RefreshCw size={13} className={loading ? "animate-spin" : undefined} />
            Recarregar
          </Button>
          <Button size="sm" onClick={() => void save()} disabled={saving}>
            {saving ? <Loader2 size={14} className="animate-spin" /> : <Save size={14} />}
            Salvar
          </Button>
        </div>
      </div>

      <StudioError error={error} />

      <div className="min-h-0 flex-1">
        <textarea
          value={content}
          onChange={(event) => setContent(event.target.value)}
          spellCheck={false}
          className="h-full min-h-0 w-full resize-none rounded-xl border border-zinc-800 bg-zinc-950/70 p-4 font-mono text-[13.5px] leading-relaxed text-zinc-200 outline-none focus:border-blue-500/60"
        />
      </div>

      <div className="mt-3 flex flex-wrap items-center gap-2 rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2 font-mono text-[11.5px] text-zinc-500">
        <span>sha256</span>
        <span>{document?.sha256 ? `${document.sha256.slice(0, 20)}…` : "—"}</span>
        <span className="ml-auto">
          comentários e formatação externos são preservados até você salvar
        </span>
      </div>
    </div>
  );
}

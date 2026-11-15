import { useCallback, useEffect, useState } from "react";
import { Clipboard, Loader2, MessageSquare, RefreshCw } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { EmptyState } from "@/components/ui/empty-state";
import { apiGet } from "@/lib/api";
import type { ChatPromptPreview, ChatThread } from "@/lib/types";
import { useAppStore } from "@/store/app-store";
import { toast } from "@/store/toast-store";
import { cn } from "@/lib/utils";
import { smallInputClass as inputClass } from "@/lib/ui-classes";
import {
  PromptPreviewTabs,
  type PromptPreviewView,
} from "./PromptPreviewTabs";
import { OpenCodeLayerView } from "./OpenCodeLayerView";

export function ChatPromptPreviewCard() {
  const workspaceId = useAppStore((state) => state.activeWorkspaceId);
  const [threads, setThreads] = useState<ChatThread[]>([]);
  const [selected, setSelected] = useState("");
  const [content, setContent] = useState("");
  const [preview, setPreview] = useState<ChatPromptPreview | null>(null);
  const [view, setView] = useState<PromptPreviewView>("sections");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!workspaceId) {
      setThreads([]);
      return;
    }
    void apiGet<ChatThread[]>(
      `/api/chat/threads?workspace_id=${encodeURIComponent(workspaceId)}`,
    )
      .then((list) => {
        setThreads(Array.isArray(list) ? list : []);
        setSelected((current) =>
          current && list.some((item) => item.id === current)
            ? current
            : (list[0]?.id ?? ""),
        );
      })
      .catch((cause) =>
        setError(cause instanceof Error ? cause.message : String(cause)),
      );
  }, [workspaceId]);

  const load = useCallback(async () => {
    if (!selected) {
      setPreview(null);
      return;
    }
    setLoading(true);
    try {
      const query = content.trim()
        ? `?content=${encodeURIComponent(content.trim())}`
        : "";
      const value = await apiGet<ChatPromptPreview>(
        `/api/chat/threads/${encodeURIComponent(selected)}/prompt-preview${query}`,
      );
      setPreview(value);
      setError(null);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setLoading(false);
    }
  }, [content, selected]);

  useEffect(() => {
    void load();
  }, [load]);

  async function copy(value: string) {
    try {
      await navigator.clipboard.writeText(value);
      toast({ tone: "success", title: "Copiado" });
    } catch {
      toast({ tone: "error", title: "Não foi possível copiar" });
    }
  }

  return (
    <Card>
      <CardHeader>
        <div>
          <CardTitle className="flex items-center gap-2">
            <MessageSquare size={15} className="text-zinc-500" />
            Prompt do chat
          </CardTitle>
          <p className="mt-1 text-xs text-zinc-500">
            Veja o que o OpenDesk envia antes de cada mensagem do chat, com o
            contexto do canal e a camada do OpenCode.
          </p>
        </div>
        <Button variant="ghost" size="sm" onClick={() => void load()} disabled={loading}>
          <RefreshCw size={13} className={loading ? "animate-spin" : undefined} />
          Atualizar
        </Button>
      </CardHeader>
      <CardContent className="space-y-3">
        {error ? (
          <p className="rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[12px] text-red-300">
            {error}
          </p>
        ) : null}
        {threads.length === 0 ? (
          <EmptyState
            icon={MessageSquare}
            title="Nenhuma conversa neste workspace"
            description="Abra o Chat ou o Telegram para gerar uma conversa."
            compact
          />
        ) : (
          <>
            <div className="flex flex-wrap items-center gap-2">
              <select
                value={selected}
                onChange={(event) => setSelected(event.target.value)}
                className={cn(inputClass, "min-w-64 flex-1")}
                aria-label="Conversa para inspecionar"
              >
                {threads.map((thread) => (
                  <option key={thread.id} value={thread.id}>
                    {thread.title} · {thread.source}
                  </option>
                ))}
              </select>
              <PromptPreviewTabs view={view} onChange={setView} />
            </div>

            <label>
              <span className="mb-1 block text-[12px] font-medium uppercase tracking-[0.12em] text-zinc-500">
                Próxima mensagem (opcional)
              </span>
              <div className="flex gap-2">
                <input
                  value={content}
                  onChange={(event) => setContent(event.target.value)}
                  placeholder="Deixe vazio para usar a última mensagem do usuário"
                  className={inputClass}
                />
                <Button size="sm" onClick={() => void load()} disabled={loading}>
                  Prévia
                </Button>
              </div>
            </label>

            {preview ? (
              <>
                <div className="flex flex-wrap items-center gap-1.5 text-[11.5px] text-zinc-500">
                  <Badge variant="blue">{preview.thread.source}</Badge>
                  <span className="font-mono">@{preview.thread.agent}</span>
                  <span className="font-mono">{preview.thread.model ?? "modelo auto"}</span>
                </div>

                {view === "sections" ? (
                  <div className="space-y-2">
                    {preview.sections.map((section) => (
                      <div
                        key={section.id}
                        className="rounded-lg border border-zinc-800/70 bg-zinc-950/30"
                      >
                        <div className="flex flex-wrap items-center gap-2 border-b border-zinc-800/60 px-3 py-2">
                          <Badge variant="blue">{section.source}</Badge>
                          <p className="text-[12.5px] font-medium text-zinc-300">
                            {section.label}
                          </p>
                        </div>
                        <pre className="max-h-72 overflow-auto whitespace-pre-wrap px-3 py-2 font-mono text-[11.5px] leading-relaxed text-zinc-400">
                          {section.content}
                        </pre>
                      </div>
                    ))}
                  </div>
                ) : null}

                {view === "desk" ? (
                  <div className="space-y-2">
                    <div className="flex justify-end">
                      <Button
                        variant="outline"
                        size="sm"
                        onClick={() => void copy(preview.prompt)}
                      >
                        <Clipboard size={12} />
                        Copiar prompt
                      </Button>
                    </div>
                    <pre className="max-h-[480px] overflow-auto whitespace-pre-wrap rounded-lg border border-zinc-800 bg-zinc-950/50 p-3 font-mono text-[11.5px] leading-relaxed text-zinc-400">
                      {preview.prompt}
                    </pre>
                  </div>
                ) : null}

                {view === "opencode" ? (
                  <OpenCodeLayerView layer={preview.opencode} />
                ) : null}

                {view === "final" ? (
                  <div className="space-y-2">
                    <p className="rounded-lg border border-amber-500/20 bg-amber-500/[0.04] px-3 py-2 text-[11.5px] text-amber-200/80">
                      {preview.opencode.note}
                    </p>
                    <div className="flex justify-end">
                      <Button
                        variant="outline"
                        size="sm"
                        onClick={() => void copy(preview.composed)}
                      >
                        <Clipboard size={12} />
                        Copiar reconstruído
                      </Button>
                    </div>
                    <pre className="max-h-[560px] overflow-auto whitespace-pre-wrap rounded-lg border border-zinc-800 bg-zinc-950/50 p-3 font-mono text-[11.5px] leading-relaxed text-zinc-400">
                      {preview.composed}
                    </pre>
                  </div>
                ) : null}
              </>
            ) : loading ? (
              <p className="flex items-center gap-2 text-[12px] text-zinc-500">
                <Loader2 size={12} className="animate-spin" />
                Montando prompt do chat…
              </p>
            ) : null}
          </>
        )}
      </CardContent>
    </Card>
  );
}

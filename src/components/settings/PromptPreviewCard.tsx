import { useCallback, useEffect, useState } from "react";
import { Clipboard, FileText, Loader2, RefreshCw, Sparkles } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { EmptyState } from "@/components/ui/empty-state";
import { apiGet } from "@/lib/api";
import type { PromptPreview, Task } from "@/lib/types";
import { useAppStore } from "@/store/app-store";
import { toast } from "@/store/toast-store";
import { cn } from "@/lib/utils";
import { smallInputClass as inputClass } from "@/lib/ui-classes";

type View = "sections" | "prompt" | "opencode";

export function PromptPreviewCard() {
  const workspaceId = useAppStore((state) => state.activeWorkspaceId);
  const [tasks, setTasks] = useState<Task[]>([]);
  const [selected, setSelected] = useState<string>("");
  const [preview, setPreview] = useState<PromptPreview | null>(null);
  const [view, setView] = useState<View>("sections");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!workspaceId) {
      setTasks([]);
      return;
    }
    void apiGet<Task[]>(
      `/api/tasks?workspace_id=${encodeURIComponent(workspaceId)}`,
    )
      .then((list) => {
        setTasks(Array.isArray(list) ? list : []);
        setSelected((current) =>
          current && list.some((task) => task.id === current)
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
      const value = await apiGet<PromptPreview>(
        `/api/tasks/${encodeURIComponent(selected)}/prompt-preview`,
      );
      setPreview(value);
      setError(null);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setLoading(false);
    }
  }, [selected]);

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
            <FileText size={15} className="text-zinc-500" />
            Prompt enviado
          </CardTitle>
          <p className="mt-1 text-xs text-zinc-500">
            Veja exatamente o prompt montado pelo OpenDesk, seção por seção, e o
            contexto que o OpenCode carrega (agente e skills).
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

        {tasks.length === 0 ? (
          <EmptyState
            icon={FileText}
            title="Nenhuma tarefa neste workspace"
            description="Crie uma tarefa para inspecionar o prompt que seria enviado."
            compact
          />
        ) : (
          <>
            <div className="flex flex-wrap items-center gap-2">
              <select
                value={selected}
                onChange={(event) => setSelected(event.target.value)}
                className={cn(inputClass, "min-w-64 flex-1")}
                aria-label="Tarefa para inspecionar"
              >
                {tasks.map((task) => (
                  <option key={task.id} value={task.id}>
                    {task.title} · {task.status}
                  </option>
                ))}
              </select>
              <div className="flex rounded-lg border border-zinc-800 bg-zinc-950/40 p-0.5">
                {(["sections", "prompt", "opencode"] as const).map((value) => (
                  <button
                    key={value}
                    type="button"
                    onClick={() => setView(value)}
                    className={cn(
                      "rounded-md px-2 py-1 text-[12px]",
                      view === value
                        ? "bg-zinc-800 text-zinc-100"
                        : "text-zinc-500 hover:text-zinc-300",
                    )}
                  >
                    {value === "sections"
                      ? "Seções"
                      : value === "prompt"
                        ? "Prompt final"
                        : "OpenCode"}
                  </button>
                ))}
              </div>
            </div>

            {preview ? (
              <>
                <div className="flex flex-wrap items-center gap-1.5 text-[11.5px] text-zinc-500">
                  <Badge variant="default">{preview.phase}</Badge>
                  <span className="font-mono">@{preview.agent}</span>
                  <span>·</span>
                  <span className="font-mono">{preview.model}</span>
                  <span>·</span>
                  <span className="break-all font-mono">{preview.output_directory}</span>
                </div>

                {view === "sections" ? (
                  <div className="space-y-2">
                    {preview.sections.map((section) => (
                      <div
                        key={`${section.id}-${section.label}`}
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

                {view === "prompt" ? (
                  <div className="space-y-2">
                    <div className="flex justify-end">
                      <Button
                        variant="outline"
                        size="sm"
                        onClick={() => void copy(preview.prompt)}
                      >
                        <Clipboard size={12} />
                        Copiar prompt completo
                      </Button>
                    </div>
                    <pre className="max-h-[480px] overflow-auto whitespace-pre-wrap rounded-lg border border-zinc-800 bg-zinc-950/50 p-3 font-mono text-[11.5px] leading-relaxed text-zinc-400">
                      {preview.prompt}
                    </pre>
                  </div>
                ) : null}

                {view === "opencode" ? (
                  <div className="space-y-2">
                    <p className="rounded-lg border border-amber-500/20 bg-amber-500/[0.04] px-3 py-2 text-[11.5px] text-amber-200/80">
                      {preview.opencode.note ??
                        "Este conteúdo é carregado pelo OpenCode, não faz parte do prompt enviado pelo OpenDesk."}
                    </p>
                    <div className="rounded-lg border border-zinc-800/70 bg-zinc-950/30 p-3">
                      <p className="flex items-center gap-1.5 text-[12.5px] font-medium text-zinc-300">
                        <Sparkles size={12} />
                        Agente
                      </p>
                      {preview.opencode.agent ? (
                        <>
                          <p className="mt-1 font-mono text-[11.5px] text-zinc-500">
                            {preview.opencode.agent.scope} · {preview.opencode.agent.path}
                          </p>
                          <pre className="mt-2 max-h-72 overflow-auto whitespace-pre-wrap font-mono text-[11.5px] leading-relaxed text-zinc-400">
                            {preview.opencode.agent.system || "(sem prompt próprio)"}
                          </pre>
                        </>
                      ) : (
                        <p className="mt-1 text-[12px] text-zinc-500">
                          Agente sem arquivo de prompt próprio.
                        </p>
                      )}
                    </div>
                    <div className="rounded-lg border border-zinc-800/70 bg-zinc-950/30 p-3">
                      <p className="text-[12.5px] font-medium text-zinc-300">
                        Skills disponíveis
                      </p>
                      {preview.opencode.skills.length === 0 ? (
                        <p className="mt-1 text-[12px] text-zinc-500">Nenhuma skill.</p>
                      ) : (
                        <div className="mt-1 space-y-1">
                          {preview.opencode.skills.map((skill) => (
                            <p key={`${skill.scope}:${skill.id}`} className="text-[12px] text-zinc-400">
                              <span className="font-mono text-zinc-500">{skill.scope}</span>{" "}
                              {skill.name}
                              {skill.description ? ` — ${skill.description}` : ""}
                            </p>
                          ))}
                        </div>
                      )}
                    </div>
                  </div>
                ) : null}
              </>
            ) : loading ? (
              <p className="flex items-center gap-2 text-[12px] text-zinc-500">
                <Loader2 size={12} className="animate-spin" />
                Montando prompt…
              </p>
            ) : null}
          </>
        )}
      </CardContent>
    </Card>
  );
}

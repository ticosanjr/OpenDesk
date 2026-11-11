import { useEffect, useState } from "react";
import {
  FileText,
  Loader2,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Modal } from "@/components/ui/modal";
import { ModelCombobox } from "@/components/ui/model-combobox";
import { apiGet, apiPost } from "@/lib/api";
import { links } from "@/lib/links";
import type { Artifact, Task } from "@/lib/types";
import { useAppStore } from "@/store/app-store";
import { toast } from "@/store/toast-store";

export function SwarmModal({
  task,
  onClose,
  onCreated,
}: {
  task: Task | null;
  onClose: () => void;
  onCreated: () => Promise<void>;
}) {
  const [models, setModels] = useState([
    "opencode-go/deepseek-v4.1-flash",
    "opencode-go/glm-5.3",
  ]);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [created, setCreated] = useState<
    Array<{ id: string; title: string; model: string }>
  >([]);
  const [artifacts, setArtifacts] = useState<Record<string, Artifact[]>>({});
  const lastRunEvent = useAppStore((state) => state.lastRunEvent);

  useEffect(() => {
    if (task) {
      setError(null);
      setCreated([]);
      setArtifacts({});
    }
  }, [task]);

  useEffect(() => {
    if (created.length === 0) return;
    void Promise.all(
      created.map(async (copy) => {
        const list = await apiGet<Artifact[]>(
          `/api/artifacts?task_id=${encodeURIComponent(copy.id)}`,
        ).catch(() => []);
        return [copy.id, list] as const;
      }),
    ).then((entries) => setArtifacts(Object.fromEntries(entries)));
  }, [created, lastRunEvent?.run_id]);

  async function run() {
    if (!task) return;
    setSaving(true);
    setError(null);
    try {
      const result = await apiPost<{
        tasks: Array<{ id: string; title: string; model: string }>;
      }>(`/api/tasks/${task.id}/swarm`, { models });
      setCreated(result.tasks);
      toast({
        tone: "success",
        title: `${result.tasks.length} cópias enfileiradas`,
        description: "Compare os entregáveis e marque o vencedor.",
      });
      await onCreated();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setSaving(false);
    }
  }

  async function chooseWinner(winnerId: string) {
    for (const copy of created) {
      await apiPost(`/api/tasks/${copy.id}/transition`, {
        status: copy.id === winnerId ? "DONE" : "CANCELLED",
      });
    }
    toast({ tone: "success", title: "Vencedor marcado como Done" });
    setCreated([]);
    onClose();
  }

  return (
    <Modal
      open={!!task}
      onClose={onClose}
      title="Enxame A/B"
      description="Duas cópias isoladas da mesma tarefa, com modelos diferentes. O original não muda."
      className="max-w-2xl"
    >
      <div className="space-y-4">
        {models.map((model, index) => (
          <label key={index}>
            <span className="mb-1 block text-[10px] font-medium uppercase tracking-wide text-zinc-500">
              Modelo {index + 1}
            </span>
            <ModelCombobox
              value={model}
              allowRouter={false}
              onChange={(value) => {
                if (!value) return;
                setModels((current) =>
                  current.map((item, itemIndex) =>
                    itemIndex === index ? value : item,
                  ),
                );
              }}
            />
          </label>
        ))}
        {error ? (
          <p className="rounded-lg border border-red-500/30 bg-red-500/10 px-3 py-2 text-[11px] text-red-200">
            {error}
          </p>
        ) : null}
        {created.length > 0 ? (
          <div className="grid gap-2 sm:grid-cols-2">
            {created.map((copy) => (
              <div
                key={copy.id}
                className="flex min-w-0 flex-col gap-2 rounded-lg border border-zinc-800 bg-zinc-950/40 px-3 py-2"
              >
                <div className="min-w-0">
                  <p className="truncate text-[12px] text-zinc-200">{copy.title}</p>
                  <p className="font-mono text-[10px] text-zinc-500">{copy.model}</p>
                </div>
                <div className="min-h-10 flex-1 space-y-1">
                  {(artifacts[copy.id] ?? []).length === 0 ? (
                    <p className="text-[10px] text-zinc-500">
                      aguardando entregáveis…
                    </p>
                  ) : (
                    artifacts[copy.id].slice(0, 6).map((artifact) => (
                      <button
                        key={artifact.id}
                        type="button"
                        onClick={() => {
                          window.location.hash = `#/artifacts?id=${encodeURIComponent(artifact.id)}`;
                        }}
                        className="flex w-full items-center gap-1.5 truncate rounded border border-zinc-800/70 px-1.5 py-1 text-left text-[10px] text-zinc-400 hover:text-zinc-100"
                        title={artifact.file_path}
                      >
                        <FileText size={10} className="shrink-0" />
                        <span className="truncate">{artifact.file_path}</span>
                      </button>
                    ))
                  )}
                </div>
                <div className="flex flex-wrap gap-1">
                  <Button
                    size="sm"
                    variant="outline"
                    className="h-7"
                    onClick={() => {
                      window.location.hash = `#${links.task(copy.id)}`;
                    }}
                  >
                    Abrir
                  </Button>
                  <Button
                    size="sm"
                    className="h-7"
                    onClick={() => void chooseWinner(copy.id)}
                  >
                    Vencedor
                  </Button>
                </div>
              </div>
            ))}
          </div>
        ) : null}
        <div className="flex justify-end gap-2">
          <Button variant="outline" size="sm" onClick={onClose}>
            Fechar
          </Button>
          <Button size="sm" onClick={() => void run()} disabled={saving || !task}>
            {saving ? <Loader2 size={13} className="animate-spin" /> : null}
            Rodar em paralelo
          </Button>
        </div>
      </div>
    </Modal>
  );
}

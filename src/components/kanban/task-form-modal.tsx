import { useEffect, useState } from "react";
import { Modal } from "@/components/ui/modal";
import { PromptAttachments } from "@/components/ui/prompt-attachments";
import { SlashTextarea } from "@/components/ui/slash-textarea";
import { apiDelete, apiGet, apiPatch, apiPost, apiPut } from "@/lib/api";
import { links } from "@/lib/links";
import { isTaskRunning } from "@/lib/task-state";
import type { Task, TaskWatcher } from "@/lib/types";
import { toast } from "@/store/toast-store";
import {
  fieldInputClass as inputClass,
  fieldLabelClass as labelClass,
} from "@/lib/ui-classes";
import { TaskFormActions } from "./form/task-form-actions";
import { TaskFormAdvanced } from "./form/task-form-advanced";
import { buildTaskPayload } from "./form/task-form-payload";

export function CreateTaskModal({
  open,
  workspaceId,
  onClose,
  onCreated,
}: {
  open: boolean;
  workspaceId: string;
  onClose: () => void;
  onCreated: () => Promise<void>;
}) {
  return (
    <TaskFormModal
      open={open}
      mode="create"
      task={null}
      workspaceId={workspaceId}
      onClose={onClose}
      onSaved={async () => {
        await onCreated();
      }}
    />
  );
}

export function EditTaskModal({
  task,
  onClose,
  onSaved,
}: {
  task: Task | null;
  onClose: () => void;
  onSaved: (task: Task, rerun: boolean) => Promise<void>;
}) {
  return (
    <TaskFormModal
      open={!!task}
      mode="edit"
      task={task}
      workspaceId={task?.workspace_id ?? null}
      onClose={onClose}
      onSaved={async (saved, rerun) => {
        await onSaved(saved, rerun);
      }}
    />
  );
}

export function ForkTaskModal({
  open,
  source,
  runId,
  onClose,
  onSaved,
}: {
  open: boolean;
  source: Task | null;
  runId?: string | null;
  onClose: () => void;
  onSaved: (task: Task) => Promise<void>;
}) {
  return (
    <TaskFormModal
      open={open && !!source}
      mode="fork"
      task={null}
      forkSource={source}
      forkRunId={runId}
      workspaceId={source?.workspace_id ?? null}
      onClose={onClose}
      onSaved={async (saved) => {
        await onSaved(saved);
      }}
    />
  );
}

export function TaskFormModal({
  open,
  mode,
  task,
  forkSource = null,
  forkRunId = null,
  workspaceId,
  onClose,
  onSaved,
}: {
  open: boolean;
  mode: "create" | "edit" | "fork";
  task: Task | null;
  forkSource?: Task | null;
  forkRunId?: string | null;
  workspaceId: string | null;
  onClose: () => void;
  onSaved: (task: Task, rerun: boolean) => Promise<void>;
}) {
  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");
  const [executionMode, setExecutionMode] = useState("AUTO_DSH");
  const [priority, setPriority] = useState(2);
  const [tags, setTags] = useState("");
  const [needsBrowser, setNeedsBrowser] = useState(false);
  const [browseAllowlist, setBrowseAllowlist] = useState("");
  const [agent, setAgent] = useState("");
  const [model, setModel] = useState<string | null>(null);
  const [harness, setHarness] = useState("");
  const [attachments, setAttachments] = useState<string[]>([]);
  const [effort, setEffort] = useState("medium");
  const [careLevel, setCareLevel] = useState<"quick" | "standard" | "careful">(
    "standard",
  );
  const [email, setEmail] = useState("");
  const [watchChannels, setWatchChannels] = useState<string[]>([]);
  const [exportToGdocs, setExportToGdocs] = useState(false);
  const [saving, setSaving] = useState<"save" | "rerun" | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    setError(null);
    const source = task ?? forkSource;
    if (source) {
      setTitle(
        mode === "fork" ? `${source.title} (fork)` : source.title,
      );
      setDescription(source.description);
      setExecutionMode(source.execution_mode);
      setPriority(source.priority);
      setAgent(source.assigned_agent ?? "");
      setModel(source.preferred_model ?? null);
      setHarness(source.preferred_harness ?? "");
      setAttachments([]);
      setEffort(source.effort_level);
      setCareLevel(source.care_level ?? "standard");
      setEmail(source.notify_email ?? "");
      setExportToGdocs(source.export_to_gdocs);
      try {
        const parsed = JSON.parse(source.tags_json) as string[];
        setNeedsBrowser(parsed.includes("needs-browser"));
        const allowlistTag = parsed.find((tag) =>
          tag.startsWith("browse-allowlist:"),
        );
        setBrowseAllowlist(
          allowlistTag?.slice("browse-allowlist:".length).split(",").join(", ") ?? "",
        );
        setTags(
          parsed
            .filter(
              (tag) =>
                tag !== "needs-browser" &&
                !tag.startsWith("browse-allowlist:"),
            )
            .join(", "),
        );
      } catch {
        setNeedsBrowser(false);
        setBrowseAllowlist("");
        setTags("");
      }
    } else {
      setTitle("");
      setDescription("");
      setExecutionMode("AUTO_DSH");
      setPriority(2);
      setAgent("");
      setModel(null);
      setAttachments([]);
      setEffort("medium");
      setCareLevel("standard");
      setEmail("");
      setExportToGdocs(false);
      setTags("");
      setNeedsBrowser(false);
      setBrowseAllowlist("");
    }
  }, [open, task, forkSource, mode]);

  useEffect(() => {
    if (!open) return;
    if (!task) {
      setWatchChannels([]);
      return;
    }
    let cancelled = false;
    void apiGet<TaskWatcher[]>(`/api/tasks/${task.id}/watchers`)
      .then((watchers) => {
        if (!cancelled) {
          setWatchChannels(watchers.map((watcher) => watcher.channel));
        }
      })
      .catch(() => {
        if (!cancelled) setWatchChannels([]);
      });
    return () => {
      cancelled = true;
    };
  }, [open, task]);

  if (!open) return null;
  const running = task ? isTaskRunning(task) : false;

  async function syncWatchers(taskId: string) {
    try {
      await apiPut(`/api/tasks/${taskId}/watchers`, {
        channels: watchChannels,
      });
    } catch (cause) {
      toast({
        tone: "error",
        title: "Tarefa salva, mas os avisos não foram atualizados",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    }
  }

  async function submit(rerun: boolean) {
    if (!workspaceId) return;
    if (!description.trim()) {
      setError("A descrição é obrigatória.");
      return;
    }
    setSaving(rerun ? "rerun" : "save");
    setError(null);
    try {
      const payload = buildTaskPayload({
        title,
        description,
        attachments,
        tags,
        needsBrowser,
        browseAllowlist,
        executionMode,
        priority,
        agent,
        model,
        harness,
        effort,
        careLevel,
        email,
        exportToGdocs,
      });
      if (mode === "fork") {
        if (!forkSource && !forkRunId) {
          throw new Error("fork sem tarefa ou execução de origem");
        }
        const forked = await apiPost<Task>(
          forkRunId
            ? `/api/runs/${forkRunId}/fork`
            : `/api/tasks/${forkSource!.id}/fork`,
          payload,
        );
        toast({
          tone: "success",
          title: "Fork criado",
          description: forked.title,
          href: links.task(forked.id),
        });
        await syncWatchers(forked.id);
        await onSaved(forked, false);
      } else if (mode === "create") {
        const created = await apiPost<Task>(
          `/api/tasks?workspace_id=${encodeURIComponent(workspaceId)}`,
          payload,
        );
        toast({
          tone: created.deduplicated ? "info" : "success",
          title: created.deduplicated
            ? "Tarefa idêntica já existe"
            : "Tarefa criada",
          description: created.deduplicated
            ? "Reutilizando a tarefa ativa ou recente com os mesmos parâmetros."
            : `“${created.title}” entrou no Kanban.`,
          href: links.task(created.id),
        });
        if (!created.deduplicated) {
          await syncWatchers(created.id);
        }
        await onSaved(created, false);
      } else if (task) {
        const saved = await apiPatch<Task>(`/api/tasks/${task.id}`, {
          ...payload,
          agent_override: Boolean(agent.trim()),
          model_override: Boolean(model),
          clear_agent: !agent.trim(),
          clear_model: !model,
          clear_harness: !harness.trim(),
        });
        toast({
          tone: "success",
          title: rerun ? "Tarefa salva e reenfileirada" : "Tarefa atualizada",
          description: saved.title,
          href: links.task(saved.id),
        });
        await syncWatchers(saved.id);
        await onSaved(saved, rerun);
      }
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setSaving(null);
    }
  }

  function cancel() {
    const paths = attachments;
    setAttachments([]);
    if (workspaceId) {
      for (const path of paths) {
        void apiDelete(
          `/api/attachments?workspace_id=${encodeURIComponent(workspaceId)}&path=${encodeURIComponent(path)}`,
        ).catch(() => {
          // Best effort; an orphan file is harmless.
        });
      }
    }
    onClose();
  }

  return (
    <Modal
      open={open}
      onClose={cancel}
      title={
        mode === "create"
          ? "Nova tarefa"
          : mode === "fork"
            ? "Criar fork"
            : "Editar tarefa"
      }
      description={
        mode === "create"
          ? "Comece pelo título e prompt. Abra “Mais opções” para modo, agente, modelo, effort e notificações. O roteador decide o resto."
          : mode === "fork"
            ? "Revise a cópia antes de criar. O fork nasce como uma tarefa independente e só entra na fila depois que você confirmar."
            : "Corrija agente, modelo, prompt e prioridade sem recriar o card."
      }
      size="xl"
    >
      <div className="space-y-3">
      <div className="grid gap-3 md:grid-cols-2">
        <label className="md:col-span-2">
          <span className={labelClass}>Título (opcional)</span>
          <input
            autoFocus
            value={title}
            onChange={(event) => setTitle(event.target.value)}
            placeholder="Se vazio, usamos o início da descrição"
            className={inputClass}
          />
          <span className="mt-1 block text-[11.5px] text-zinc-500">
            Sem título, o OpenDesk usa a primeira linha da descrição, limitada a
            80 caracteres.
          </span>
        </label>
        <div className="md:col-span-2">
          <span className={labelClass}>Descrição / prompt</span>
          <SlashTextarea
            value={description}
            onChange={setDescription}
            workspaceId={workspaceId}
            rows={5}
            placeholder="Descreva o objetivo, as entradas e os critérios de aceite… Digite / para comandos e skills."
          />
          <PromptAttachments
            workspaceId={workspaceId}
            value={attachments}
            onChange={setAttachments}
          />
        </div>
      </div>
        <TaskFormAdvanced
          workspaceId={workspaceId}
          executionMode={executionMode}
          setExecutionMode={setExecutionMode}
          priority={priority}
          setPriority={setPriority}
          agent={agent}
          setAgent={setAgent}
          model={model}
          setModel={setModel}
          harness={harness}
          setHarness={setHarness}
          effort={effort}
          setEffort={setEffort}
          careLevel={careLevel}
          setCareLevel={setCareLevel}
          tags={tags}
          setTags={setTags}
          needsBrowser={needsBrowser}
          setNeedsBrowser={setNeedsBrowser}
          browseAllowlist={browseAllowlist}
          setBrowseAllowlist={setBrowseAllowlist}
          email={email}
          setEmail={setEmail}
          watchChannels={watchChannels}
          setWatchChannels={setWatchChannels}
          exportToGdocs={exportToGdocs}
          setExportToGdocs={setExportToGdocs}
        />
      </div>
      {running ? (
        <p className="mt-3 rounded-lg border border-amber-500/20 bg-amber-500/10 px-3 py-2 text-[12px] text-amber-200">
          Cancele a execução atual antes de salvar alterações.
        </p>
      ) : null}
      {error ? (
        <p className="mt-3 rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[13px] text-red-300">
          {error}
        </p>
      ) : null}
      <TaskFormActions
        mode={mode}
        saving={saving}
        running={running}
        onSubmit={(rerun) => void submit(rerun)}
        onCancel={cancel}
      />
    </Modal>
  );
}


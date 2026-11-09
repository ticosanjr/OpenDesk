import { useEffect, useState } from "react";
import {
  ArrowRightLeft,
  Loader2,
  Plus,
} from "lucide-react";
import { AgentCombobox } from "@/components/ui/agent-combobox";
import { Button } from "@/components/ui/button";
import { CollapsibleSection } from "@/components/ui/collapsible-section";
import { Modal } from "@/components/ui/modal";
import { ModelCombobox } from "@/components/ui/model-combobox";
import { PromptAttachments } from "@/components/ui/prompt-attachments";
import { SlashTextarea } from "@/components/ui/slash-textarea";
import { apiDelete, apiPatch, apiPost } from "@/lib/api";
import { links } from "@/lib/links";
import { PRIORITY_LABEL } from "@/lib/task-ui";
import { isTaskRunning } from "@/lib/task-state";
import type { Task } from "@/lib/types";
import { toast } from "@/store/toast-store";
import {
  fieldInputClass as inputClass,
  fieldLabelClass as labelClass,
} from "@/lib/ui-classes";

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

export function TaskFormModal({
  open,
  mode,
  task,
  workspaceId,
  onClose,
  onSaved,
}: {
  open: boolean;
  mode: "create" | "edit";
  task: Task | null;
  workspaceId: string | null;
  onClose: () => void;
  onSaved: (task: Task, rerun: boolean) => Promise<void>;
}) {
  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");
  const [executionMode, setExecutionMode] = useState("AUTO_DSH");
  const [priority, setPriority] = useState(2);
  const [tags, setTags] = useState("");
  const [agent, setAgent] = useState("");
  const [model, setModel] = useState<string | null>(null);
  const [attachments, setAttachments] = useState<string[]>([]);
  const [effort, setEffort] = useState("medium");
  const [email, setEmail] = useState("");
  const [exportToGdocs, setExportToGdocs] = useState(false);
  const [saving, setSaving] = useState<"save" | "rerun" | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    setError(null);
    if (task) {
      setTitle(task.title);
      setDescription(task.description);
      setExecutionMode(task.execution_mode);
      setPriority(task.priority);
      setAgent(task.assigned_agent ?? "");
      setModel(task.preferred_model ?? null);
      setAttachments([]);
      setEffort(task.effort_level);
      setEmail(task.notify_email ?? "");
      setExportToGdocs(task.export_to_gdocs);
      try {
        setTags((JSON.parse(task.tags_json) as string[]).join(", "));
      } catch {
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
      setEmail("");
      setExportToGdocs(false);
      setTags("");
    }
  }, [open, task]);

  if (!open) return null;
  const running = task ? isTaskRunning(task) : false;

  async function submit(rerun: boolean) {
    if (!workspaceId) return;
    if (!title.trim() || !description.trim()) {
      setError("Título e descrição são obrigatórios.");
      return;
    }
    setSaving(rerun ? "rerun" : "save");
    setError(null);
    try {
      const attachmentBlock =
        attachments.length > 0
          ? `\n\nArquivos anexados:\n${attachments
              .map((path) => `- ${path}`)
              .join("\n")}`
          : "";
      const payload = {
        title: title.trim(),
        description: `${description.trim()}${attachmentBlock}`,
        execution_mode: executionMode,
        priority,
        assigned_agent: agent.trim() || null,
        preferred_model: model,
        effort_level: effort,
        notify_email: email.trim() || null,
        export_to_gdocs: exportToGdocs,
        tags: tags
          .split(",")
          .map((tag) => tag.trim())
          .filter(Boolean),
      };
      if (mode === "create") {
        const created = await apiPost<Task>(
          `/api/tasks?workspace_id=${encodeURIComponent(workspaceId)}`,
          payload,
        );
        toast({
          tone: "success",
          title: "Tarefa criada",
          description: `“${created.title}” entrou no Kanban.`,
          href: links.task(created.id),
        });
        await onSaved(created, false);
      } else if (task) {
        const saved = await apiPatch<Task>(`/api/tasks/${task.id}`, {
          ...payload,
          agent_override: Boolean(agent.trim()),
          model_override: Boolean(model),
          clear_agent: !agent.trim(),
          clear_model: !model,
        });
        toast({
          tone: "success",
          title: rerun ? "Tarefa salva e reenfileirada" : "Tarefa atualizada",
          description: saved.title,
          href: links.task(saved.id),
        });
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

  const advancedSummary = [
    model ? `modelo: ${model}` : "roteador automático",
    `prioridade ${priority}`,
    agent ? `@${agent}` : null,
    tags.trim() ? "tags" : null,
    email.trim() ? "e-mail" : null,
    exportToGdocs ? "Google Docs" : null,
  ]
    .filter(Boolean)
    .join(" · ");

  return (
    <Modal
      open={open}
      onClose={cancel}
      title={mode === "create" ? "Nova tarefa" : "Editar tarefa"}
      description={
        mode === "create"
          ? "Comece pelo título e prompt. Abra “Mais opções” para modo, agente, modelo, effort e notificações. O roteador decide o resto."
          : "Corrija agente, modelo, prompt e prioridade sem recriar o card."
      }
      className="max-w-2xl"
    >
      <div className="space-y-3">
      <div className="grid gap-3 md:grid-cols-2">
        <label className="md:col-span-2">
          <span className={labelClass}>Título</span>
          <input
            autoFocus
            value={title}
            onChange={(event) => setTitle(event.target.value)}
            placeholder="Relatório executivo de vendas"
            className={inputClass}
          />
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
        <CollapsibleSection title="Mais opções" summary={advancedSummary}>
          <div className="grid gap-3 md:grid-cols-2">
        <label>
          <span className={labelClass}>Modo</span>
          <select
            value={executionMode}
            onChange={(event) => setExecutionMode(event.target.value)}
            className={inputClass}
          >
            <option value="AUTO_DSH">AUTO_DSH — roteador decide</option>
            <option value="PLAN_ONLY">PLAN_ONLY — só plano</option>
            <option value="PLAN_THEN_BUILD">PLAN_THEN_BUILD — aprovar plano</option>
            <option value="BUILD_DIRECT">BUILD_DIRECT — executar direto</option>
          </select>
        </label>
        <label>
          <span className={labelClass}>Prioridade</span>
          <select
            value={priority}
            onChange={(event) => setPriority(Number(event.target.value))}
            className={inputClass}
          >
            {[4, 3, 2, 1].map((value) => (
              <option key={value} value={value}>
                {value} — {PRIORITY_LABEL[value]}
              </option>
            ))}
          </select>
        </label>
        <div>
          <span className={labelClass}>Agente (opcional)</span>
          <AgentCombobox
            value={agent}
            onChange={(value) => setAgent(value)}
            workspaceId={workspaceId}
            placeholder="automático (router)"
          />
        </div>
        <div>
          <span className={labelClass}>Modelo (opcional)</span>
          <ModelCombobox
            value={model}
            onChange={setModel}
            placeholder="digite para filtrar (ex.: deepseek, glm, opencode-go/…)"
          />
        </div>
        <label>
          <span className={labelClass}>Effort</span>
          <select
            value={effort}
            onChange={(event) => setEffort(event.target.value)}
            className={inputClass}
          >
            <option value="low">low</option>
            <option value="medium">medium</option>
            <option value="high">high</option>
          </select>
        </label>
        <label>
          <span className={labelClass}>Tags (vírgula)</span>
          <input
            value={tags}
            onChange={(event) => setTags(event.target.value)}
            placeholder="roteiro, code, auditoria"
            className={inputClass}
          />
        </label>
        <label>
          <span className={labelClass}>E-mail de notificação</span>
          <input
            value={email}
            onChange={(event) => setEmail(event.target.value)}
            placeholder="ops@empresa.com"
            className={inputClass}
          />
        </label>
        <label className="flex items-end gap-2 pb-2 text-[11px] text-zinc-400">
          <input
            type="checkbox"
            checked={exportToGdocs}
            onChange={(event) => setExportToGdocs(event.target.checked)}
            className="accent-blue-500"
          />
          exportar para Google Docs
        </label>
          </div>
        </CollapsibleSection>
      </div>
      {running ? (
        <p className="mt-3 rounded-lg border border-amber-500/20 bg-amber-500/10 px-3 py-2 text-[10px] text-amber-200">
          Cancele a execução atual antes de salvar alterações.
        </p>
      ) : null}
      {error ? (
        <p className="mt-3 rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[11px] text-red-300">
          {error}
        </p>
      ) : null}
      <div className="mt-4 flex flex-wrap justify-end gap-2 border-t border-zinc-800/70 pt-4">
        <Button variant="ghost" size="sm" onClick={cancel}>
          Cancelar
        </Button>
        {mode === "create" ? (
          <Button size="sm" onClick={() => void submit(false)} disabled={saving !== null}>
            {saving ? <Loader2 size={14} className="animate-spin" /> : <Plus size={14} />}
            Criar tarefa
          </Button>
        ) : (
          <>
            <Button
              variant="outline"
              size="sm"
              onClick={() => void submit(false)}
              disabled={saving !== null || running}
            >
              {saving === "save" ? <Loader2 size={14} className="animate-spin" /> : null}
              Salvar
            </Button>
            <Button
              size="sm"
              onClick={() => void submit(true)}
              disabled={saving !== null || running}
            >
              {saving === "rerun" ? (
                <Loader2 size={14} className="animate-spin" />
              ) : (
                <ArrowRightLeft size={14} />
              )}
              Salvar e reexecutar
            </Button>
          </>
        )}
      </div>
    </Modal>
  );
}


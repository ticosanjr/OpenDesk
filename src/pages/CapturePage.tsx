import { useEffect, useRef, useState } from "react";
import { ClipboardPaste, Loader2, Send, X } from "lucide-react";
import { useNavigate } from "react-router-dom";
import { Button } from "@/components/ui/button";
import { apiPost } from "@/lib/api";
import { links } from "@/lib/links";
import { useAppStore } from "@/store/app-store";
import { toast } from "@/store/toast-store";
import { hideWindowIfTauri } from "@/lib/runtime-env";

export function CapturePage() {
  const workspaces = useAppStore((state) => state.workspaces);
  const activeWorkspaceId = useAppStore((state) => state.activeWorkspaceId);
  const boot = useAppStore((state) => state.boot);
  const [workspaceId, setWorkspaceId] = useState(activeWorkspaceId ?? "");
  const [content, setContent] = useState("");
  const [agent, setAgent] = useState("");
  const [image, setImage] = useState<{ data: string; name: string } | null>(null);
  const [sending, setSending] = useState(false);
  const textarea = useRef<HTMLTextAreaElement>(null);
  const navigate = useNavigate();

  useEffect(() => {
    void boot();
  }, [boot]);

  useEffect(() => {
    if (!workspaceId && activeWorkspaceId) setWorkspaceId(activeWorkspaceId);
  }, [activeWorkspaceId, workspaceId]);

  useEffect(() => {
    textarea.current?.focus();
  }, []);

  async function close() {
    const hidden = await hideWindowIfTauri();
    if (!hidden) navigate("/");
  }

  function attachImage(file: File) {
    if (!file.type.startsWith("image/")) return;
    const reader = new FileReader();
    reader.onload = () => {
      setImage({
        data: String(reader.result ?? ""),
        name: file.name || "captura.png",
      });
    };
    reader.readAsDataURL(file);
  }

  function onPaste(event: React.ClipboardEvent<HTMLTextAreaElement>) {
    const file = Array.from(event.clipboardData.files).find((item) =>
      item.type.startsWith("image/"),
    );
    if (file) {
      event.preventDefault();
      attachImage(file);
    }
  }

  async function save() {
    if (!content.trim() && !image) return;
    setSending(true);
    try {
      const task = await apiPost<{ id: string; title: string }>("/api/capture", {
        workspace_id: workspaceId || undefined,
        content,
        image_base64: image?.data,
        filename: image?.name,
        assigned_agent: agent.trim() || undefined,
      });
      toast({
        tone: "success",
        title: "Capturado",
        description: task.title,
        href: links.task(task.id),
      });
      await close();
    } catch (cause) {
      toast({
        tone: "error",
        title: "Captura falhou",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setSending(false);
    }
  }

  return (
    <div
      className="flex h-screen flex-col gap-2 bg-zinc-950 p-3 text-zinc-200"
      onDragOver={(event) => event.preventDefault()}
      onDrop={(event) => {
        event.preventDefault();
        const file = Array.from(event.dataTransfer.files).find((item) =>
          item.type.startsWith("image/"),
        );
        if (file) attachImage(file);
      }}
    >
      <div className="flex items-center gap-2">
        <ClipboardPaste size={15} className="text-blue-400" />
        <p className="text-[13.5px] font-semibold">Quick Capture</p>
        <select
          value={workspaceId}
          onChange={(event) => setWorkspaceId(event.target.value)}
          className="ml-auto h-7 max-w-56 rounded-md border border-zinc-800 bg-zinc-900 px-2 text-[13px]"
        >
          <option value="">workspace ativo</option>
          {workspaces.map((workspace) => (
            <option key={workspace.id} value={workspace.id}>
              {workspace.name}
            </option>
          ))}
        </select>
        <Button variant="ghost" size="icon" className="h-7 w-7" aria-label="Fechar captura" onClick={() => void close()}>
          <X size={14} />
        </Button>
      </div>

      <textarea
        ref={textarea}
        value={content}
        onChange={(event) => setContent(event.target.value)}
        onPaste={onPaste}
        onKeyDown={(event) => {
          if ((event.metaKey || event.ctrlKey) && event.key === "Enter") {
            event.preventDefault();
            void save();
          }
          if (event.key === "Escape") void close();
        }}
        placeholder="Cole um texto, link ou print (Ctrl+V). Ctrl+Enter para capturar."
        className="min-h-0 flex-1 resize-none rounded-lg border border-zinc-800 bg-zinc-900/40 p-3 text-[14px] outline-none placeholder:text-zinc-500 focus:border-blue-500/40"
      />

      {image ? (
        <div className="flex items-center gap-2 rounded-lg border border-zinc-800 bg-zinc-900/40 px-2 py-1.5">
          <img src={image.data} alt="" className="h-10 w-10 rounded object-cover" />
          <span className="min-w-0 flex-1 truncate text-[12px] text-zinc-400">
            {image.name}
          </span>
          <button
            type="button"
            className="text-zinc-500 hover:text-zinc-200"
            onClick={() => setImage(null)}
          >
            <X size={13} />
          </button>
        </div>
      ) : null}

      <div className="flex items-center justify-between gap-2">
        <div className="flex items-center gap-2">
          <input
            value={agent}
            onChange={(event) => setAgent(event.target.value)}
            placeholder="@agente (opcional)"
            className="h-7 w-36 rounded-md border border-zinc-800 bg-zinc-900 px-2 font-mono text-[12px] outline-none placeholder:text-zinc-500 focus:border-blue-500/40"
          />
          <p className="text-[12px] text-zinc-500">
            Backlog · Ctrl+Enter envia · Esc fecha
          </p>
        </div>
        <Button size="sm" onClick={() => void save()} disabled={sending || (!content.trim() && !image)}>
          {sending ? <Loader2 size={13} className="animate-spin" /> : <Send size={13} />}
          Capturar
        </Button>
      </div>
    </div>
  );
}

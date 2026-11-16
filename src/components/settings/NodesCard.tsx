import { useCallback, useEffect, useState } from "react";
import { Loader2, Network, Plus, Trash2 } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { EmptyState } from "@/components/ui/empty-state";
import { apiDelete, apiGet, apiPost } from "@/lib/api";
import { relativeTime } from "@/lib/format";
import { toast } from "@/store/toast-store";

interface NodeView {
  id: string;
  name: string;
  endpoint: string;
  capabilities: string[];
  status: string;
  online: boolean;
  last_seen_at?: string | null;
  created_at: string;
}

const inputClass =
  "h-8 w-full rounded border border-zinc-700 bg-zinc-950 px-2 text-[12.5px] text-zinc-200";

export function NodesCard() {
  const [nodes, setNodes] = useState<NodeView[]>([]);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState<string | null>(null);
  const [revealed, setRevealed] = useState<{ name: string; token: string } | null>(
    null,
  );
  const [form, setForm] = useState({ name: "", endpoint: "", capabilities: "opencode" });

  const load = useCallback(async () => {
    setLoading(true);
    try {
      setNodes(await apiGet<NodeView[]>("/api/nodes"));
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível carregar os nós",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  async function create() {
    if (!form.name.trim()) return;
    setBusy("create");
    try {
      const result = await apiPost<{ token: string; node: NodeView }>("/api/nodes", {
        name: form.name.trim(),
        endpoint: form.endpoint.trim(),
        capabilities: form.capabilities
          .split(",")
          .map((value) => value.trim())
          .filter(Boolean),
      });
      setRevealed({ name: result.node.name, token: result.token });
      setForm({ name: "", endpoint: "", capabilities: "opencode" });
      await load();
      toast({
        tone: "success",
        title: "Nó registrado",
        description: "Copie o token agora; ele não é exibido novamente.",
      });
    } catch (cause) {
      toast({
        tone: "error",
        title: "Falha ao registrar",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setBusy(null);
    }
  }

  async function rotate(id: string, name: string) {
    setBusy(`rotate:${id}`);
    try {
      const result = await apiPost<{ token: string }>(`/api/nodes/${id}/rotate`);
      setRevealed({ name, token: result.token });
      await load();
      toast({
        tone: "success",
        title: "Token rotacionado",
        description: "O token anterior deixou de autenticar. Copie o novo agora.",
      });
    } catch (cause) {
      toast({
        tone: "error",
        title: "Falha ao rotacionar",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setBusy(null);
    }
  }

  async function remove(id: string) {
    setBusy(id);
    try {
      await apiDelete(`/api/nodes/${id}`);
      await load();
    } finally {
      setBusy(null);
    }
  }

  return (
    <Card id="settings-nodes" className="scroll-mt-14">
      <CardHeader>
        <div>
          <CardTitle className="flex items-center gap-2">
            <Network size={15} className="text-zinc-500" />
            Nós remotos
          </CardTitle>
          <p className="mt-1 text-xs text-zinc-500">
            Workers puxam jobs pela API com token próprio (
            <code className="font-mono">/api/nodes/next-job</code>), enviam
            heartbeat e concluem a execução. O workspace precisa estar acessível
            ao nó (montagem compartilhada).
          </p>
        </div>
      </CardHeader>
      <CardContent className="space-y-3">
        <div className="grid gap-2 rounded-lg border border-zinc-800/70 bg-zinc-950/40 p-3 sm:grid-cols-4">
          <input
            value={form.name}
            onChange={(event) => setForm({ ...form, name: event.target.value })}
            placeholder="nome"
            className={inputClass}
          />
          <input
            value={form.endpoint}
            onChange={(event) => setForm({ ...form, endpoint: event.target.value })}
            placeholder="http://host:4096"
            className={inputClass}
          />
          <input
            value={form.capabilities}
            onChange={(event) =>
              setForm({ ...form, capabilities: event.target.value })
            }
            placeholder="harnesses (vírgula)"
            className={inputClass}
          />
          <Button
            size="sm"
            className="h-8"
            disabled={busy === "create"}
            onClick={() => void create()}
          >
            <Plus size={12} />
            Registrar
          </Button>
        </div>

        {revealed ? (
          <div className="rounded-lg border border-amber-500/20 bg-amber-500/5 px-3 py-2 text-[12px] text-amber-200">
            Token de <strong>{revealed.name}</strong> (copie agora):{" "}
            <code className="break-all font-mono">{revealed.token}</code>
          </div>
        ) : null}

        {loading ? (
          <div className="flex items-center gap-2 text-[12.5px] text-zinc-500">
            <Loader2 size={13} className="animate-spin" />
            carregando nós…
          </div>
        ) : nodes.length === 0 ? (
          <EmptyState
            title="Nenhum nó"
            description="A fila roda localmente até você registrar um worker remoto."
          />
        ) : (
          <div className="space-y-1.5">
            {nodes.map((node) => (
              <div
                key={node.id}
                className="flex flex-wrap items-center justify-between gap-2 rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2"
              >
                <div className="min-w-0">
                  <div className="flex flex-wrap items-center gap-1.5">
                    <span className="text-[13px] font-medium text-zinc-200">
                      {node.name}
                    </span>
                    <Badge variant={node.online ? "green" : "default"}>
                      {node.online ? "online" : "offline"}
                    </Badge>
                    <Badge variant="default">{node.status}</Badge>
                  </div>
                  <p className="truncate font-mono text-[11px] text-zinc-600">
                    {node.endpoint || "endpoint não informado"}
                  </p>
                  <p className="text-[11px] text-zinc-600">
                    {node.capabilities.join(", ") || "sem capabilities"} ·{" "}
                    {node.last_seen_at
                      ? `visto ${relativeTime(node.last_seen_at)}`
                      : "nunca conectou"}
                  </p>
                </div>
                <div className="flex items-center gap-1.5">
                  <Button
                    variant="outline"
                    size="sm"
                    className="h-7"
                    disabled={busy === `rotate:${node.id}`}
                    onClick={() => void rotate(node.id, node.name)}
                  >
                    Rotacionar token
                  </Button>
                  <Button
                    variant="ghost"
                    size="sm"
                    className="h-7 px-2"
                    disabled={busy === node.id}
                    onClick={() => void remove(node.id)}
                  >
                    <Trash2 size={11} />
                  </Button>
                </div>
              </div>
            ))}
          </div>
        )}
      </CardContent>
    </Card>
  );
}

import { useCallback, useEffect, useState } from "react";
import {
  Download,
  Loader2,
  Pencil,
  Plus,
  Terminal,
  Trash2,
} from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { EmptyState } from "@/components/ui/empty-state";
import { JsonImportButton } from "@/components/ui/json-import-button";
import { Switch } from "@/components/ui/switch";
import { apiDelete, apiGet, apiPut } from "@/lib/api";
import { exportJsonBundle, type BundleEnvelope } from "@/lib/import-export";
import type { ScriptTool, ScriptToolList } from "@/lib/types";
import { confirmAction } from "@/store/confirm-store";
import { toast } from "@/store/toast-store";
import { ScriptToolModal } from "./ScriptToolModal";

export function ScriptToolsCard({ workspaceId }: { workspaceId: string }) {
  const [tools, setTools] = useState<ScriptTool[]>([]);
  const [runtimes, setRuntimes] = useState<string[]>([]);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [modalOpen, setModalOpen] = useState(false);
  const [selected, setSelected] = useState<ScriptTool | null>(null);

  const query = `?workspace_id=${encodeURIComponent(workspaceId)}`;

  const load = useCallback(async () => {
    if (!workspaceId) return;
    setLoading(true);
    try {
      const value = await apiGet<ScriptToolList>(`/api/script-tools${query}`);
      setTools(value.tools);
      setRuntimes(value.runtimes);
      setError(null);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setLoading(false);
    }
  }, [query, workspaceId]);

  useEffect(() => {
    void load();
  }, [load]);

  async function toggle(tool: ScriptTool) {
    setBusy(tool.id);
    try {
      await apiPut(`/api/script-tools/${tool.id}`, { enabled: !tool.enabled });
      await load();
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível alterar a ferramenta",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setBusy(null);
    }
  }

  async function remove(tool: ScriptTool) {
    if (
      !(await confirmAction({
        title: `Excluir ${tool.name}?`,
        description:
          "A ferramenta e sua configuração são removidas. Scripts já executados não são afetados.",
        confirmLabel: "Excluir ferramenta",
        tone: "destructive",
      }))
    ) {
      return;
    }
    setBusy(tool.id);
    try {
      await apiDelete(`/api/script-tools/${tool.id}`);
      await load();
      toast({ tone: "success", title: "Ferramenta excluída" });
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível excluir",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setBusy(null);
    }
  }

  async function exportTools() {
    if (!workspaceId) return;
    setBusy("export");
    try {
      const bundle = await apiGet<BundleEnvelope>(`/api/script-tools/export${query}`);
      exportJsonBundle(
        "script_tools",
        bundle.items,
        `opendesk-script-tools-${new Date().toISOString().slice(0, 10)}.json`,
        workspaceId,
      );
      toast({ tone: "success", title: "Ferramentas exportadas" });
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível exportar",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setBusy(null);
    }
  }

  return (
    <>
      <Card id="integrations-script-tools">
        <CardHeader>
          <div>
            <CardTitle className="flex items-center gap-2">
              <Terminal size={15} className="text-zinc-500" />
              Ferramentas (Scripts MCP)
            </CardTitle>
            <p className="mt-1 text-xs text-zinc-500">
              Scripts locais expostos ao OpenCode como <code>script_&lt;slug&gt;</code>.
              Entrada validada por JSON Schema, timeout e segredos apenas via
              <code> secret_ref</code>.
            </p>
          </div>
          <div className="flex flex-wrap items-center gap-1.5">
            <Button
              variant="outline"
              size="sm"
              onClick={() => void exportTools()}
              disabled={busy === "export"}
            >
              {busy === "export" ? (
                <Loader2 size={13} className="animate-spin" />
              ) : (
                <Download size={13} />
              )}
              Exportar JSON
            </Button>
            <JsonImportButton
              kind="script_tools"
              endpoint="/api/script-tools/import"
              workspaceId={workspaceId}
              disabled={!workspaceId}
              onImported={load}
            />
            <Button
              size="sm"
              onClick={() => {
                setSelected(null);
                setModalOpen(true);
              }}
            >
              <Plus size={13} />
              Nova ferramenta
            </Button>
          </div>
        </CardHeader>
        <CardContent className="space-y-2">
          {error ? (
            <p className="rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[12px] text-red-300">
              {error}
            </p>
          ) : null}
          {loading && tools.length === 0 ? (
            <p className="flex items-center gap-2 text-[12px] text-zinc-500">
              <Loader2 size={12} className="animate-spin" />
              Carregando ferramentas…
            </p>
          ) : tools.length === 0 ? (
            <EmptyState
              icon={Terminal}
              title="Nenhuma ferramenta ainda"
              description="Crie um script e ele vira uma tool MCP disponível para os agentes deste workspace."
              action={
                <Button
                  size="sm"
                  onClick={() => {
                    setSelected(null);
                    setModalOpen(true);
                  }}
                >
                  <Plus size={13} />
                  Nova ferramenta
                </Button>
              }
            />
          ) : (
            tools.map((tool) => (
              <div
                key={tool.id}
                className="flex flex-wrap items-center gap-2 rounded-lg border border-zinc-800/70 bg-zinc-950/30 px-3 py-2"
              >
                <div className="min-w-0 flex-1">
                  <div className="flex flex-wrap items-center gap-1.5">
                    <p className="truncate text-[13px] font-medium text-zinc-200">
                      {tool.name}
                    </p>
                    <Badge variant="default">script_{tool.slug}</Badge>
                    <Badge variant={tool.workspace_id ? "blue" : "purple"}>
                      {tool.workspace_id ? "workspace" : "global"}
                    </Badge>
                    <Badge variant="default">{tool.runtime}</Badge>
                    {tool.require_approval ? (
                      <Badge variant="yellow">HITL</Badge>
                    ) : null}
                  </div>
                  <p className="mt-0.5 truncate text-[11.5px] text-zinc-500">
                    {tool.description || tool.agent_prompt || "Sem descrição"}
                  </p>
                </div>
                <Switch
                  checked={tool.enabled}
                  onCheckedChange={() => void toggle(tool)}
                  disabled={busy === tool.id}
                  label={tool.enabled ? "Desativar ferramenta" : "Ativar ferramenta"}
                />
                <Button
                  variant="outline"
                  size="sm"
                  className="h-7"
                  onClick={() => {
                    setSelected(tool);
                    setModalOpen(true);
                  }}
                >
                  <Pencil size={11} />
                  Editar
                </Button>
                <Button
                  variant="ghost"
                  size="sm"
                  className="h-7 text-zinc-500 hover:text-red-300"
                  onClick={() => void remove(tool)}
                  disabled={busy === tool.id}
                  aria-label={`Excluir ${tool.name}`}
                >
                  <Trash2 size={11} />
                </Button>
              </div>
            ))
          )}
        </CardContent>
      </Card>

      <ScriptToolModal
        open={modalOpen}
        tool={selected}
        workspaceId={workspaceId}
        runtimes={runtimes}
        onClose={() => setModalOpen(false)}
        onSaved={load}
      />
    </>
  );
}

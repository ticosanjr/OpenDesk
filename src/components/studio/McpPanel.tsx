import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Loader2, Plus, Save, Trash2 } from "lucide-react";
import { apiDelete, apiGet, apiPut } from "@/lib/api";
import type {
  ConfigDocument,
  McpListResponse,
  McpServerEntry,
  StudioScope,
} from "@/lib/types";
import { useAppStore } from "@/store/app-store";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import {
  StudioError,
  StudioField,
  StudioList,
  StudioListItem,
} from "@/components/studio/StudioList";
import { cn } from "@/lib/utils";
import { confirmAction } from "@/store/confirm-store";

const DEFAULT_SERVER = `{
  "type": "local",
  "command": ["npx", "-y", "@modelcontextprotocol/server-everything"]
}`;

export function McpPanel({
  scope,
  workspaceId,
  focusId,
}: {
  scope: StudioScope;
  workspaceId: string | null;
  focusId?: string | null;
}) {
  const lastFsEvent = useAppStore((state) => state.lastFsEvent);
  const query = useMemo(
    () => `scope=${scope}&workspace_id=${encodeURIComponent(workspaceId ?? "")}`,
    [scope, workspaceId],
  );

  const [document, setDocument] = useState<ConfigDocument | null>(null);
  const [servers, setServers] = useState<McpServerEntry[]>([]);
  const [name, setName] = useState("");
  const [text, setText] = useState(DEFAULT_SERVER);
  const [isNew, setIsNew] = useState(true);
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const response = await apiGet<McpListResponse>(`/api/studio/mcp?${query}`);
      setDocument(response.document);
      setServers(response.servers);
      setError(null);
      setName((current) => {
        if (!current || isNew) return current;
        const found = response.servers.find((server) => server.name === current);
        if (found) setText(JSON.stringify(found.server, null, 2));
        return found ? current : "";
      });
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setLoading(false);
    }
  }, [query, isNew]);

  useEffect(() => {
    void load();
  }, [load, lastFsEvent?.path]);

  const lastFocus = useRef<string | null>(null);
  useEffect(() => {
    if (!focusId || lastFocus.current === focusId) return;
    const found = servers.find((server) => server.name === focusId);
    if (found) {
      lastFocus.current = focusId;
      setName(found.name);
      setText(JSON.stringify(found.server, null, 2));
      setIsNew(false);
    }
  }, [focusId, servers]);

  async function save() {
    const serverName = name.trim();
    if (!/^[a-z0-9][a-z0-9-_]*$/.test(serverName)) {
      setError("Nome inválido. Use letras minúsculas, números, '-' ou '_'.");
      return;
    }

    let parsed: unknown;
    try {
      parsed = JSON.parse(text);
    } catch (cause) {
      setError(`JSON inválido: ${cause instanceof Error ? cause.message : String(cause)}`);
      return;
    }
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
      setError("O servidor MCP deve ser um objeto JSON.");
      return;
    }

    setSaving(true);
    setError(null);
    try {
      const next = await apiPut<ConfigDocument>(
        `/api/studio/mcp/${encodeURIComponent(serverName)}?${query}`,
        {
          server: parsed,
          expected_sha256: document?.exists ? document.sha256 : null,
        },
      );
      setDocument(next);
      setIsNew(false);
      setName(serverName);
      await load();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setSaving(false);
    }
  }

  async function remove() {
    const serverName = name.trim();
    if (!serverName || isNew) return;
    if (!(await confirmAction({ title: `Excluir o servidor MCP ${serverName}?`, description: "A entrada será removida do opencode.jsonc do escopo selecionado.", confirmLabel: "Excluir MCP", tone: "destructive" }))) return;
    setSaving(true);
    try {
      const next = await apiDelete<ConfigDocument>(
        `/api/studio/mcp/${encodeURIComponent(serverName)}?${query}&expected_sha256=${encodeURIComponent(
          document?.sha256 ?? "",
        )}`,
      );
      setDocument(next);
      setIsNew(true);
      setName("");
      setText(DEFAULT_SERVER);
      await load();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setSaving(false);
    }
  }

  function startNew() {
    setIsNew(true);
    setName("");
    setText(DEFAULT_SERVER);
    setError(null);
  }

  return (
    <div className="flex min-h-0 flex-1 overflow-hidden">
      <StudioList title="MCP servers" count={servers.length} onNew={startNew} newLabel="Novo servidor">
        {loading && servers.length === 0 ? (
          <p className="px-3 py-6 text-center text-[12px] text-zinc-500">Carregando…</p>
        ) : servers.length === 0 ? (
          <p className="px-3 py-6 text-center text-[12px] text-zinc-500">
            Nenhum servidor MCP no <span className="font-mono">opencode.jsonc</span>.
          </p>
        ) : (
          servers.map((server) => {
            const type =
              typeof server.server.type === "string" ? server.server.type : "?";
            return (
              <StudioListItem
                key={server.name}
                selected={!isNew && name === server.name}
                title={server.name}
                subtitle={
                  type === "remote"
                    ? String(server.server.url ?? "remote")
                    : Array.isArray(server.server.command)
                      ? String(server.server.command[0] ?? "local")
                      : "local"
                }
                badge={<Badge variant={type === "remote" ? "blue" : "default"}>{type}</Badge>}
                onClick={() => {
                  setName(server.name);
                  setText(JSON.stringify(server.server, null, 2));
                  setIsNew(false);
                  setError(null);
                }}
              />
            );
          })
        )}
      </StudioList>

      <div className="min-w-0 flex-1 overflow-y-auto p-5">
        <StudioError error={error} />
        <div className="mx-auto max-w-3xl space-y-4">
          <div className="flex items-start justify-between gap-3">
            <div>
              <div className="flex items-center gap-2">
                <h2 className="text-sm font-semibold text-zinc-100">
                  {isNew ? "Novo servidor MCP" : name}
                </h2>
                <Badge variant="purple">mcp.servers</Badge>
              </div>
              <p className="mt-0.5 font-mono text-[11.5px] text-zinc-500">
                {document?.path ?? "opencode.jsonc"}
              </p>
            </div>
            <div className="flex gap-2">
              {!isNew ? (
                <Button variant="destructive" size="sm" onClick={() => void remove()} disabled={saving}>
                  <Trash2 size={13} />
                  Excluir
                </Button>
              ) : null}
              <Button size="sm" onClick={() => void save()} disabled={saving}>
                {saving ? <Loader2 size={14} className="animate-spin" /> : <Save size={14} />}
                Salvar
              </Button>
            </div>
          </div>

          <StudioField label="Nome" hint="Chave em mcp.servers; letras minúsculas, números, '-' e '_'.">
            <input
              value={name}
              disabled={!isNew}
              onChange={(event) => setName(event.target.value)}
              placeholder="google-workspace"
              className={cn(
                "h-9 w-full rounded-lg border border-zinc-800 bg-zinc-900/50 px-2.5 font-mono text-[13px] text-zinc-100 outline-none placeholder:text-zinc-500 focus:border-blue-500/60 disabled:opacity-60",
              )}
            />
          </StudioField>

          <StudioField
            label="Configuração JSON"
            hint='V2: {"type":"local","command":[…]}, {"type":"remote","url":"…"}; use "disabled": true, nunca "enabled".'
          >
            <textarea
              value={text}
              onChange={(event) => setText(event.target.value)}
              rows={16}
              spellCheck={false}
              className="min-h-[320px] w-full resize-y rounded-lg border border-zinc-800 bg-zinc-950/70 px-3 py-2 font-mono text-[13px] leading-relaxed text-zinc-200 outline-none focus:border-blue-500/60"
            />
          </StudioField>

          {document ? (
            <div className="flex flex-wrap items-center gap-2 rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2 font-mono text-[11.5px] text-zinc-500">
              <span>{document.exists ? "arquivo existente" : "arquivo será criado"}</span>
              <span>·</span>
              <span>sha256 {document.sha256 ? `${document.sha256.slice(0, 12)}…` : "—"}</span>
              <span>·</span>
              <span>comentários preservados pelo editor CST</span>
            </div>
          ) : null}

          <div className="rounded-xl border border-zinc-800/70 bg-zinc-900/20 p-4">
            <p className="flex items-center gap-2 text-[13px] font-medium text-zinc-300">
              <Plus size={13} className="text-zinc-500" />
              Exemplos
            </p>
            <pre className="mt-2 overflow-x-auto rounded-lg bg-zinc-950/60 p-3 font-mono text-[12px] leading-relaxed text-zinc-500">
{`{
  "type": "local",
  "command": ["opendesk", "mcp-google"],
  "environment": { "WORKSPACE_ID": "default" },
  "disabled": false
}`}
            </pre>
          </div>
        </div>
      </div>
    </div>
  );
}

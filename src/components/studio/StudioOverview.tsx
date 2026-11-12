import { useCallback, useEffect, useMemo, useState } from "react";
import {
  ArrowRightLeft,
  Bot,
  BookOpen,
  Copy,
  History,
  Loader2,
  Plus,
  Search,
  Terminal,
  ToggleLeft,
  ToggleRight,
  Trash2,
  Plug,
  RefreshCw,
} from "lucide-react";
import { apiDelete, apiGet, apiPost } from "@/lib/api";
import type {
  BackupEntry,
  EffectiveStudioItem,
  StudioInstallPreview,
  StudioRuntime,
} from "@/lib/types";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Modal } from "@/components/ui/modal";
import { useAppStore } from "@/store/app-store";
import { cn } from "@/lib/utils";
import { StudioInstallModal } from "@/components/studio/StudioInstallModal";
import { confirmAction } from "@/store/confirm-store";

const KIND_META = {
  agent: { label: "Agente", icon: Bot },
  skill: { label: "Skill", icon: BookOpen },
  command: { label: "Comando", icon: Terminal },
  mcp: { label: "MCP", icon: Plug },
} as const;

export function StudioOverview({
  workspaceId,
  onOpenEditor,
}: {
  workspaceId: string | null;
  onOpenEditor: (kind: "agent" | "skill" | "command" | "mcp", scope: "global" | "workspace", id: string) => void;
}) {
  const workspaces = useAppStore((state) => state.workspaces);
  const [items, setItems] = useState<EffectiveStudioItem[]>([]);
  const [runtime, setRuntime] = useState<StudioRuntime | null>(null);
  const [runtimeLocation, setRuntimeLocation] = useState("");
  const [search, setSearch] = useState("");
  const [kindFilter, setKindFilter] = useState<"all" | EffectiveStudioItem["kind"]>("all");
  const [statusFilter, setStatusFilter] = useState<"all" | "enabled" | "disabled">("all");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [transfer, setTransfer] = useState<{
    item: EffectiveStudioItem;
    mode: "copy" | "move";
  } | null>(null);
  const [installOpen, setInstallOpen] = useState(false);
  const [backupsFor, setBackupsFor] = useState<EffectiveStudioItem | null>(null);

  const workspaceRoot = workspaces.find((workspace) => workspace.id === workspaceId)?.root_path ?? "";
  useEffect(() => {
    if (!runtimeLocation && workspaceRoot) setRuntimeLocation(workspaceRoot);
  }, [workspaceRoot, runtimeLocation]);

  const load = useCallback(async () => {
    if (!workspaceId) {
      setItems([]);
      return;
    }
    const locationQuery = runtimeLocation
      ? `&location=${encodeURIComponent(runtimeLocation)}`
      : "";
    setLoading(true);
    try {
      const [response, runtimeResponse] = await Promise.all([
        apiGet<{ items: EffectiveStudioItem[] }>(
          `/api/studio/effective?workspace_id=${encodeURIComponent(workspaceId)}${locationQuery}`,
        ),
        apiGet<StudioRuntime>(
          `/api/studio/runtime?workspace_id=${encodeURIComponent(workspaceId)}${locationQuery}`,
        ).catch(() => null),
      ]);
      setItems(response.items);
      setRuntime(runtimeResponse);
      setError(null);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setLoading(false);
    }
  }, [workspaceId, runtimeLocation]);

  useEffect(() => {
    void load();
  }, [load]);

  const filtered = useMemo(
    () =>
      items.filter((item) => {
        if (kindFilter !== "all" && item.kind !== kindFilter) return false;
        if (statusFilter === "enabled" && item.disabled) return false;
        if (statusFilter === "disabled" && !item.disabled) return false;
        if (search.trim()) {
          const value = item.value as { name?: string; description?: string };
          const haystack = `${item.id} ${value.name ?? ""} ${value.description ?? ""}`.toLowerCase();
          return haystack.includes(search.trim().toLowerCase());
        }
        return true;
      }),
    [items, kindFilter, statusFilter, search],
  );

  async function importRuntime(kind: "skill" | "agent", id: string) {
    setError(null);
    try {
      await apiPost("/api/studio/runtime/import", {
        kind,
        id,
        target_scope: "workspace",
        target_workspace_id: workspaceId,
      });
      setNotice(`Runtime ${kind} ${id} importado para o workspace.`);
      await load();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  }

  async function duplicate(item: EffectiveStudioItem) {
    setError(null);
    try {
      await apiPost(
        `/api/studio/${item.kind}/${encodeURIComponent(item.id)}/duplicate?scope=${item.source_scope}`,
        { new_id: `${item.id}-copy` },
      );
      setNotice(`${KIND_META[item.kind].label} duplicado.`);
      await load();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  }

  async function toggle(item: EffectiveStudioItem) {
    setError(null);
    try {
      await apiPost(
        `/api/studio/${item.kind}/${encodeURIComponent(item.id)}/toggle?scope=${item.source_scope}`,
        { enabled: item.disabled },
      );
      setNotice(item.disabled ? "Item ativado." : "Item desativado.");
      await load();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  }

  async function remove(item: EffectiveStudioItem) {
    if (
      !(await confirmAction({
        title: `Excluir ${KIND_META[item.kind].label} ${item.id}?`,
        description: `O item será removido do escopo ${item.source_scope}.`,
        confirmLabel: "Excluir item",
        tone: "destructive",
      }))
    ) {
      return;
    }
    setError(null);
    try {
      await apiDelete(
        `/api/studio/${item.kind}/${encodeURIComponent(item.id)}?scope=${item.source_scope}${
          item.kind === "mcp" ? `&workspace_id=${workspaceId ?? ""}` : ""
        }`,
      );
      setNotice("Item excluído.");
      await load();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  }

  return (
    <>
      <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
        <div className="flex flex-wrap items-center gap-2 border-b border-zinc-800/70 px-4 py-3">
          <div className="flex h-9 min-w-[220px] flex-1 items-center gap-2 rounded-lg border border-zinc-800 bg-zinc-950/40 px-2.5 focus-within:border-zinc-600">
            <Search size={13} className="text-zinc-500" />
            <input
              value={search}
              onChange={(event) => setSearch(event.target.value)}
              placeholder="Buscar por ID, nome ou descrição…"
              className="h-full flex-1 bg-transparent text-[13px] outline-none placeholder:text-zinc-500"
              aria-label="Buscar no Studio"
            />
          </div>
          <select
            aria-label="Filtrar por tipo"
            value={kindFilter}
            onChange={(event) => setKindFilter(event.target.value as typeof kindFilter)}
            className={selectClass}
          >
            <option value="all">todos os tipos</option>
            <option value="agent">agentes</option>
            <option value="skill">skills</option>
            <option value="command">comandos</option>
            <option value="mcp">MCPs</option>
          </select>
          <select
            aria-label="Filtrar por estado"
            value={statusFilter}
            onChange={(event) => setStatusFilter(event.target.value as typeof statusFilter)}
            className={selectClass}
          >
            <option value="all">todos os estados</option>
            <option value="enabled">ativos</option>
            <option value="disabled">desativados</option>
          </select>
          <div className="flex min-w-[260px] flex-1 items-center gap-1">
            <span className="text-[11px] uppercase tracking-wide text-zinc-500">
              runtime
            </span>
            <input
              value={runtimeLocation}
              onChange={(event) => setRuntimeLocation(event.target.value)}
              placeholder={workspaceRoot || "diretório do OpenCode TUI"}
              className="h-8 min-w-0 flex-1 rounded-md border border-zinc-800 bg-zinc-950/40 px-2.5 font-mono text-[11.5px] text-zinc-300 outline-none focus:border-blue-500/60"
              aria-label="Diretório de runtime do OpenCode"
            />
            <Button
              variant="ghost"
              size="xs"
              onClick={() => void load()}
              disabled={loading}
            >
              carregar
            </Button>
          </div>
          <Badge variant="default">{filtered.length}</Badge>
          <Button variant="outline" size="sm" onClick={() => void load()} disabled={loading}>
            <RefreshCw size={13} className={loading ? "animate-spin" : undefined} />
            Atualizar
          </Button>
          <Button size="sm" onClick={() => setInstallOpen(true)}>
            <Plus size={13} />
            Instalar
          </Button>
        </div>

        {error ? (
          <p className="mx-4 mt-3 rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[10px] text-red-300">
            {error}
          </p>
        ) : null}
        {notice ? (
          <p className="mx-4 mt-3 rounded-lg border border-emerald-500/20 bg-emerald-500/10 px-3 py-2 text-[10px] text-emerald-300">
            {notice}
          </p>
        ) : null}

        <div className="min-h-0 flex-1 overflow-y-auto p-4">
          <div className="mb-3 grid gap-2 lg:grid-cols-3">
            <div className="rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2">
              <p className="text-[9px] uppercase tracking-wide text-zinc-500">
                Inventário em disco
              </p>
              <p className="mt-1 text-[11px] text-zinc-300">
                {items.filter((item) => item.source_scope === "global").length} globais ·{" "}
                {items.filter((item) => item.source_scope === "workspace").length} no workspace ·{" "}
                {items.filter((item) => item.disabled).length} desativados
              </p>
            </div>
            <div className="rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2">
              <p className="text-[9px] uppercase tracking-wide text-zinc-500">
                Agentes runtime do OpenCode
              </p>
              <p className="mt-1 truncate text-[11px] text-zinc-300">
                {runtime?.engine.online
                  ? `${runtime.agents.length} disponíveis · ${runtime.agents
                      .slice(0, 6)
                      .map((agent) => `@${agent.id}`)
                      .join(" ")}`
                  : "engine offline — inicie o motor para listar agentes runtime"}
              </p>
            </div>
            <div className="rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2">
              <p className="text-[9px] uppercase tracking-wide text-zinc-500">
                Modelos disponíveis
              </p>
              <p className="mt-1 text-[11px] text-zinc-300">
                {runtime?.engine.online
                  ? `${runtime.models.length} modelos reportados pelo OpenCode`
                  : "engine offline"}
              </p>
            </div>
          </div>
          {runtime?.engine.online && runtime.agents.length > 0 ? (
            <div className="mb-3 flex flex-wrap gap-1.5">
              {runtime.agents.map((agent) => (
                <Badge key={agent.id} variant="default">
                  @{agent.id}
                </Badge>
              ))}
            </div>
          ) : null}
          {runtime?.engine.online ? (
            <div className="mb-3 grid gap-2 lg:grid-cols-3">
              <div className="rounded-lg border border-zinc-800/70 bg-zinc-950/40 p-3">
                <p className="text-[9px] uppercase tracking-wide text-zinc-500">
                  Skills carregadas pelo OpenCode
                </p>
                <p className="mt-1 text-[11px] text-zinc-300">
                  {runtime.skills.length} disponíveis
                </p>
                <div className="mt-1 flex flex-wrap gap-1">
                  {runtime.skills.slice(0, 12).map((skill) => (
                    <Badge key={skill.id} variant="purple">
                      {skill.id}
                    </Badge>
                  ))}
                  {runtime.skills.length > 12 ? (
                    <Badge variant="default">+{runtime.skills.length - 12}</Badge>
                  ) : null}
                </div>
                {runtime.skills.length > 0 ? (
                  <details className="mt-2">
                    <summary className="cursor-pointer text-[9px] text-zinc-500">
                      ver todas
                    </summary>
                    <div className="mt-1 max-h-40 overflow-y-auto text-[9px] text-zinc-500">
                      {runtime.skills.map((skill) => (
                        <div key={skill.id} className="flex items-center gap-1">
                          <p className="min-w-0 flex-1 truncate">
                            <span className="font-mono text-zinc-300">{skill.id}</span>
                            {skill.description ? ` · ${skill.description}` : ""}
                          </p>
                          <button
                            type="button"
                            className="shrink-0 text-[8px] text-blue-300/80 underline-offset-2 hover:underline"
                            onClick={() => void importRuntime("skill", skill.id)}
                          >
                            importar
                          </button>
                        </div>
                      ))}
                    </div>
                  </details>
                ) : null}
              </div>
              <div className="rounded-lg border border-zinc-800/70 bg-zinc-950/40 p-3">
                <p className="text-[9px] uppercase tracking-wide text-zinc-500">
                  Comandos carregados pelo OpenCode
                </p>
                <p className="mt-1 text-[11px] text-zinc-300">
                  {runtime.commands.length} disponíveis
                </p>
                <div className="mt-1 max-h-40 overflow-y-auto text-[9px] text-zinc-500">
                  {runtime.commands.map((command) => (
                    <p key={command.name} className="truncate">
                      <span className="font-mono text-zinc-300">/{command.name}</span>
                      {command.description ? ` · ${command.description}` : ""}
                    </p>
                  ))}
                </div>
              </div>
              <div className="rounded-lg border border-zinc-800/70 bg-zinc-950/40 p-3">
                <p className="text-[9px] uppercase tracking-wide text-zinc-500">
                  MCPs carregados pelo OpenCode
                </p>
                <p className="mt-1 text-[11px] text-zinc-300">
                  {runtime.mcps.length} configurados
                </p>
                <div className="mt-1 space-y-1">
                  {runtime.mcps.map((mcp) => {
                    const status =
                      typeof mcp.status === "string"
                        ? mcp.status
                        : mcp.status?.status ?? "unknown";
                    return (
                      <div key={mcp.name} className="flex items-center gap-2 text-[9px]">
                        <span className="font-mono text-zinc-300">{mcp.name}</span>
                        <Badge variant={status === "connected" ? "green" : "red"}>
                          {status}
                        </Badge>
                      </div>
                    );
                  })}
                </div>
              </div>
            </div>
          ) : null}
          {filtered.length === 0 ? (
            <div className="rounded-xl border border-dashed border-zinc-800 p-8 text-center text-[11px] text-zinc-500">
              Nenhum item encontrado. Use “Instalar” para importar de GitHub, URL, npm,
              catálogo ou pasta local.
            </div>
          ) : (
            <div className="space-y-2">
              {filtered.map((item) => {
                const meta = KIND_META[item.kind];
                const Icon = meta.icon;
                return (
                  <Card key={`${item.kind}:${item.source_scope}:${item.id}`} className="overflow-hidden">
                    <CardContent className="flex flex-wrap items-center gap-3 p-3">
                      <div className="flex h-8 w-8 items-center justify-center rounded-lg border border-zinc-800 bg-zinc-900/50 text-zinc-500">
                        <Icon size={14} />
                      </div>
                      <div className="min-w-0 flex-1">
                        <div className="flex flex-wrap items-center gap-1.5">
                          <p className="truncate text-[14px] font-semibold text-zinc-100">
                            {String((item.value as { name?: string }).name || item.id)}
                          </p>
                          <Badge variant="default">{meta.label}</Badge>
                          <Badge
                            variant={
                              item.source_scope === "workspace"
                                ? "blue"
                                : item.source_scope === "ancestor"
                                  ? "yellow"
                                  : item.source_scope === "runtime"
                                    ? "green"
                                    : "purple"
                            }
                          >
                            {item.source_scope}
                          </Badge>
                          {item.overridden ? <Badge variant="yellow">override</Badge> : null}
                          {item.disabled ? <Badge variant="red">desativado</Badge> : null}
                        </div>
                        <p className="truncate font-mono text-[11.5px] text-zinc-500">
                          {item.id}
                          {(item.value as { description?: string }).description
                            ? ` · ${(item.value as { description?: string }).description}`
                            : ""}
                        </p>
                      </div>
                      <div className="flex flex-wrap items-center gap-1.5">
                        <Button
                          variant="outline"
                          size="sm"
                          onClick={() => {
                            if (item.source_scope !== "ancestor" && item.source_scope !== "runtime") {
                              onOpenEditor(item.kind, item.source_scope, item.id);
                            }
                          }}
                        >
                          Editar
                        </Button>
                        {item.source_scope !== "ancestor" && item.source_scope !== "runtime" ? (
                          <Button
                            variant="outline"
                            size="xs"
                            onClick={() => void duplicate(item)}
                          >
                            <Copy size={13} />
                            duplicar
                          </Button>
                        ) : null}
                        {item.source_scope !== "ancestor" && item.source_scope !== "runtime" ? (
                          <Button
                            variant="outline"
                            size="xs"
                            onClick={() => setTransfer({ item, mode: "copy" })}
                          >
                            <ArrowRightLeft size={13} />
                            copiar/mover
                          </Button>
                        ) : null}
                        {item.source_scope !== "ancestor" && item.source_scope !== "runtime" ? (
                          <>
                            <Button
                              variant="outline"
                              size="xs"
                              onClick={() => void toggle(item)}
                            >
                              {item.disabled ? <ToggleLeft size={13} /> : <ToggleRight size={13} />}
                              {item.disabled ? "ativar" : "desativar"}
                            </Button>
                            <Button
                              variant="ghost"
                              size="xs"
                              onClick={() => setBackupsFor(item)}
                              title="Ver backups"
                              aria-label="Ver backups"
                            >
                              <History size={13} />
                            </Button>
                            <Button
                              variant="ghost"
                              size="xs"
                              className="text-red-300/80"
                              onClick={() => void remove(item)}
                              title="Excluir item"
                              aria-label="Excluir item"
                            >
                              <Trash2 size={13} />
                            </Button>
                          </>
                        ) : null}
                      </div>
                      <details className="w-full rounded-lg border border-zinc-800/60 bg-zinc-950/30">
                        <summary className="cursor-pointer select-none px-3 py-2 text-[11px] font-medium text-zinc-400 hover:text-zinc-200">
                          ver detalhes, frontmatter e origem
                        </summary>
                        <div className="max-h-64 overflow-auto border-t border-zinc-800/60 p-3">
                          <pre className="whitespace-pre-wrap break-words font-mono text-[12px] leading-relaxed text-zinc-400">
                            {JSON.stringify(item.value, null, 2)}
                          </pre>
                        </div>
                      </details>
                    </CardContent>
                  </Card>
                );
              })}
            </div>
          )}
        </div>
      </div>

      <TransferModal
        transfer={transfer}
        workspaces={workspaces}
        workspaceId={workspaceId}
        onClose={() => setTransfer(null)}
        onDone={async (message) => {
          setTransfer(null);
          setNotice(message);
          await load();
        }}
      />
      <BackupsModal
        item={backupsFor}
        workspaceId={workspaceId}
        onClose={() => setBackupsFor(null)}
        onRestored={async () => {
          setNotice("Backup restaurado.");
          setBackupsFor(null);
          await load();
        }}
      />
      <StudioInstallModal
        open={installOpen}
        workspaceId={workspaceId}
        defaultScope="workspace"
        onClose={() => setInstallOpen(false)}
        onInstalled={async (preview: StudioInstallPreview) => {
          setInstallOpen(false);
          setNotice(`${preview.items.length} item(ns) instalado(s).`);
          await load();
        }}
      />
    </>
  );
}

function TransferModal({
  transfer,
  workspaces,
  workspaceId,
  onClose,
  onDone,
}: {
  transfer: { item: EffectiveStudioItem; mode: "copy" | "move" } | null;
  workspaces: Array<{ id: string; name: string }>;
  workspaceId: string | null;
  onClose: () => void;
  onDone: (message: string) => Promise<void>;
}) {
  const [mode, setMode] = useState<"copy" | "move">("copy");
  const [scope, setScope] = useState<"workspace" | "global">("workspace");
  const [targetWorkspace, setTargetWorkspace] = useState(workspaceId ?? "");
  const [newId, setNewId] = useState("");
  const [overwrite, setOverwrite] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!transfer) return;
    setMode(transfer.mode);
    setScope(
      transfer.item.source_scope === "workspace" ? "global" : "workspace",
    );
    setTargetWorkspace(workspaceId ?? "");
    setNewId(transfer.item.id);
    setOverwrite(false);
    setError(null);
  }, [transfer, workspaceId]);

  if (!transfer) return null;

  async function apply() {
    if (!transfer) return;
    setBusy(true);
    setError(null);
    try {
      await apiPost(
        `/api/studio/${transfer.item.kind}/${encodeURIComponent(transfer.item.id)}/${mode}?scope=${transfer.item.source_scope}${
          transfer.item.source_scope === "workspace" ? `&workspace_id=${workspaceId ?? ""}` : ""
        }`,
        {
          target_scope: scope,
          target_workspace_id: scope === "workspace" ? targetWorkspace : null,
          new_id: newId.trim() || transfer.item.id,
          overwrite,
        },
      );
      await onDone(
        mode === "move" ? "Item movido." : "Item copiado.",
      );
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Modal open onClose={onClose} title={`Copiar/mover ${transfer.item.id}`} size="md">
      <div className="space-y-3">
        <div className="flex gap-2">
          {(["copy", "move"] as const).map((value) => (
            <button
              key={value}
              type="button"
              onClick={() => setMode(value)}
              className={cn(
                "rounded-lg border px-3 py-1.5 text-[11px]",
                mode === value
                  ? "border-blue-500/50 bg-blue-500/10 text-blue-200"
                  : "border-zinc-800 text-zinc-500",
              )}
            >
              {value === "copy" ? "Copiar" : "Mover"}
            </button>
          ))}
        </div>
        <label className="block">
          <span className={labelClass}>Escopo destino</span>
          <select
            value={scope}
            onChange={(event) => setScope(event.target.value as "workspace" | "global")}
            className={inputClass}
          >
            <option value="workspace">workspace</option>
            <option value="global">global</option>
          </select>
        </label>
        {scope === "workspace" ? (
          <label className="block">
            <span className={labelClass}>Workspace destino</span>
            <select
              value={targetWorkspace}
              onChange={(event) => setTargetWorkspace(event.target.value)}
              className={inputClass}
            >
              {workspaces.map((workspace) => (
                <option key={workspace.id} value={workspace.id}>
                  {workspace.name}
                </option>
              ))}
            </select>
          </label>
        ) : null}
        <label className="block">
          <span className={labelClass}>Novo ID</span>
          <input
            value={newId}
            onChange={(event) => setNewId(event.target.value)}
            className={inputClass}
          />
        </label>
        <label className="flex items-center gap-2 text-[11px] text-zinc-400">
          <input
            type="checkbox"
            checked={overwrite}
            onChange={(event) => setOverwrite(event.target.checked)}
            className="accent-blue-500"
          />
          sobrescrever se já existir
        </label>
        {error ? (
          <p className="rounded border border-red-500/20 bg-red-500/10 p-2 text-[10px] text-red-300">
            {error}
          </p>
        ) : null}
        <div className="flex justify-end gap-2 pt-2">
          <Button variant="ghost" size="sm" onClick={onClose}>
            Cancelar
          </Button>
          <Button size="sm" onClick={() => void apply()} disabled={busy}>
            {busy ? <Loader2 size={13} className="animate-spin" /> : null}
            {mode === "copy" ? "Copiar" : "Mover"}
          </Button>
        </div>
      </div>
    </Modal>
  );
}

const inputClass =
  "h-9 w-full rounded-lg border border-zinc-800 bg-zinc-900/50 px-2.5 text-[13px] text-zinc-100 outline-none transition-colors hover:border-zinc-700 focus:border-blue-500/60";
const selectClass =
  "h-9 rounded-lg border border-zinc-800 bg-zinc-900/50 px-2.5 text-[12.5px] text-zinc-300 outline-none transition-colors hover:border-zinc-700 focus:border-blue-500/60";
const labelClass =
  "mb-1 block text-[11px] font-medium uppercase tracking-[0.1em] text-zinc-400";


function BackupsModal({
  item,
  workspaceId,
  onClose,
  onRestored,
}: {
  item: EffectiveStudioItem | null;
  workspaceId: string | null;
  onClose: () => void;
  onRestored: () => Promise<void>;
}) {
  const [backups, setBackups] = useState<BackupEntry[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);

  useEffect(() => {
    if (!item) return;
    setLoading(true);
    setError(null);
    const query = `scope=${item.source_scope}${
      item.source_scope === "workspace" && workspaceId
        ? `&workspace_id=${workspaceId}`
        : ""
    }`;
    apiGet<{ backups: BackupEntry[] }>(
      `/api/studio/${item.kind}/${encodeURIComponent(item.id)}/backups?${query}`,
    )
      .then((value) => setBackups(value.backups))
      .catch((cause) =>
        setError(cause instanceof Error ? cause.message : String(cause)),
      )
      .finally(() => setLoading(false));
  }, [item, workspaceId]);

  if (!item) return null;

  async function restore(backup: BackupEntry) {
    if (!item) return;
    if (!(await confirmAction({ title: "Restaurar backup?", description: `O conteúdo atual será substituído pelo backup de ${backup.created_at}.`, confirmLabel: "Restaurar", tone: "destructive" }))) return;
    setBusy(backup.id);
    setError(null);
    try {
      const query = `scope=${item.source_scope}${
        item.source_scope === "workspace" && workspaceId
          ? `&workspace_id=${workspaceId}`
          : ""
      }`;
      await apiPost(
        `/api/studio/${item.kind}/${encodeURIComponent(item.id)}/restore?${query}`,
        { backup_id: backup.id },
      );
      await onRestored();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(null);
    }
  }

  return (
    <Modal
      open
      onClose={onClose}
      title={`Backups de ${item.id}`}
      description="Cada gravação cria um backup antes de sobrescrever o arquivo nativo."
      size="md"
    >
      <div className="space-y-2">
        {error ? (
          <p className="rounded border border-red-500/20 bg-red-500/10 p-2 text-[10px] text-red-300">
            {error}
          </p>
        ) : null}
        {loading ? (
          <p className="flex items-center gap-2 text-[10px] text-zinc-500">
            <Loader2 size={12} className="animate-spin" />
            carregando…
          </p>
        ) : backups.length === 0 ? (
          <p className="text-[10px] text-zinc-500">
            Nenhum backup para este item ainda.
          </p>
        ) : (
          backups.map((backup) => (
            <div
              key={backup.id}
              className="flex items-center gap-2 rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2"
            >
              <div className="min-w-0 flex-1">
                <p className="text-[10px] text-zinc-300">{backup.created_at}</p>
                <p className="truncate font-mono text-[9px] text-zinc-500">
                  {backup.id}
                </p>
              </div>
              <Button
                variant="outline"
                size="sm"
                className="h-7"
                onClick={() => void restore(backup)}
                disabled={busy !== null}
              >
                {busy === backup.id ? (
                  <Loader2 size={12} className="animate-spin" />
                ) : null}
                restaurar
              </Button>
            </div>
          ))
        )}
      </div>
    </Modal>
  );
}

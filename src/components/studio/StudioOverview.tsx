import { useCallback, useEffect, useMemo, useState } from "react";
import { apiDelete, apiGet, apiPost } from "@/lib/api";
import type {
  EffectiveStudioItem,
  StudioInstallPreview,
  StudioRuntime,
} from "@/lib/types";
import { useAppStore } from "@/store/app-store";
import { StudioInstallModal } from "@/components/studio/StudioInstallModal";
import { confirmAction } from "@/store/confirm-store";
import { useFavorites } from "@/lib/favorites";
import { BackupsModal } from "./overview/BackupsModal";
import { StudioItemCard } from "./overview/StudioItemCard";
import { StudioOverviewToolbar } from "./overview/StudioOverviewToolbar";
import { StudioRuntimeSummary } from "./overview/StudioRuntimeSummary";
import { TransferModal } from "./overview/TransferModal";
import { KIND_META } from "./overview/studio-kind-meta";

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
  const { toggle: toggleFavorite, isFavorite } = useFavorites();
  const favoriteKey = (item: EffectiveStudioItem) =>
    `${item.source_scope}/${workspaceId ?? "global"}/${item.id}`;

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
        <StudioOverviewToolbar
          search={search}
          onSearchChange={setSearch}
          kindFilter={kindFilter}
          onKindFilterChange={setKindFilter}
          statusFilter={statusFilter}
          onStatusFilterChange={setStatusFilter}
          runtimeLocation={runtimeLocation}
          onRuntimeLocationChange={setRuntimeLocation}
          workspaceRoot={workspaceRoot}
          count={filtered.length}
          loading={loading}
          onLoad={() => void load()}
          onInstall={() => setInstallOpen(true)}
        />

        {error ? (
          <p className="mx-4 mt-3 rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[12px] text-red-300">
            {error}
          </p>
        ) : null}
        {notice ? (
          <p className="mx-4 mt-3 rounded-lg border border-emerald-500/20 bg-emerald-500/10 px-3 py-2 text-[12px] text-emerald-300">
            {notice}
          </p>
        ) : null}

        <div className="min-h-0 flex-1 overflow-y-auto p-4">
          <StudioRuntimeSummary
            items={items}
            runtime={runtime}
            onImportRuntime={(kind, id) => void importRuntime(kind, id)}
          />
          {filtered.length === 0 ? (
            <div className="rounded-xl border border-dashed border-zinc-800 p-8 text-center text-[13px] text-zinc-500">
              Nenhum item encontrado. Use “Instalar” para importar de GitHub, URL, npm,
              catálogo ou pasta local.
            </div>
          ) : (
            <div className="space-y-2">
              {filtered.map((item) => (
                <StudioItemCard
                  key={`${item.kind}:${item.source_scope}:${item.id}`}
                  item={item}
                  isFavorite={isFavorite}
                  favoriteKey={favoriteKey}
                  toggleFavorite={toggleFavorite}
                  onOpenEditor={onOpenEditor}
                  onDuplicate={(value) => void duplicate(value)}
                  onCopyMove={(value) => setTransfer({ item: value, mode: "copy" })}
                  onToggle={(value) => void toggle(value)}
                  onBackups={setBackupsFor}
                  onRemove={(value) => void remove(value)}
                />
              ))}
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

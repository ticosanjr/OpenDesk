import { useEffect, useState } from "react";
import {
  Check,
  ExternalLink,
  FolderOpen,
  FolderPlus,
  HardDrive,
  Info,
  Loader2,
  Save,
  Settings as SettingsIcon,
} from "lucide-react";
import { openPath } from "@tauri-apps/plugin-opener";
import { PageBody, PageHeader } from "@/components/layout/PageHeader";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { EmptyState } from "@/components/ui/empty-state";
import { EngineCard } from "@/components/settings/EngineCard";
import { UpdatesCard } from "@/components/settings/UpdatesCard";
import { useAppStore } from "@/store/app-store";
import { useUiStore } from "@/store/ui-store";
import { apiGet, apiPut } from "@/lib/api";
import { pickFolder } from "@/lib/folder";
import { shortPath } from "@/lib/format";
import { ROUTER_POLICY_LABELS } from "@/lib/types";
import { cn } from "@/lib/utils";

export function SettingsPage() {
  const workspaces = useAppStore((state) => state.workspaces);
  const activeWorkspaceId = useAppStore((state) => state.activeWorkspaceId);
  const selectWorkspace = useAppStore((state) => state.selectWorkspace);
  const setWorkspaceDialogOpen = useUiStore(
    (state) => state.setWorkspaceDialogOpen,
  );

  const [root, setRoot] = useState("");
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    apiGet<{ workspaces_root: string }>("/api/app/defaults")
      .then((defaults) => setRoot(defaults.workspaces_root))
      .catch((cause) => setError(cause instanceof Error ? cause.message : String(cause)))
      .finally(() => setLoading(false));
  }, []);

  async function chooseRoot() {
    const selected = await pickFolder(
      "Pasta padrão para novos workspaces",
      root || undefined,
    );
    if (selected) {
      setRoot(selected);
      setSaved(false);
    }
  }

  async function saveRoot() {
    if (!root.trim()) {
      setError("Informe uma pasta válida.");
      return;
    }
    setSaving(true);
    setError(null);
    try {
      const value = await apiPut<string>("/api/settings/workspaces_root", root.trim());
      setRoot(value);
      setSaved(true);
      window.setTimeout(() => setSaved(false), 2500);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setSaving(false);
    }
  }

  async function reveal(path: string) {
    try {
      await openPath(path);
    } catch (cause) {
      console.warn("OpenDesk: could not open path", cause);
    }
  }

  return (
    <PageBody>
      <PageHeader
        eyebrow="preferências do app"
        title="Configurações"
        description="Pasta padrão para novos workspaces e visão dos workspaces registrados. O OpenDesk não move nem renomeia pastas existentes."
        actions={
          <Button size="sm" onClick={() => setWorkspaceDialogOpen(true)}>
            <FolderPlus size={14} />
            Novo workspace
          </Button>
        }
      />

      <EngineCard />
      <UpdatesCard />

      <div className="grid gap-3 lg:grid-cols-2">
        <Card>
          <CardHeader>
            <div>
              <CardTitle className="flex items-center gap-2">
                <HardDrive size={15} className="text-zinc-500" />
                Pasta padrão de workspaces
              </CardTitle>
              <p className="mt-1 text-xs text-zinc-500">
                Novos workspaces são criados como <span className="font-mono">{"{raiz}/{slug-do-nome}"}</span>.
              </p>
            </div>
            {saved ? (
              <Badge variant="green">
                <Check size={10} />
                salvo
              </Badge>
            ) : null}
          </CardHeader>
          <CardContent className="space-y-3">
            <div className="flex gap-2">
              <div className="relative min-w-0 flex-1">
                <FolderOpen
                  size={15}
                  className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-zinc-600"
                />
                <input
                  value={root}
                  onChange={(event) => {
                    setRoot(event.target.value);
                    setSaved(false);
                  }}
                  placeholder="~/Documents/OpenDesk Workspaces"
                  className={cn(
                    "h-9 w-full rounded-lg border border-zinc-800 bg-zinc-900/50 pl-9 pr-3 font-mono text-[11px] text-zinc-100 outline-none placeholder:text-zinc-600 focus:border-blue-500/60",
                    loading && "opacity-60",
                  )}
                />
              </div>
              <Button variant="outline" size="sm" className="h-9" onClick={() => void chooseRoot()}>
                Escolher
              </Button>
              <Button size="sm" className="h-9" onClick={() => void saveRoot()} disabled={saving || loading}>
                {saving ? <Loader2 size={14} className="animate-spin" /> : <Save size={14} />}
                Salvar
              </Button>
            </div>
            <div className="flex items-center gap-2">
              <Button
                variant="ghost"
                size="sm"
                onClick={() => void reveal(root)}
                disabled={!root}
              >
                <ExternalLink size={12} />
                Abrir pasta
              </Button>
              <p className="text-[9px] text-zinc-600">
                A pasta é criada automaticamente se não existir.
              </p>
            </div>
            {error ? (
              <p className="rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[11px] text-red-300">
                {error}
              </p>
            ) : null}
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <div>
              <CardTitle className="flex items-center gap-2">
                <SettingsIcon size={15} className="text-zinc-500" />
                Sobre
              </CardTitle>
              <p className="mt-1 text-xs text-zinc-500">
                OpenDesk grava configuração nativa e mantém apenas estado operacional no SQLite.
              </p>
            </div>
            <Badge variant="blue">V2-only</Badge>
          </CardHeader>
          <CardContent className="space-y-2">
            <InfoRow label="Versão" value="0.6.2" />
            <InfoRow label="OpenCode API" value="v2 (/api/*)" />
            <InfoRow label="Fase" value="resiliência (Fase 6)" />
            <InfoRow label="Banco" value="~/.local/share/opendesk/opendesk.db" />
            <InfoRow label="Workspaces" value={String(workspaces.length)} />
          </CardContent>
        </Card>
      </div>

      <Card>
        <CardHeader>
          <div>
            <CardTitle className="flex items-center gap-2">
              <Info size={15} className="text-zinc-500" />
              Workspaces registrados
            </CardTitle>
            <p className="mt-1 text-xs text-zinc-500">
              Você pode registrar pastas de qualquer local do disco.
            </p>
          </div>
          <Badge variant="default">{workspaces.length}</Badge>
        </CardHeader>
        <CardContent>
          {workspaces.length === 0 ? (
            <EmptyState
              icon={FolderPlus}
              title="Nenhum workspace registrado"
              description="Crie uma pasta nova na raiz padrão ou adicione uma pasta existente."
              action={
                <Button size="sm" onClick={() => setWorkspaceDialogOpen(true)}>
                  <FolderPlus size={14} />
                  Adicionar workspace
                </Button>
              }
            />
          ) : (
            <div className="space-y-2">
              {workspaces.map((workspace) => {
                const active = workspace.id === activeWorkspaceId;
                return (
                  <div
                    key={workspace.id}
                    className={cn(
                      "flex flex-wrap items-center gap-3 rounded-lg border px-3 py-2.5",
                      active
                        ? "border-blue-500/30 bg-blue-500/5"
                        : "border-zinc-800/70 bg-zinc-950/40",
                    )}
                  >
                    <span
                      className="h-2.5 w-2.5 shrink-0 rounded-full"
                      style={{ background: workspace.color_hex }}
                    />
                    <div className="min-w-0 flex-1">
                      <div className="flex items-center gap-2">
                        <p className="truncate text-[12px] font-medium text-zinc-200">
                          {workspace.name}
                        </p>
                        {active ? <Badge variant="blue">ativo</Badge> : null}
                      </div>
                      <p className="truncate font-mono text-[9px] text-zinc-600">
                        {shortPath(workspace.root_path, 80)}
                      </p>
                    </div>
                    <Badge variant="default">
                      {ROUTER_POLICY_LABELS[workspace.default_router_policy] ??
                        workspace.default_router_policy}
                    </Badge>
                    <Button
                      variant="outline"
                      size="sm"
                      className="h-7"
                      onClick={() => void reveal(workspace.root_path)}
                    >
                      <ExternalLink size={12} />
                      Abrir
                    </Button>
                    {!active ? (
                      <Button
                        size="sm"
                        className="h-7"
                        onClick={() => void selectWorkspace(workspace.id)}
                      >
                        Ativar
                      </Button>
                    ) : null}
                  </div>
                );
              })}
            </div>
          )}
        </CardContent>
      </Card>
    </PageBody>
  );
}

function InfoRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-center justify-between gap-3 text-[11px]">
      <span className="text-zinc-600">{label}</span>
      <span className="truncate font-mono text-[10px] text-zinc-300">{value}</span>
    </div>
  );
}

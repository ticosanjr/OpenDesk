import { useState } from "react";
import { Download, ExternalLink, FolderPlus, Info, Loader2 } from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { EmptyState } from "@/components/ui/empty-state";
import { JsonImportButton } from "@/components/ui/json-import-button";
import { exportJsonBundle, type BundleEnvelope } from "@/lib/import-export";
import { apiDelete, apiGet } from "@/lib/api";
import { shortPath } from "@/lib/format";
import { ROUTER_POLICY_LABELS } from "@/lib/types";
import type { Workspace } from "@/lib/types";
import { cn } from "@/lib/utils";
import { confirmAction } from "@/store/confirm-store";
import { links } from "@/lib/links";
import { toast } from "@/store/toast-store";

export function WorkspacesCard({
  workspaces,
  activeWorkspaceId,
  onEdit,
  onSelect,
  onReveal,
  onRefresh,
  onOpenDialog,
}: {
  workspaces: Workspace[];
  activeWorkspaceId: string | null;
  onEdit: (workspace: Workspace) => void;
  onSelect: (id: string) => Promise<void>;
  onReveal: (path: string) => void;
  onRefresh: () => Promise<void>;
  onOpenDialog: () => void;
}) {
  const [busy, setBusy] = useState(false);

  async function exportWorkspaces() {
    setBusy(true);
    try {
      const bundle = await apiGet<BundleEnvelope>(
        "/api/bundles/workspaces/export",
      );
      exportJsonBundle(
        "workspaces_bundle",
        bundle.items,
        `opendesk-workspaces-${new Date().toISOString().slice(0, 10)}.json`,
      );
      toast({ tone: "success", title: "Workspaces exportados" });
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível exportar",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setBusy(false);
    }
  }

  return (
    <Card id="settings-workspaces" className="scroll-mt-14">
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
        <div className="flex flex-wrap items-center gap-1.5">
          <Button
            variant="outline"
            size="sm"
            onClick={() => void exportWorkspaces()}
            disabled={busy}
          >
            {busy ? (
              <Loader2 size={13} className="animate-spin" />
            ) : (
              <Download size={13} />
            )}
            Exportar JSON
          </Button>
          <JsonImportButton
            kind="workspaces_bundle"
            endpoint="/api/bundles/workspaces/import"
            workspaceId={null}
            label="Importar JSON"
            onImported={onRefresh}
          />
          <Badge variant="default">{workspaces.length}</Badge>
        </div>
      </CardHeader>
      <CardContent>
        {workspaces.length === 0 ? (
          <EmptyState
            icon={FolderPlus}
            title="Nenhum workspace registrado"
            description="Crie uma pasta nova na raiz padrão ou adicione uma pasta existente."
            action={
              <Button size="sm" onClick={() => onOpenDialog()}>
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
                      ? "workspace-selected"
                      : "border-zinc-800/70 bg-zinc-950/40",
                  )}
                >
                  <span
                    className="h-2.5 w-2.5 shrink-0 rounded-full"
                    style={{ background: workspace.color_hex }}
                  />
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-2">
                      <p className="truncate text-[13.5px] font-medium text-zinc-200">
                        {workspace.name}
                      </p>
                      {active ? <Badge variant="accent">ativo</Badge> : null}
                      {workspace.root_path.startsWith("/tmp/") ||
                      workspace.root_path.startsWith("/var/tmp/") ? (
                        <Badge variant="yellow">temporário</Badge>
                      ) : null}
                    </div>
                    <p className="truncate font-mono text-[11.5px] text-zinc-500">
                      {shortPath(workspace.root_path, 80)}
                    </p>
                    {workspace.root_path.startsWith("/tmp/") ||
                    workspace.root_path.startsWith("/var/tmp/") ? (
                      <p className="mt-0.5 text-[11.5px] leading-relaxed text-amber-300/80">
                        Pasta temporária: edite a raiz para um diretório em
                        Documentos/OpenDesk Workareas para preservar os arquivos.
                      </p>
                    ) : null}
                  </div>
                  <Badge variant="default">
                    {ROUTER_POLICY_LABELS[workspace.default_router_policy] ??
                      workspace.default_router_policy}
                  </Badge>
                  <Button
                    variant="outline"
                    size="sm"
                    className="h-7"
                    onClick={() => onEdit(workspace)}
                  >
                    Editar
                  </Button>
                  <Button
                    variant="outline"
                    size="sm"
                    className="h-7"
                    onClick={() => onReveal(workspace.root_path)}
                  >
                    <ExternalLink size={12} />
                    Abrir
                  </Button>
                  {!active ? (
                    <Button
                      size="sm"
                      className="h-7"
                      onClick={() => void onSelect(workspace.id)}
                    >
                      Ativar
                    </Button>
                  ) : null}
                  <Button
                    variant="destructive"
                    size="sm"
                    className="h-7"
                    onClick={async () => {
                      if (
                        !(await confirmAction({
                          title: `Remover o workspace ${workspace.name}?`,
                          description:
                            "O registro é removido do OpenDesk. Nenhum arquivo ou pasta do disco é apagado.",
                          confirmLabel: "Remover registro",
                          tone: "destructive",
                        }))
                      ) {
                        return;
                      }
                      try {
                        await apiDelete(`/api/workspaces/${workspace.id}`);
                        await onRefresh();
                        toast({
                          tone: "success",
                          title: "Workspace removido",
                          description: "A pasta original continua intacta no disco.",
                          href: links.settings("workspaces"),
                        });
                      } catch (cause) {
                        toast({
                          tone: "error",
                          title: "Não foi possível remover o workspace",
                          description: cause instanceof Error ? cause.message : String(cause),
                        });
                      }
                    }}
                  >
                    Remover
                  </Button>
                </div>
              );
            })}
          </div>
        )}
      </CardContent>
    </Card>
  );
}

import { ExternalLink, FileText, FolderInput, FolderOutput, FolderTree } from "lucide-react";
import { openPath } from "@tauri-apps/plugin-opener";
import { PageBody, PageHeader } from "@/components/layout/PageHeader";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { EmptyState } from "@/components/ui/empty-state";
import { useAppStore } from "@/store/app-store";
import { useUiStore } from "@/store/ui-store";
import { shortPath } from "@/lib/format";

export function FilesPage() {
  const workspaces = useAppStore((state) => state.workspaces);
  const activeWorkspaceId = useAppStore((state) => state.activeWorkspaceId);
  const setWorkspaceDialogOpen = useUiStore(
    (state) => state.setWorkspaceDialogOpen,
  );
  const workspace = workspaces.find((item) => item.id === activeWorkspaceId);

  async function reveal(path: string) {
    try {
      await openPath(path);
    } catch (error) {
      console.warn("OpenDesk: could not open path", error);
    }
  }

  return (
    <PageBody>
      <PageHeader
        eyebrow="explorador estruturado"
        title="Arquivos"
        description="Entradas em input/ e entregáveis determinísticos em output/YYYY-MM/<task-slug>."
        actions={<Badge variant="default">fase 4</Badge>}
      />

      {!workspace ? (
        <EmptyState
          icon={FolderTree}
          title="Nenhum workspace ativo"
          description="Selecione ou crie um workspace para ver as pastas input/ e output/."
          action={
            <Button size="sm" onClick={() => setWorkspaceDialogOpen(true)}>
              Criar workspace
            </Button>
          }
        />
      ) : (
        <>
          <div className="flex flex-wrap items-center gap-2 rounded-xl border border-zinc-800/70 bg-zinc-900/20 px-4 py-3">
            <FolderTree size={15} className="text-zinc-600" />
            <span className="text-[11px] font-medium text-zinc-300">
              {workspace.name}
            </span>
            <span className="font-mono text-[10px] text-zinc-600">
              {shortPath(workspace.root_path, 72)}
            </span>
          </div>

          <div className="grid gap-3 md:grid-cols-2">
            <Card>
              <CardHeader>
                <CardTitle className="flex items-center gap-2">
                  <FolderInput size={15} className="text-zinc-500" />
                  input/
                </CardTitle>
                <Badge variant="blue">entrada</Badge>
              </CardHeader>
              <CardContent className="space-y-3">
                <p className="text-[11px] leading-relaxed text-zinc-500">
                  Referências, briefs, planilhas e arquivos que os agentes devem
                  ler. O OpenDesk nunca move nem apaga conteúdo existente.
                </p>
                <div className="rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2 font-mono text-[10px] text-zinc-500">
                  {workspace.root_path}/input
                </div>
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => void reveal(`${workspace.root_path}/input`)}
                >
                  <ExternalLink size={13} />
                  Abrir pasta
                </Button>
              </CardContent>
            </Card>

            <Card>
              <CardHeader>
                <CardTitle className="flex items-center gap-2">
                  <FolderOutput size={15} className="text-zinc-500" />
                  output/
                </CardTitle>
                <Badge variant="green">entregáveis</Badge>
              </CardHeader>
              <CardContent className="space-y-3">
                <p className="text-[11px] leading-relaxed text-zinc-500">
                  Resultados finais por mês e slug de tarefa. A regra de output é
                  injetada no prompt de toda execução Build ou rotina.
                </p>
                <div className="rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2 font-mono text-[10px] text-zinc-500">
                  {workspace.root_path}/output/2026-11/&lt;task-slug&gt;
                </div>
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => void reveal(`${workspace.root_path}/output`)}
                >
                  <ExternalLink size={13} />
                  Abrir pasta
                </Button>
              </CardContent>
            </Card>
          </div>

          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <FileText size={15} className="text-zinc-500" />
                Árvore estruturada
              </CardTitle>
              <Badge variant="default">fase 2</Badge>
            </CardHeader>
            <CardContent>
              <EmptyState
                icon={FileText}
                title="O explorador de arquivos chega com o watcher"
                description="A Fase 2 adiciona índice incremental, eventos FS_SYNC_EVENT e visualização de Markdown/código."
              />
            </CardContent>
          </Card>
        </>
      )}
    </PageBody>
  );
}

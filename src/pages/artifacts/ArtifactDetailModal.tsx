import { Link } from "react-router-dom";
import {
  Download,
  ExternalLink,
  FileText,
  GitCompare,
  RotateCcw,
  Star,
} from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/ui/empty-state";
import { Modal } from "@/components/ui/modal";
import { EntityLink } from "@/components/ui/entity-link";
import { LazyFileViewer } from "@/components/viewer/LazyFileViewer";
import type { ArchiveEntry } from "@/components/viewer/FileViewer";
import { DiffView } from "@/components/viewer/DiffView";
import { relativeTime, shortPath } from "@/lib/format";
import { links } from "@/lib/links";
import { cn } from "@/lib/utils";
import type { Artifact } from "@/lib/types";
import {
  artifactActionLabel,
  containingFolder,
  type ArtifactContent,
} from "@/pages/artifacts/artifact-helpers";

export function ArtifactDetailModal({
  selected,
  onClose,
  tab,
  onTabChange,
  onTogglePin,
  onRollback,
  onOpenRun,
  onReveal,
  onSave,
  content,
  contentLoading,
  contentError,
  archive,
  archiveLoading,
  archiveError,
}: {
  selected: Artifact | null;
  onClose: () => void;
  tab: "content" | "diff";
  onTabChange: (tab: "content" | "diff") => void;
  onTogglePin: (artifact: Artifact) => void;
  onRollback: (artifact: Artifact) => void;
  onOpenRun: (runId: string) => void;
  onReveal: (path: string) => void;
  onSave: (artifact: Artifact) => void;
  content: ArtifactContent | null;
  contentLoading: boolean;
  contentError: string | null;
  archive: ArchiveEntry[] | null;
  archiveLoading: boolean;
  archiveError: string | null;
}) {
  return (
    <Modal
      open={!!selected}
      onClose={onClose}
      title={selected?.file_path ?? "Artefato"}
      description={
        selected ? shortPath(selected.absolute_path, 90) : undefined
      }
      size="xl"
      className="max-w-[72rem]"
    >
      {selected ? (
        <div className="space-y-3">
          <div className="flex flex-wrap items-start justify-between gap-3">
            <div className="flex flex-wrap items-center gap-1.5">
              <EntityLink
                to={links.agent(
                  selected.created_by_agent,
                  "workspace",
                  selected.workspace_id,
                )}
                chip
              >
                @{selected.created_by_agent}
              </EntityLink>
              <Badge variant="default">
                {artifactActionLabel(selected.action_type)}
              </Badge>
              {selected.source === "chat" ? (
                <>
                  <Badge variant="blue">
                    chat · {selected.channel ?? "desktop"}
                  </Badge>
                  {selected.origin_ref ? (
                    <Link
                      to={links.chat(selected.origin_ref)}
                      className="text-[11.5px] text-zinc-400 underline decoration-zinc-700 underline-offset-2 hover:text-zinc-200"
                    >
                      abrir conversa
                    </Link>
                  ) : null}
                </>
              ) : null}
              <Badge variant="default">{relativeTime(selected.created_at)}</Badge>
            </div>
            <div className="flex flex-wrap items-center gap-1.5">
              <Button
                variant="outline"
                size="sm"
                onClick={() => onTogglePin(selected)}
                title={selected.pinned ? "Desafixar entregável" : "Fixar como entregável"}
              >
                <Star size={13} fill={selected.pinned ? "currentColor" : "none"} />
                {selected.pinned ? "Fixado" : "Fixar"}
              </Button>
              {selected.diff_patch || selected.action_type === "CREATED" ? (
                <Button
                  variant="outline"
                  size="sm"
                  className="border-amber-500/30 text-amber-200"
                  onClick={() => onRollback(selected)}
                  title="Reverter esta alteração no arquivo"
                >
                  <RotateCcw size={13} />
                  Reverter
                </Button>
              ) : null}
              {selected.run_id ? (
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => onOpenRun(selected.run_id ?? "")}
                >
                  Ver execução
                </Button>
              ) : null}
              <Button
                variant="outline"
                size="sm"
                onClick={() => onReveal(containingFolder(selected.absolute_path))}
              >
                <ExternalLink size={13} />
                Abrir pasta
              </Button>
              <Button
                variant="outline"
                size="sm"
                onClick={() => onSave(selected)}
              >
                <Download size={13} />
                Baixar
              </Button>
            </div>
          </div>

          <div className="flex items-center gap-1 border-b border-zinc-800/70 pb-1">
            <button
              type="button"
              onClick={() => onTabChange("content")}
              className={cn(
                "inline-flex h-8 items-center gap-1.5 rounded-md px-2.5 text-[13.5px] transition-colors",
                tab === "content"
                  ? "bg-zinc-800 text-zinc-100"
                  : "text-zinc-500 hover:bg-zinc-900 hover:text-zinc-200",
              )}
            >
              <FileText size={14} />
              Conteúdo
            </button>
            <button
              type="button"
              onClick={() => onTabChange("diff")}
              className={cn(
                "inline-flex h-8 items-center gap-1.5 rounded-md px-2.5 text-[13.5px] transition-colors",
                tab === "diff"
                  ? "bg-zinc-800 text-zinc-100"
                  : "text-zinc-500 hover:bg-zinc-900 hover:text-zinc-200",
              )}
            >
              <GitCompare size={14} />
              Diff
            </button>
          </div>

          <div className="flex h-[min(64dvh,52rem)] min-h-[360px] flex-col overflow-hidden rounded-xl border border-zinc-800/70 bg-zinc-950/30">
            {tab === "content" ? (
              contentLoading ? (
                <p className="p-4 text-[13.5px] text-zinc-500">
                  Carregando visualizador…
                </p>
              ) : contentError ? (
                <div className="space-y-2 p-4">
                  {contentError.includes("outside the workspace") ? (
                    <>
                      <p className="rounded-lg border border-amber-500/20 bg-amber-500/10 px-3 py-2 text-[13.5px] text-amber-200">
                        Este artefato está fora do workspace. Por segurança, a
                        prévia interna fica desativada para arquivos que saem do
                        diretório do workspace.
                      </p>
                      <p className="truncate font-mono text-[12px] text-zinc-500">
                        {selected.absolute_path}
                      </p>
                    </>
                  ) : (
                    <p className="rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[13.5px] text-red-300">
                      Não foi possível carregar a prévia: {contentError}
                    </p>
                  )}
                  <div className="flex flex-wrap gap-2">
                    <Button
                      variant="outline"
                      size="sm"
                      onClick={() => onReveal(selected.absolute_path)}
                      title="Abrir o arquivo no sistema"
                    >
                      Abrir arquivo externamente
                    </Button>
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={() =>
                        void navigator.clipboard.writeText(selected.absolute_path)
                      }
                    >
                      Copiar caminho
                    </Button>
                  </div>
                </div>
              ) : content ? (
                <LazyFileViewer
                  embedded
                  name={selected.file_path}
                  path={content.path}
                  content={content.content}
                  binary={content.binary}
                  mime={content.mime}
                  truncated={content.truncated}
                  size={content.size}
                  rawUrl={`/api/artifacts/${selected.id}/raw`}
                  onOpenExternal={() => onReveal(selected.absolute_path)}
                  onOpenFolder={() =>
                    onReveal(containingFolder(selected.absolute_path))
                  }
                  onDownload={() => onSave(selected)}
                  archive={archive}
                  archiveLoading={archiveLoading}
                  archiveError={archiveError}
                />
              ) : (
                <p className="p-4 text-[13.5px] text-zinc-500">
                  Sem prévia para este artefato.
                </p>
              )
            ) : (
              <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
                <p className="mx-4 mt-3 text-[13.5px] leading-relaxed text-zinc-400">
                  {selected.creation_reason}
                </p>
                {selected.diff_patch ? (
                  <DiffView patch={selected.diff_patch} />
                ) : (
                  <EmptyState
                    icon={GitCompare}
                    title="Sem diff registrado"
                    description="Este artefato não tem patch para comparar."
                    compact
                    className="m-4 border-0 bg-transparent"
                  />
                )}
              </div>
            )}
          </div>
        </div>
      ) : null}
    </Modal>
  );
}

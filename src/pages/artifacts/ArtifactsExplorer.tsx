import {
  Archive,
  Download,
  FileText,
  Star,
  X,
} from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/ui/empty-state";
import { FilterSearch, FilterSelect } from "@/components/ui/filter-bar";
import { formatNumber, relativeTime } from "@/lib/format";
import { cn } from "@/lib/utils";
import type { Artifact } from "@/lib/types";

export function ArtifactsExplorer({
  artifacts,
  agentFilter,
  onAgentFilter,
  actionFilter,
  onActionFilter,
  runFilter,
  onClearRun,
  limit,
  onLoadMore,
  onSelect,
  onSave,
  onTogglePin,
}: {
  artifacts: Artifact[];
  agentFilter: string;
  onAgentFilter: (value: string) => void;
  actionFilter: string;
  onActionFilter: (value: string) => void;
  runFilter: string;
  onClearRun: () => void;
  limit: number;
  onLoadMore: () => void;
  onSelect: (artifact: Artifact) => void;
  onSave: (artifact: Artifact) => void;
  onTogglePin: (artifact: Artifact) => void;
}) {
  return (
    <Card>
      <CardHeader className="gap-3">
        <CardTitle>Explorador de proveniência</CardTitle>
        <div className="flex flex-wrap items-center gap-2">
          <FilterSearch
            value={agentFilter}
            onChange={onAgentFilter}
            placeholder="Filtrar por agente…"
            ariaLabel="Filtrar artefatos por agente"
            className="min-w-[220px]"
          />
          <FilterSelect
            value={actionFilter}
            onChange={onActionFilter}
            ariaLabel="Filtrar artefatos por ação"
          >
            <option value="">toda ação</option>
            <option value="CREATED">criado</option>
            <option value="MODIFIED">modificado</option>
            <option value="DELETED">deletado</option>
          </FilterSelect>
          <Badge variant="default">
            {artifacts.length} {artifacts.length === 1 ? "artefato" : "artefatos"}
          </Badge>
          {runFilter ? (
            <button
              type="button"
              onClick={onClearRun}
              className="inline-flex items-center gap-1 rounded-md border border-blue-500/25 bg-blue-500/10 px-1.5 py-0.5 font-mono text-[11.5px] text-blue-300 hover:text-blue-100"
              title="Remover filtro por execução"
            >
              run {runFilter.slice(0, 8)}
              <X size={9} />
            </button>
          ) : null}
        </div>
      </CardHeader>
      <CardContent className="p-0">
        {artifacts.length === 0 ? (
          <div className="p-4">
            <EmptyState
              icon={Archive}
              title="Nenhum artefato registrado"
              description="Quando um run criar ou modificar arquivos, o stream interceptor grava autor, motivo, timestamp e diff."
              compact
            />
          </div>
        ) : (
          <div className="scroll-area max-h-[calc(100dvh-18rem)] overflow-auto">
            <table className="w-full text-left">
              <thead className="border-b border-zinc-800/70 text-[11.5px] uppercase tracking-[0.12em] text-zinc-500">
                <tr>
                  <th className="px-4 py-2.5 font-medium">Arquivo</th>
                  <th className="px-3 py-2.5 font-medium">Agente</th>
                  <th className="px-3 py-2.5 font-medium">Motivo</th>
                  <th className="px-3 py-2.5 font-medium">Quando</th>
                  <th className="px-3 py-2.5 font-medium">Ação</th>
                  <th className="px-3 py-2.5 font-medium">Destinos</th>
                </tr>
              </thead>
              <tbody>
                {artifacts.map((artifact) => (
                  <tr
                    key={artifact.id}
                    className="cursor-pointer border-b border-zinc-800/40 text-[13px] transition-colors hover:bg-zinc-900/40"
                    onClick={() => onSelect(artifact)}
                  >
                    <td className="px-4 py-2.5">
                      <div className="flex items-center gap-2">
                        <button
                          type="button"
                          title={`Baixar ${artifact.file_path}`}
                          aria-label={`Baixar ${artifact.file_path}`}
                          onClick={(event) => {
                            event.stopPropagation();
                            onSave(artifact);
                          }}
                          className="-m-1 rounded p-1 text-zinc-500 transition-colors hover:text-zinc-100"
                        >
                          <Download size={13} />
                        </button>
                        <button
                          type="button"
                          title={artifact.pinned ? "Desafixar entregável" : "Fixar como entregável"}
                          aria-label={artifact.pinned ? "Desafixar entregável" : "Fixar como entregável"}
                          onClick={(event) => {
                            event.stopPropagation();
                            onTogglePin(artifact);
                          }}
                          className={cn(
                            "-m-1 rounded p-1 transition-colors",
                            artifact.pinned
                              ? "text-amber-300"
                              : "text-zinc-500 hover:text-amber-300",
                          )}
                        >
                          <Star size={13} fill={artifact.pinned ? "currentColor" : "none"} />
                        </button>
                        <FileText size={13} className="shrink-0 text-zinc-500" />
                        <div className="min-w-0">
                          <p className="truncate font-mono text-[12px] text-zinc-200">
                            {artifact.file_path}
                          </p>
                          <p className="font-mono text-[11px] text-zinc-500">
                            {formatNumber(artifact.file_size_bytes)} bytes ·{" "}
                            {artifact.file_type || "file"}
                          </p>
                        </div>
                      </div>
                    </td>
                    <td className="px-3 py-2.5">
                      <div className="flex flex-wrap items-center gap-1">
                        <Badge variant="purple">@{artifact.created_by_agent}</Badge>
                        {artifact.source === "chat" ? (
                          <Badge variant="blue">
                            chat · {artifact.channel ?? "desktop"}
                          </Badge>
                        ) : null}
                      </div>
                    </td>
                    <td className="max-w-[260px] px-3 py-2.5">
                      <p className="truncate text-[13.5px] text-zinc-400">
                        {artifact.creation_reason}
                      </p>
                    </td>
                    <td className="px-3 py-2.5">
                      <span className="font-mono text-[13px] text-zinc-500">
                        {relativeTime(artifact.created_at)}
                      </span>
                    </td>
                    <td className="px-3 py-2.5">
                      <Badge
                        variant={
                          artifact.action_type === "CREATED"
                            ? "green"
                            : artifact.action_type === "DELETED"
                              ? "red"
                              : "yellow"
                        }
                      >
                        {artifact.action_type === "CREATED"
                          ? "criado"
                          : artifact.action_type === "DELETED"
                            ? "excluído"
                            : artifact.action_type === "MODIFIED"
                              ? "alterado"
                              : artifact.action_type}
                      </Badge>
                    </td>
                    <td className="px-3 py-2.5">
                      <div className="flex gap-1">
                        {artifact.google_doc_url ? <Badge variant="blue">docs</Badge> : null}
                        {artifact.google_drive_url ? <Badge variant="blue">drive</Badge> : null}
                        {artifact.emailed_to ? <Badge variant="blue">email</Badge> : null}
                        {!artifact.google_doc_url &&
                        !artifact.google_drive_url &&
                        !artifact.emailed_to ? (
                          <span className="text-[11.5px] text-zinc-500">—</span>
                        ) : null}
                      </div>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
        {artifacts.length >= limit ? (
          <div className="border-t border-zinc-800/60 p-3 text-center">
            <Button variant="outline" size="sm" onClick={onLoadMore}>
              Carregar mais artefatos
            </Button>
          </div>
        ) : null}
      </CardContent>
    </Card>
  );
}

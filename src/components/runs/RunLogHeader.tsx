import {
  BookmarkPlus,
  Braces,
  Copy,
  FileText,
  FolderOpen,
  FolderTree,
  GitFork,
  Loader2,
  RefreshCw,
} from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { CountPill } from "@/components/ui/count-pill";
import { EntityLink } from "@/components/ui/entity-link";
import { IconLabel } from "@/components/ui/icon-label";
import { WorkingIndicator } from "@/components/ui/working-indicator";
import { links } from "@/lib/links";
import { runStatusLabel, runStatusTone } from "@/lib/status";
import {
  formatClock,
  formatDuration,
  formatNumber,
  formatUsd,
  relativeTime,
} from "@/lib/format";
import { cn } from "@/lib/utils";
import type { RunTimeline } from "@/lib/types";
import type { RunEventFilter } from "@/components/runs/run-log-parts";

export function RunLogHeader({
  runId,
  timeline,
  status,
  totals,
  attemptsCount,
  stepsCount,
  view,
  filter,
  bandsOpen,
  showRaw,
  refreshing,
  openingFolder,
  forkLoading,
  onToggleView,
  onFilterChange,
  onToggleBands,
  onToggleRaw,
  onRefresh,
  onOpenFolder,
  onPinMemory,
  onFork,
  onCopyExecution,
}: {
  runId: string;
  timeline: RunTimeline | null;
  status: string;
  totals: { input: number; output: number; tokens: number; cost: number };
  attemptsCount: number;
  stepsCount: number;
  view: "stream" | "bands";
  filter: RunEventFilter;
  bandsOpen: boolean;
  showRaw: boolean;
  refreshing: boolean;
  openingFolder: boolean;
  forkLoading: boolean;
  onToggleView: () => void;
  onFilterChange: (value: RunEventFilter) => void;
  onToggleBands: () => void;
  onToggleRaw: () => void;
  onRefresh: () => void;
  onOpenFolder: () => void;
  onPinMemory: () => void;
  onFork: () => void;
  onCopyExecution: () => void;
}) {
  const run = timeline?.run;

  return (
    <>
      <div className="shrink-0 border-b border-zinc-800/70 px-4 py-3">
        <div className="flex flex-wrap items-center gap-2">
          <Badge variant={runStatusTone(status)}>{runStatusLabel(status)}</Badge>
          {status === "RUNNING" ? <WorkingIndicator label="executando" /> : null}
          {timeline?.status === "RUNNING" || timeline?.log_missing === false ? (
            <span className="inline-flex items-center gap-1 font-mono text-[13px] text-zinc-500">
              <span
                className={cn(
                  "h-1.5 w-1.5 rounded-full",
                  status === "RUNNING"
                    ? "animate-pulse bg-blue-400"
                    : "bg-emerald-400",
                )}
              />
              {status === "RUNNING" ? "ao vivo" : "encerrado"}
            </span>
          ) : null}
          <span className="font-mono text-[11.5px] text-zinc-500">
            {formatDuration(timeline?.duration_ms ?? run?.duration_ms)}
          </span>
          {attemptsCount > 1 ? (
            <Badge
              variant="purple"
              title={`Esta execução tem ${attemptsCount} tentativas. A linha do tempo abaixo é contínua.`}
            >
              {attemptsCount} tentativas
            </Badge>
          ) : null}
          {stepsCount ? (
            <Badge variant="default">{stepsCount} passos</Badge>
          ) : null}
          <span className="ml-auto flex flex-wrap items-center gap-1.5">
            {run?.task_id ? (
              <EntityLink
                to={links.task(run.task_id)}
                chip
                title="Abrir o card da tarefa"
              >
                <IconLabel icon={<FileText size={10} />}>abrir tarefa</IconLabel>
              </EntityLink>
            ) : null}
            {timeline?.artifacts.length ? (
              <EntityLink
                to={`/artifacts?run=${encodeURIComponent(runId)}`}
                chip
                title="Abrir os artefatos desta execução"
              >
                <IconLabel icon={<FileText size={10} />}>artefatos</IconLabel>
                <CountPill value={timeline.artifacts.length} tone="blue" />
              </EntityLink>
            ) : null}
            {timeline?.output_directory ? (
              <>
                <Button
                  variant="ghost"
                  size="sm"
                  className="h-8"
                  onClick={onOpenFolder}
                  disabled={openingFolder}
                  title={
                    openingFolder
                      ? "Abrindo pasta…"
                      : `Abrir a pasta de output no sistema (${timeline.output_directory})`
                  }
                >
                  <IconLabel
                    icon={
                      openingFolder ? (
                        <Loader2 size={12} className="animate-spin" />
                      ) : (
                        <FolderOpen size={12} />
                      )
                    }
                  >
                    pasta
                  </IconLabel>
                </Button>
                {timeline.output_directory_relative ? (
                  <EntityLink
                    to={links.dir(timeline.output_directory_relative)}
                    chip
                    title="Abrir a pasta de output no explorador do OpenDesk"
                  >
                    <IconLabel icon={<FolderTree size={10} />}>explorador</IconLabel>
                  </EntityLink>
                ) : null}
              </>
            ) : null}
            {timeline?.run?.task_id ? (
              <>
                <Button
                  variant="ghost"
                  size="sm"
                  className="h-7"
                  onClick={onPinMemory}
                  title="Salvar o resultado em .opencode/MEMORY.md; você pode desfazer no aviso"
                >
                  <IconLabel icon={<BookmarkPlus size={12} />}>memória</IconLabel>
                </Button>
                <Button
                  variant="ghost"
                  size="sm"
                  className="h-7"
                  onClick={onFork}
                  disabled={forkLoading}
                  title="Revisar e criar uma tarefa independente com os artefatos deste run"
                >
                  {forkLoading ? (
                    <Loader2 size={12} className="animate-spin" />
                  ) : (
                    <GitFork size={12} />
                  )}
                  fork
                </Button>
              </>
            ) : null}
            <Button
              variant="ghost"
              size="sm"
              className="h-7"
              onClick={onCopyExecution}
              title="Copiar a transcrição completa desta execução"
            >
              <IconLabel icon={<Copy size={12} />}>copiar execução</IconLabel>
            </Button>
            {view === "stream" ? (
              <>
                <select
                  value={filter}
                  onChange={(event) =>
                    onFilterChange(event.target.value as RunEventFilter)
                  }
                  aria-label="Filtrar eventos da execução"
                  title="Filtrar eventos da execução"
                  className="h-7 rounded-md border border-zinc-800 bg-zinc-950/60 px-1.5 text-[12px] text-zinc-300"
                >
                  <option value="all">tudo</option>
                  <option value="tools">ferramentas</option>
                  <option value="errors">erros</option>
                  <option value="conversation">conversa</option>
                </select>
                <Button
                  variant="ghost"
                  size="sm"
                  className="h-7"
                  onClick={onToggleBands}
                  title={
                    bandsOpen
                      ? "Recolher todas as seções"
                      : "Expandir todas as seções"
                  }
                >
                  {bandsOpen ? "recolher" : "expandir"}
                </Button>
              </>
            ) : null}
            <Button
              variant="ghost"
              size="sm"
              className="h-7"
              onClick={onToggleView}
              title="Alternar entre linha do tempo e resumo agrupado"
            >
              {view === "stream" ? "resumo" : "linha do tempo"}
            </Button>
            <Button variant="ghost" size="sm" className="h-7" onClick={onToggleRaw}>
              <Braces size={12} />
              {showRaw ? "timeline" : "bruto"}
            </Button>
            <Button
              variant="ghost"
              size="sm"
              className="h-7"
              onClick={onRefresh}
              title="Recarregar a execução"
              disabled={refreshing}
            >
              <RefreshCw size={12} className={refreshing ? "animate-spin" : undefined} />
            </Button>
          </span>
        </div>
        <div className="mt-1 flex flex-wrap items-center gap-x-3 gap-y-1 font-mono text-[11.5px] text-zinc-500">
          <span>{run?.agent_name ?? "agente"}</span>
          <span>{run?.model_id ?? "modelo"}</span>
          {run ? (
            <>
              <span>
                ↑ {formatNumber(totals.input)} · ↓ {formatNumber(totals.output)} · Σ{" "}
                {formatNumber(totals.tokens)} tokens
              </span>
              <span>{formatUsd(totals.cost)}</span>
            </>
          ) : null}
          <span>início {formatClock(timeline?.started_at ?? run?.started_at)}</span>
          <span>fim {formatClock(timeline?.completed_at ?? run?.completed_at)}</span>
          {run?.started_at ? <span>{relativeTime(run.started_at)}</span> : null}
        </div>
        <div className="mt-2 flex items-center gap-2">
          <span className="font-mono text-[12px] text-zinc-500">contexto</span>
          <div className="h-1.5 flex-1 overflow-hidden rounded-full bg-zinc-800">
            <div
              className={cn(
                "h-full rounded-full",
                totals.tokens > 102_400
                  ? "bg-red-400"
                  : totals.tokens > 81_920
                    ? "bg-amber-400"
                    : "bg-blue-400",
              )}
              style={{ width: `${Math.min(100, (totals.tokens / 128_000) * 100)}%` }}
            />
          </div>
          <span className="font-mono text-[12px] text-zinc-500">
            {formatNumber(totals.tokens)} / 128k
          </span>
        </div>
      </div>

      {timeline?.artifacts.length ? (
        <div className="flex shrink-0 flex-wrap items-center gap-1.5 border-b border-zinc-800/70 bg-zinc-950/40 px-4 py-2">
          <span className="mr-1 font-mono text-[11.5px] uppercase tracking-wide text-zinc-500">
            entregáveis
          </span>
          {timeline.artifacts.slice(0, 8).map((artifact) => (
            <EntityLink key={artifact.id} to={links.artifact(artifact.id)} chip>
              <FileText size={9} />
              {artifact.file_path.split("/").pop() ?? artifact.file_path}
            </EntityLink>
          ))}
          {timeline.artifacts.length > 8 ? (
            <EntityLink to={`/artifacts?run=${encodeURIComponent(runId)}`} chip>
              +{timeline.artifacts.length - 8}
            </EntityLink>
          ) : null}
        </div>
      ) : null}
    </>
  );
}

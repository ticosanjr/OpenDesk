import { useMemo, useState, type ReactNode } from "react";

import {
  Braces,
  BrainCircuit,
  CheckCircle2,
  ChevronRight,
  Clock3,
  Copy,
  ExternalLink,
  MessageSquare,
  PlayCircle,
  User,
  Wrench,
  XCircle,
} from "lucide-react";
import type {
  RunTimeline,
  TaskMessage,
  TimelineEvent,
  TimelineTool,
} from "@/lib/types";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EntityLink } from "@/components/ui/entity-link";
import { links } from "@/lib/links";
import {
  formatClock,
  formatDuration,
} from "@/lib/format";
import { cn } from "@/lib/utils";


function Expandable({
  open,
  onToggle,
  header,
  actions,
  children,
  className,
  headerClassName,
  contentClassName,
}: {
  open: boolean;
  onToggle: () => void;
  header: ReactNode;
  actions?: ReactNode;
  children: ReactNode;
  className?: string;
  headerClassName?: string;
  contentClassName?: string;
}) {
  return (
    <div
      className={cn(
        "group overflow-hidden rounded-2xl border bg-zinc-950/30 transition-[border-color,background-color,box-shadow] duration-200 hover:border-zinc-700/80 hover:bg-white/[0.015]",
        className,
      )}
    >
      <div className="flex items-center gap-2">
        <button
          type="button"
          aria-expanded={open}
          onClick={onToggle}
          className={cn(
            "flex min-w-0 flex-1 flex-wrap items-center gap-2 px-3.5 py-2.5 text-left transition-colors hover:bg-white/[0.02]",
            headerClassName,
          )}
        >
          {header}
        </button>
        {actions ? (
          <div className="flex shrink-0 items-center gap-1 pr-2">{actions}</div>
        ) : null}
      </div>
      <div
        className={cn(
          "grid transition-[grid-template-rows,opacity] duration-300 ease-out",
          open ? "grid-rows-[1fr] opacity-100" : "grid-rows-[0fr] opacity-0",
        )}
        aria-hidden={!open}
      >
        <div className="overflow-hidden">
          <div
            onClick={(event) => {
              const target = event.target as HTMLElement;
              if (
                target.closest(
                  "button,a,input,textarea,select,[data-no-collapse]",
                )
              ) {
                return;
              }
              if (open) onToggle();
            }}
            className={cn(
              "border-t border-zinc-800/60 px-3.5 py-3",
              contentClassName,
            )}
          >
            {children}
          </div>
        </div>
      </div>
    </div>
  );
}

function CommentCard({ event }: { event: TimelineEvent }) {
  const [open, setOpen] = useState(true);
  return (
    <Expandable
      open={open}
      onToggle={() => setOpen((current) => !current)}
      className="border-zinc-800/70 bg-zinc-900/40"
      header={
        <>
          <MessageSquare size={12} className="shrink-0 text-blue-300" />
          <span className="text-[11px] font-semibold text-zinc-200">Resposta</span>
          <span className="font-mono text-[9px] text-zinc-500">
            {formatClock(event.started_at)} → {formatClock(event.ended_at)}
          </span>
          <span className="font-mono text-[9px] text-zinc-500">
            {formatDuration(event.duration_ms)}
          </span>
          <ChevronRight
            size={13}
            className={cn(
              "ml-auto shrink-0 text-zinc-500 transition-transform duration-200",
              open && "rotate-90",
            )}
          />
        </>
      }
    >
      <p className="whitespace-pre-wrap break-words text-[12px] leading-relaxed text-zinc-200">
        {event.text || "Sem texto."}
      </p>
    </Expandable>
  );
}

function GenericEventCard({ event }: { event: TimelineEvent }) {
  const [open, setOpen] = useState(false);
  return (
    <Expandable
      open={open}
      onToggle={() => setOpen((current) => !current)}
      className="border-zinc-800/70 bg-zinc-950/40"
      header={
        <>
          <Braces size={12} className="shrink-0 text-zinc-400" />
          <span className="font-mono text-[10px] text-zinc-300">
            {event.event_type || event.text}
          </span>
          <Badge variant="default">evento</Badge>
          <span className="ml-auto font-mono text-[9px] text-zinc-500">
            {formatClock(event.started_at)}
          </span>
          <ChevronRight
            size={13}
            className={cn(
              "shrink-0 text-zinc-500 transition-transform duration-200",
              open && "rotate-90",
            )}
          />
        </>
      }
    >
      <pre className="max-h-96 overflow-auto whitespace-pre-wrap break-words rounded-lg border border-zinc-800/60 bg-zinc-950 p-2 font-mono text-[10px] leading-relaxed text-zinc-400">
        {event.data ? pretty(event.data) : "Sem dados."}
      </pre>
    </Expandable>
  );
}

export function StreamView({
  timeline,
  messages,
  workspaceRoot,
  summary,
  failedRun,
  copy,
}: {
  timeline: RunTimeline | null;
  messages: TaskMessage[];
  workspaceRoot?: string | null;
  summary?: string;
  failedRun: boolean;
  copy: (text: string) => Promise<void>;
}) {
  const entries = useMemo(() => {
    const items: Array<{
      at: number;
      event?: TimelineEvent;
      message?: TaskMessage;
    }> = [];
    for (const event of timeline?.events ?? []) {
      items.push({
        at: event.started_at ? new Date(event.started_at).getTime() : 0,
        event,
      });
    }
    for (const message of messages) {
      items.push({ at: new Date(message.created_at).getTime(), message });
    }
    return items.sort((left, right) => left.at - right.at);
  }, [messages, timeline?.events]);

  if (entries.length === 0) {
    return (
      <div className="space-y-2">
        <p className="rounded-lg border border-dashed border-zinc-800 px-4 py-8 text-center text-[12px] text-zinc-500">
          Nenhum evento estruturado. Use “bruto” para ver o JSONL original.
        </p>
        <ResultCard
          text={timeline?.result.text?.trim() ?? ""}
          summary={summary}
          failed={failedRun}
          durationMs={timeline?.result.duration_ms ?? 0}
          startedAt={timeline?.result.started_at}
          endedAt={timeline?.result.ended_at}
          truncated={timeline?.result.truncated ?? false}
          copy={copy}
        />
      </div>
    );
  }

  return (
    <div className="space-y-2">
      {entries.map((entry, index) => {
        if (entry.message) {
          const message = entry.message;
          return (
            <div key={`message-${message.id}`} className="flex justify-end">
              <div className="max-w-[85%] rounded-xl border border-blue-500/25 bg-blue-500/10 px-3 py-2 text-blue-100">
                <div className="mb-1 flex items-center justify-end gap-1 font-mono text-[10px] text-blue-300/70">
                  <User size={10} />
                  você · {formatClock(message.created_at)}
                </div>
                <p className="whitespace-pre-wrap text-[12px] leading-relaxed">
                  {message.content}
                </p>
              </div>
            </div>
          );
        }

        const event = entry.event!;
        if (event.kind === "step") {
          return (
            <div
              key={`step-${event.step_index}-${index}`}
              className="flex items-center gap-2 py-1 font-mono text-[11px] font-semibold uppercase tracking-wide text-zinc-500"
            >
              <PlayCircle size={12} />
              passo {event.step_index} · {formatDuration(event.duration_ms)}
            </div>
          );
        }
        if (event.kind === "thinking") {
          return (
            <Band
              key={`thinking-${index}`}
              title="Raciocínio"
              icon={<BrainIcon />}
              tooltip="Pensamento do modelo"
              defaultOpen={false}
              startedAt={event.started_at}
              endedAt={event.ended_at}
              durationMs={event.duration_ms}
              summary={event.text.trim().split("\n")[0] || "pensamento sem texto"}
              tone="purple"
            >
              <pre className="whitespace-pre-wrap break-words font-mono text-[11px] leading-relaxed text-zinc-300">
                {event.text || "Sem conteúdo de raciocínio."}
              </pre>
            </Band>
          );
        }
        if (event.kind === "tool" && event.tool) {
          return (
            <ToolRow
              key={`tool-${event.tool.id}-${index}`}
              tool={event.tool}
              workspaceRoot={workspaceRoot}
            />
          );
        }
        if (event.kind === "comment") {
          return <CommentCard key={`comment-${index}`} event={event} />;
        }
        if (event.kind === "event") {
          return <GenericEventCard key={`event-${index}`} event={event} />;
        }
        return null;
      })}
      <ResultCard
        text={timeline?.result.text?.trim() ?? ""}
        summary={summary}
        failed={failedRun}
        durationMs={timeline?.result.duration_ms ?? 0}
        startedAt={timeline?.result.started_at}
        endedAt={timeline?.result.ended_at}
        truncated={timeline?.result.truncated ?? false}
        copy={copy}
      />
    </div>
  );
}

export function ResultCard({
  text,
  summary,
  failed,
  durationMs,
  startedAt,
  endedAt,
  truncated,
  copy,
}: {
  text: string;
  summary?: string;
  failed: boolean;
  durationMs: number;
  startedAt?: string | null;
  endedAt?: string | null;
  truncated: boolean;
  copy: (text: string) => Promise<void>;
}) {
  const [open, setOpen] = useState(true);
  return (
    <Expandable
      open={open}
      onToggle={() => setOpen((current) => !current)}
      className={cn(
        failed
          ? "border-red-500/20 bg-red-500/[0.035]"
          : "border-emerald-500/20 bg-emerald-500/[0.035]",
      )}
      actions={
        <Button
          variant="ghost"
          size="sm"
          className="h-6"
          onClick={() => void copy(text || summary || "")}
        >
          <Copy size={11} />
          copiar
        </Button>
      }
      header={
        <>
          {failed ? (
            <XCircle size={14} className="text-red-400" />
          ) : (
            <CheckCircle2 size={14} className="text-emerald-400" />
          )}
          <span
            className={cn(
              "text-[12px] font-semibold",
              failed ? "text-red-100" : "text-emerald-100",
            )}
          >
            Resultado
          </span>
          <span className="font-mono text-[10px] text-zinc-500">
            {formatDuration(durationMs)}
          </span>
          <span className="hidden font-mono text-[10px] text-zinc-500 sm:inline">
            {formatClock(startedAt)} → {formatClock(endedAt)}
          </span>
          <ChevronRight
            size={13}
            className={cn(
              "ml-auto shrink-0 text-zinc-500 transition-transform duration-200",
              open && "rotate-90",
            )}
          />
        </>
      }
    >
      <pre className="whitespace-pre-wrap break-words font-sans text-[12px] leading-relaxed text-zinc-100">
        {text || summary || "Sem texto final. Veja as ferramentas executadas e o modo bruto."}
      </pre>
      {truncated ? (
        <p className="mt-2 text-[10px] text-amber-300">
          Resultado truncado. Use o modo bruto para ver o log completo.
        </p>
      ) : null}
    </Expandable>
  );
}

export function Band({
  title,
  icon,
  tooltip,
  defaultOpen,
  startedAt,
  endedAt,
  durationMs,
  summary,
  tone,
  children,
}: {
  title: string;
  icon: ReactNode;
  tooltip?: string;
  defaultOpen: boolean;
  startedAt?: string | null;
  endedAt?: string | null;
  durationMs: number;
  summary: string;
  tone: "purple" | "blue";
  children: ReactNode;
}) {
  const [open, setOpen] = useState(defaultOpen);
  return (
    <Expandable
      open={open}
      onToggle={() => setOpen((current) => !current)}
      className={cn(
        tone === "purple"
          ? "border-violet-500/20 bg-violet-500/[0.035]"
          : "border-blue-500/20 bg-blue-500/[0.035]",
      )}
      header={
        <>
          <ChevronRight
            size={13}
            className={cn(
              "shrink-0 text-zinc-500 transition-transform duration-200",
              open && "rotate-90",
            )}
          />
          <span className="shrink-0">{icon}</span>
          <span
            className={cn(
              "text-[11px] font-semibold",
              tone === "purple" ? "text-violet-100" : "text-blue-100",
            )}
            title={tooltip}
          >
            {title}
          </span>
          <span className="font-mono text-[9px] text-zinc-500">
            {formatDuration(durationMs)}
          </span>
          <span className="hidden font-mono text-[9px] text-zinc-500 sm:inline">
            {formatClock(startedAt)} → {formatClock(endedAt)}
          </span>
          <span className="min-w-0 flex-1 truncate text-right text-[9px] text-zinc-500">
            {summary}
          </span>
        </>
      }
      contentClassName="px-3.5 py-3"
    >
      {children}
    </Expandable>
  );
}

export function ToolRow({
  tool,
  workspaceRoot,
}: {
  tool: TimelineTool;
  workspaceRoot?: string | null;
}) {
  const [open, setOpen] = useState(false);
  const path = useMemo(
    () => relativeToolPath(toolPath(tool.input), workspaceRoot),
    [tool.input, workspaceRoot],
  );
  const failed = tool.status === "failed";
  return (
    <Expandable
      open={open}
      onToggle={() => setOpen((current) => !current)}
      className="border-zinc-800/70 bg-zinc-950/40"
      headerClassName="px-2.5 py-1.5"
      contentClassName="space-y-2 px-2.5 py-2"
      header={
        <>
          {failed ? (
            <XCircle size={12} className="shrink-0 text-red-400" />
          ) : (
            <Wrench size={12} className="shrink-0 text-zinc-500" />
          )}
          <span className="font-mono text-[10px] text-zinc-200">
            {tool.summary}
          </span>
          <Badge variant={failed ? "red" : "green"}>{tool.status}</Badge>
          <span className="ml-auto flex shrink-0 items-center gap-2 font-mono text-[9px] text-zinc-500">
            <Clock3 size={9} />
            {formatClock(tool.started_at)}
            <span>{formatDuration(tool.duration_ms)}</span>
          </span>
          <ChevronRight
            size={13}
            className={cn(
              "shrink-0 text-zinc-500 transition-transform duration-200",
              open && "rotate-90",
            )}
          />
        </>
      }
    >
      <div className="flex flex-wrap items-center gap-2">
        {path ? (
          <EntityLink to={links.file(path)} chip>
            <ExternalLink size={9} />
            abrir {path.split("/").pop()}
          </EntityLink>
        ) : null}
        <span className="font-mono text-[9px] text-zinc-500">
          {formatClock(tool.started_at)} → {formatClock(tool.ended_at)}
        </span>
      </div>
      <div>
        <p className="mb-1 font-mono text-[8px] uppercase tracking-wide text-zinc-500">
          entrada
        </p>
        <pre className="max-h-40 overflow-auto whitespace-pre-wrap break-words rounded border border-zinc-800/60 bg-zinc-950 p-2 font-mono text-[9px] text-zinc-400">
          {pretty(tool.input)}
        </pre>
      </div>
      <div>
        <p className="mb-1 font-mono text-[8px] uppercase tracking-wide text-zinc-500">
          saída
        </p>
        <pre className="max-h-56 overflow-auto whitespace-pre-wrap break-words rounded border border-zinc-800/60 bg-zinc-950 p-2 font-mono text-[9px] text-zinc-400">
          {tool.output || "sem saída"}
        </pre>
        {tool.output_truncated ? (
          <p className="mt-1 text-[9px] text-amber-300">Saída truncada.</p>
        ) : null}
      </div>
    </Expandable>
  );
}

function relativeToolPath(path: string | null, root?: string | null): string | null {
  if (!path) return null;
  if (!path.startsWith("/") && !path.includes(":\\")) return path;
  if (root) {
    const normalizedRoot = root.replace(/\/$/, "");
    if (path.startsWith(normalizedRoot + "/")) {
      return path.slice(normalizedRoot.length + 1);
    }
  }
  return null;
}

function toolPath(input: string): string | null {
  try {
    const value = JSON.parse(input) as Record<string, unknown>;
    const path = value.path ?? value.filePath ?? value.file_path;
    return typeof path === "string" ? path : null;
  } catch {
    return null;
  }
}

function pretty(value: string): string {
  try {
    return JSON.stringify(JSON.parse(value), null, 2);
  } catch {
    return value;
  }
}

export function BrainIcon() {
  return <BrainCircuit size={14} className="text-violet-400" />;
}

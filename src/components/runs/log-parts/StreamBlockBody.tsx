import { MessageSquare, PlayCircle, User, Wrench } from "lucide-react";
import {
  InteractionCard,
  PermissionRequestCard,
} from "@/components/interactions/InteractionCard";
import { formatClock, formatDuration } from "@/lib/format";
import { Band } from "@/components/runs/log-parts/Band";
import { BrainIcon } from "@/components/runs/log-parts/BrainIcon";
import { CommentCard } from "@/components/runs/log-parts/CommentCard";
import { GenericEventCard } from "@/components/runs/log-parts/GenericEventCard";
import { ToolRow } from "@/components/runs/log-parts/ToolRow";
import type { StreamBlock } from "@/components/runs/log-parts/types";

export function StreamBlockBody({
  block,
  index,
  workspaceRoot,
  onInteractionsChanged,
  bandForce,
}: {
  block: StreamBlock;
  index: number;
  workspaceRoot?: string | null;
  onInteractionsChanged?: () => void;
  bandForce?: { signal: number; open: boolean };
}) {
  if (block.kind === "message") {
    const message = block.message;
    return (
      <div className="flex justify-end">
        <div className="max-w-[85%] rounded-xl border border-blue-500/25 bg-blue-500/10 px-3 py-2 text-blue-100">
          <div className="mb-1 flex items-center justify-end gap-1 font-mono text-[12px] text-blue-300/70">
            <User size={10} />
            você · {formatClock(message.created_at)}
          </div>
          <p className="whitespace-pre-wrap text-[13.5px] leading-relaxed">
            {message.content}
          </p>
        </div>
      </div>
    );
  }

  if (block.kind === "tools") {
    const first = block.events[0]?.tool;
    if (block.events.length === 1 && first) {
      return <ToolRow tool={first} workspaceRoot={workspaceRoot} />;
    }
    const duration = block.events.reduce(
      (total, event) => total + event.duration_ms,
      0,
    );
    return (
      <Band
        title={`${block.events.length} ferramentas`}
        icon={<Wrench size={14} />}
        tooltip="Ferramentas executadas em sequência"
        defaultOpen={false}
        startedAt={block.events[0]?.started_at}
        endedAt={block.events[block.events.length - 1]?.ended_at}
        durationMs={duration}
        summary={`${block.events.length} ferramentas · ${formatDuration(duration)}`}
        tone="blue"
        forceOpen={bandForce}
      >
        <div className="space-y-1.5">
          {block.events.map((event) =>
            event.tool ? (
              <ToolRow
                key={event.tool.id}
                tool={event.tool}
                workspaceRoot={workspaceRoot}
              />
            ) : null,
          )}
        </div>
      </Band>
    );
  }

  if (block.kind === "interaction") {
    return (
      <InteractionCard
        interaction={block.interaction}
        onResolved={onInteractionsChanged}
      />
    );
  }

  if (block.kind === "permission") {
    return (
      <PermissionRequestCard
        request={block.permission}
        onDecided={onInteractionsChanged}
      />
    );
  }

  if (block.kind === "comments") {
    if (block.events.length === 1) {
      return <CommentCard event={block.events[0]} />;
    }
    const duration = block.events.reduce(
      (total, event) => total + event.duration_ms,
      0,
    );
    return (
      <Band
        title={`${block.events.length} mensagens`}
        icon={<MessageSquare size={14} />}
        tooltip="Mensagens consecutivas do modelo"
        defaultOpen={false}
        startedAt={block.events[0]?.started_at}
        endedAt={block.events[block.events.length - 1]?.ended_at}
        durationMs={duration}
        summary={`${block.events.length} mensagens`}
        tone="blue"
        forceOpen={bandForce}
      >
        <div className="space-y-1.5">
          {block.events.map((event, eventIndex) => (
            <CommentCard key={`${index}-${eventIndex}`} event={event} />
          ))}
        </div>
      </Band>
    );
  }

  const event = block.event;
  if (event.kind === "step") {
    return (
      <div className="flex items-center gap-2 py-1 font-mono text-[13px] font-semibold uppercase tracking-wide text-zinc-500">
        <PlayCircle size={12} />
        passo {event.step_index} · {formatDuration(event.duration_ms)}
      </div>
    );
  }
  if (event.kind === "thinking") {
    return (
      <Band
        title="Raciocínio"
        icon={<BrainIcon />}
        tooltip="Pensamento do modelo"
        defaultOpen={false}
        startedAt={event.started_at}
        endedAt={event.ended_at}
        durationMs={event.duration_ms}
        summary={
          event.text.trim().split("\n")[0] || "pensamento sem texto"
        }
        tone="purple"
        forceOpen={bandForce}
      >
        <pre className="whitespace-pre-wrap break-words font-mono text-[13px] leading-relaxed text-zinc-300">
          {event.text || "Sem conteúdo de raciocínio."}
        </pre>
      </Band>
    );
  }
  if (event.kind === "event") {
    return <GenericEventCard event={event} />;
  }
  return null;
}

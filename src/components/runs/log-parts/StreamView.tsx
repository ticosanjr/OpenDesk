import { Fragment, useMemo } from "react";

import type {
  RunAttempt,
  RunTimeline,
  TaskInteraction,
  TaskMessage,
  TimelineEvent,
} from "@/lib/types";
import { ResultCard } from "@/components/runs/log-parts/ResultCard";
import { StreamBlockBody } from "@/components/runs/log-parts/StreamBlockBody";
import { isFailedTool } from "@/components/runs/log-parts/helpers";
import type {
  InteractionPermission,
  RunEventFilter,
  StreamBlock,
} from "@/components/runs/log-parts/types";

export function StreamView({
  timeline,
  messages,
  workspaceRoot,
  summary,
  failedRun,
  copy,
  interactions = [],
  permissions = [],
  onInteractionsChanged,
  filter = "all",
  bandForce,
}: {
  timeline: RunTimeline | null;
  messages: TaskMessage[];
  workspaceRoot?: string | null;
  summary?: string;
  failedRun: boolean;
  copy: (text: string) => Promise<void>;
  interactions?: TaskInteraction[];
  permissions?: InteractionPermission[];
  onInteractionsChanged?: () => void;
  filter?: RunEventFilter;
  bandForce?: { signal: number; open: boolean };
}) {
  const entries = useMemo(() => {
    const items: Array<{
      at: number;
      event?: TimelineEvent;
      message?: TaskMessage;
      interaction?: TaskInteraction;
      permission?: InteractionPermission;
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
    for (const interaction of interactions) {
      items.push({
        at: new Date(interaction.created_at).getTime(),
        interaction,
      });
    }
    for (const permission of permissions) {
      items.push({
        at: permission.created_at
          ? new Date(permission.created_at).getTime()
          : 0,
        permission,
      });
    }
    const matches = (entry: (typeof items)[number]) => {
      // Questions, permissions and user messages stay visible under every
      // filter: they are the only actionable surfaces inside the run view.
      if (entry.message || entry.interaction || entry.permission) return true;
      if (filter === "all") return true;
      if (filter === "tools") {
        return entry.event?.kind === "tool";
      }
      if (filter === "errors") {
        return Boolean(entry.event && isFailedTool(entry.event));
      }
      return entry.event?.kind === "comment";
    };
    return items.filter(matches).sort((left, right) => left.at - right.at);
  }, [filter, interactions, messages, permissions, timeline?.events]);

  const blocks = useMemo<StreamBlock[]>(() => {
    const output: StreamBlock[] = [];
    for (const entry of entries) {
      if (entry.message) {
        output.push({ kind: "message", message: entry.message });
        continue;
      }
      if (entry.interaction) {
        output.push({ kind: "interaction", interaction: entry.interaction });
        continue;
      }
      if (entry.permission) {
        output.push({ kind: "permission", permission: entry.permission });
        continue;
      }
      const event = entry.event!;
      const last = output[output.length - 1];
      if (event.kind === "tool") {
        if (
          last?.kind === "tools" &&
          last.events[0]?.attempt_number === event.attempt_number
        ) {
          last.events.push(event);
        } else {
          output.push({ kind: "tools", events: [event] });
        }
        continue;
      }
      if (event.kind === "comment") {
        if (
          last?.kind === "comments" &&
          last.events[0]?.attempt_number === event.attempt_number
        ) {
          last.events.push(event);
        } else {
          output.push({ kind: "comments", events: [event] });
        }
        continue;
      }
      output.push({ kind: "event", event });
    }
    return output;
  }, [entries]);

  if (entries.length === 0) {
    return (
      <div className="space-y-2">
        <p className="rounded-lg border border-dashed border-zinc-800 px-4 py-8 text-center text-[13.5px] text-zinc-500">
          {filter === "all"
            ? "Nenhum evento estruturado. Use “bruto” para ver o JSONL original."
            : "Nenhum evento corresponde a este filtro."}
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

  let lastAttempt: number | null = null;
  return (
    <div className="space-y-2">
      {blocks.map((block, index) => {
        const blockAttempt =
          block.kind === "message" ||
          block.kind === "interaction" ||
          block.kind === "permission"
            ? null
            : block.kind === "event"
              ? (block.event.attempt_number ?? null)
              : (block.events[0]?.attempt_number ?? null);
        const attempt: RunAttempt | undefined =
          blockAttempt === null
            ? undefined
            : timeline?.attempts?.find(
                (item) => item.attempt_number === blockAttempt,
              );
        const divider =
          blockAttempt !== null &&
          blockAttempt !== lastAttempt &&
          (timeline?.attempts?.length ?? 0) > 1 ? (
            <div className="flex items-center gap-2 pt-1 font-mono text-[12px] uppercase tracking-wide text-zinc-500">
              <span className="h-px flex-1 bg-zinc-800" />
              Tentativa {blockAttempt}
              {attempt ? ` · ${attempt.status}` : ""}
              {attempt
                ? ` · ↑${attempt.tokens_input} ↓${attempt.tokens_output}`
                : ""}
              <span className="h-px flex-1 bg-zinc-800" />
            </div>
          ) : null;
        if (blockAttempt !== null) lastAttempt = blockAttempt;

        return (
          <Fragment key={`block-${index}`}>
            {divider}
            <StreamBlockBody
              block={block}
              index={index}
              workspaceRoot={workspaceRoot}
              onInteractionsChanged={onInteractionsChanged}
              bandForce={bandForce}
            />
          </Fragment>
        );
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

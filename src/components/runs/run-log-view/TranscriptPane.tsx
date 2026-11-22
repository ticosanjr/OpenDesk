import type { ReactNode, RefObject } from "react";

import { ArrowDown } from "lucide-react";
import type {
  RunTimeline,
  TaskInteraction,
  TaskMessage,
} from "@/lib/types";
import { Button } from "@/components/ui/button";
import {
  InteractionCard,
  PermissionRequestCard,
} from "@/components/interactions/InteractionCard";
import { StreamView, type RunEventFilter } from "@/components/runs/run-log-parts";
import { RunLogBands } from "@/components/runs/RunLogBands";

export function TranscriptPane({
  transcriptRef,
  onScroll,
  atBottom,
  onScrollToBottom,
  view,
  filter,
  bandSignal,
  bandsOpen,
  interactions,
  permissions,
  timeline,
  messages,
  summary,
  failedRun,
  error,
  copy,
  onInteractionsChanged,
  onCopy,
  children,
}: {
  transcriptRef: RefObject<HTMLDivElement | null>;
  onScroll: () => void;
  atBottom: boolean;
  onScrollToBottom: () => void;
  view: "stream" | "bands";
  filter: RunEventFilter;
  bandSignal: number;
  bandsOpen: boolean;
  interactions: TaskInteraction[];
  permissions: Array<{
    request_id: string;
    permission_action: string;
    resources?: string | null;
    task_id?: string | null;
    run_id?: string | null;
    created_at?: string;
  }>;
  timeline: RunTimeline | null;
  messages: TaskMessage[];
  summary: string | undefined;
  failedRun: boolean;
  error: string | null;
  copy: (text: string) => Promise<void>;
  onInteractionsChanged: () => void;
  onCopy: (value: string) => void;
  children: ReactNode;
}) {
  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="relative min-h-0 flex-1">
        <div
          ref={transcriptRef}
          onScroll={onScroll}
          className="h-full space-y-2 overflow-auto p-3"
        >
          {view === "bands" ? (
            <>
              {interactions.map((interaction) => (
                <InteractionCard
                  key={interaction.id}
                  interaction={interaction}
                  onResolved={onInteractionsChanged}
                />
              ))}
              {permissions.map((request) => (
                <PermissionRequestCard
                  key={request.request_id}
                  request={request}
                  onDecided={onInteractionsChanged}
                />
              ))}
            </>
          ) : null}
          {view === "stream" ? (
            <StreamView
              timeline={timeline}
              messages={messages}
              workspaceRoot={timeline?.workspace_root}
              summary={summary}
              failedRun={failedRun}
              copy={copy}
              interactions={interactions}
              permissions={permissions}
              onInteractionsChanged={onInteractionsChanged}
              filter={filter}
              bandForce={{ signal: bandSignal, open: bandsOpen }}
            />
          ) : (
            <RunLogBands
              timeline={timeline}
              failedRun={failedRun}
              summary={summary}
              error={error}
              onCopy={onCopy}
            />
          )}
        </div>
        {!atBottom ? (
          <Button
            variant="outline"
            size="sm"
            className="absolute bottom-3 right-4 h-8 gap-1.5 rounded-full bg-zinc-950/95 shadow-lg"
            onClick={() => onScrollToBottom()}
            title="Ir para as ações mais recentes"
          >
            <ArrowDown size={13} />
            ir para o fim
          </Button>
        ) : null}
      </div>
      {children}
    </div>
  );
}

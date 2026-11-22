import { Link } from "react-router-dom";
import { Check, Mail, Pencil, Trash2, X } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { CardHeader, CardTitle } from "@/components/ui/card";
import { EntityLink } from "@/components/ui/entity-link";
import type { ChatThread, ChatTurn } from "@/lib/types";
import { links } from "@/lib/links";

export function ThreadHeader({
  thread,
  workspaceId,
  renaming,
  titleDraft,
  onTitleDraftChange,
  onRename,
  onCancelRename,
  onStartRename,
  onReplyEmail,
  onRemove,
  thinking,
  latestTurn,
}: {
  thread: ChatThread | null;
  workspaceId: string;
  renaming: boolean;
  titleDraft: string;
  onTitleDraftChange: (value: string) => void;
  onRename: () => void;
  onCancelRename: () => void;
  onStartRename: () => void;
  onReplyEmail: () => void;
  onRemove: () => void;
  thinking: boolean;
  latestTurn: ChatTurn | null;
}) {
  return (
    <CardHeader className="px-4 py-3">
      <div className="min-w-0 flex-1">
        {renaming && thread ? (
          <div className="flex items-center gap-1">
            <input
              autoFocus
              value={titleDraft}
              onChange={(event) => onTitleDraftChange(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter") onRename();
                if (event.key === "Escape") onCancelRename();
              }}
              className="h-8 min-w-0 flex-1 rounded-md border border-zinc-700 bg-zinc-950 px-2.5 text-[13.5px] text-zinc-100 outline-none"
            />
            <Button variant="ghost" size="icon" className="h-8 w-8" aria-label="Confirmar novo nome" onClick={onRename}>
              <Check size={14} />
            </Button>
            <Button variant="ghost" size="icon" className="h-8 w-8" aria-label="Cancelar renomeação" onClick={onCancelRename}>
              <X size={14} />
            </Button>
          </div>
        ) : (
          <CardTitle className="truncate text-[13.5px]">
            {thread?.title ?? "Selecione uma conversa"}
          </CardTitle>
        )}
        {thread ? (
          <p className="mt-0.5 flex flex-wrap items-center gap-1 font-mono text-[13px] text-zinc-500">
            <EntityLink
              to={links.agent(thread.agent, "workspace", workspaceId)}
              chip
            >
              @{thread.agent}
            </EntityLink>
            {thread.model ? <span>· {thread.model}</span> : null}
            {thread.opencode_session_id
              ? ` · sessão ${thread.opencode_session_id.slice(0, 12)}`
              : " · sessão será criada no primeiro envio"}
          </p>
        ) : null}
      </div>
      <div className="flex items-center gap-2">
        {thread?.source === "email" ? (
          <Button
            variant="ghost"
            size="sm"
            title="Responder por e-mail"
            onClick={onReplyEmail}
          >
            <Mail size={13} />
          </Button>
        ) : null}
        {thread ? (
          <Button
            variant="ghost"
            size="sm"
            title="Renomear conversa"
            onClick={onStartRename}
          >
            <Pencil size={13} />
          </Button>
        ) : null}
        {thread ? (
          <Button
            variant="ghost"
            size="sm"
            onClick={onRemove}
          >
            <Trash2 size={13} />
          </Button>
        ) : null}
        <Badge variant={thinking ? "yellow" : "blue"}>
          {thinking ? "pensando…" : "pronto"}
        </Badge>
        {latestTurn && !thinking ? (
          <span className="text-[11.5px] text-zinc-500">
            última execução: {latestTurn.status}
            {latestTurn.run_id ? (
              <Link
                to={links.run(latestTurn.run_id)}
                className="ml-1 underline decoration-zinc-600 underline-offset-2 hover:text-zinc-300"
              >
                ver
              </Link>
            ) : null}
          </span>
        ) : null}
      </div>
    </CardHeader>
  );
}

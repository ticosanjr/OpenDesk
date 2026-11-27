import { Trash2 } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { MemoryScopeSelect } from "@/components/learning/MemoryScopeControls";
import { relativeTime } from "@/lib/format";
import type { MemoryUsageScope } from "@/lib/types";

export interface MemoryNote {
  id: string;
  namespace: string;
  kind: string;
  title: string;
  body: string;
  tags_json: string;
  confidence: number;
  trust_level: string;
  expires_at?: string | null;
  usage_scope?: MemoryUsageScope;
  created_at: string;
}

export function StructuredMemoryRow({
  note,
  busy,
  onScopeChange,
  onRemove,
}: {
  note: MemoryNote;
  busy: string | null;
  onScopeChange: (scope: MemoryUsageScope) => void;
  onRemove: () => void;
}) {
  return (
    <div className="flex items-start justify-between gap-3 rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2">
      <div className="min-w-0">
        <div className="flex flex-wrap items-center gap-1.5">
          <Badge variant="blue">{note.namespace}</Badge>
          <span className="text-[13px] font-medium text-zinc-200">{note.title}</span>
          <Badge variant={note.trust_level === "verified" ? "green" : "default"}>
            {note.trust_level}
          </Badge>
          <span className="font-mono text-[10.5px] text-zinc-600">
            {(note.confidence * 100).toFixed(0)}%
          </span>
          <MemoryScopeSelect
            value={note.usage_scope ?? "both"}
            label={note.title}
            disabled={busy === `scope:${note.id}`}
            onChange={onScopeChange}
          />
        </div>
        <p className="mt-0.5 line-clamp-2 text-[12px] text-zinc-500">{note.body}</p>
        <p className="text-[11px] text-zinc-600">
          {note.kind}
          {note.tags_json !== "[]" ? ` · ${note.tags_json}` : ""} ·{" "}
          {relativeTime(note.created_at)}
        </p>
      </div>
      <Button
        variant="ghost"
        size="sm"
        className="h-6 px-2"
        aria-label={`Remover memória ${note.title}`}
        disabled={busy === `note:${note.id}`}
        onClick={onRemove}
      >
        <Trash2 size={11} />
      </Button>
    </div>
  );
}

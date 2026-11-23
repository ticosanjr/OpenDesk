import { RotateCcw } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { relativeTime } from "@/lib/format";

export interface RecipeRunNode {
  id: string;
  node_index: number;
  node_id: string;
  node_path?: string | null;
  parent_path?: string | null;
  item_key?: string | null;
  kind: string;
  status: string;
  attempts: number;
  output_json?: string | null;
  error_message?: string | null;
  started_at?: string | null;
  finished_at?: string | null;
}

function statusVariant(status: string) {
  if (status === "DONE") return "green" as const;
  if (status === "FAILED") return "red" as const;
  if (status === "WAITING_HITL") return "yellow" as const;
  if (status === "RUNNING") return "blue" as const;
  return "default" as const;
}

export function RecipeNodeTree({
  node,
  children,
  depth,
  onRetry,
}: {
  node: RecipeRunNode;
  children: Map<string, RecipeRunNode[]>;
  depth: number;
  onRetry?: (path: string) => void;
}) {
  const key = node.node_path ?? node.node_id;
  const nested = children.get(key) ?? [];
  return (
    <div className={depth > 0 ? "border-l border-zinc-800/70 pl-3" : undefined}>
      <div className="flex flex-wrap items-start justify-between gap-2 rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2">
        <div className="min-w-0">
          <div className="flex flex-wrap items-center gap-1.5">
            <Badge variant={statusVariant(node.status)}>{node.status}</Badge>
            <span className="font-mono text-[11px] text-zinc-500">#{node.node_index}</span>
            <span className="text-[13px] text-zinc-200">
              {node.node_path ?? node.node_id}
            </span>
            <Badge variant="default">{node.kind}</Badge>
            {node.item_key ? (
              <span className="font-mono text-[11px] text-zinc-500">
                item {node.item_key}
              </span>
            ) : null}
            {node.attempts > 1 ? (
              <span className="text-[11px] text-zinc-500">{node.attempts} tentativas</span>
            ) : null}
          </div>
          {node.error_message ? (
            <p className="mt-1 whitespace-pre-wrap text-[11.5px] text-red-300">
              {node.error_message}
            </p>
          ) : null}
          {node.output_json ? (
            <pre className="mt-1 max-h-24 overflow-auto whitespace-pre-wrap font-mono text-[10.5px] text-zinc-500">
              {node.output_json}
            </pre>
          ) : null}
        </div>
        <div className="flex flex-col items-end gap-1">
          <span className="text-[11px] text-zinc-600">
            {node.started_at ? relativeTime(node.started_at) : "não iniciado"}
            {node.finished_at ? ` · fim ${relativeTime(node.finished_at)}` : ""}
          </span>
          {node.status === "FAILED" && onRetry ? (
            <Button
              variant="ghost"
              size="sm"
              onClick={() => onRetry(node.node_path ?? node.node_id)}
            >
              <RotateCcw size={11} /> Retomar nó
            </Button>
          ) : null}
        </div>
      </div>
      {nested.length > 0 ? (
        <div className="mt-1.5 space-y-1.5">
          {nested.map((child) => (
            <RecipeNodeTree
              key={child.id}
              node={child}
              children={children}
              depth={depth + 1}
              onRetry={onRetry}
            />
          ))}
        </div>
      ) : null}
    </div>
  );
}

import {
  ArrowRightLeft,
  Copy,
  History,
  Star,
  ToggleLeft,
  ToggleRight,
  Trash2,
} from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import type { FavoriteKind } from "@/lib/favorites";
import type { EffectiveStudioItem } from "@/lib/types";
import { KIND_META } from "./studio-kind-meta";

export function StudioItemCard({
  item,
  isFavorite,
  favoriteKey,
  toggleFavorite,
  onOpenEditor,
  onDuplicate,
  onCopyMove,
  onToggle,
  onBackups,
  onRemove,
}: {
  item: EffectiveStudioItem;
  isFavorite: (kind: FavoriteKind, id: string) => boolean;
  favoriteKey: (item: EffectiveStudioItem) => string;
  toggleFavorite: (kind: FavoriteKind, id: string) => void;
  onOpenEditor: (
    kind: "agent" | "skill" | "command" | "mcp",
    scope: "global" | "workspace",
    id: string,
  ) => void;
  onDuplicate: (item: EffectiveStudioItem) => void;
  onCopyMove: (item: EffectiveStudioItem) => void;
  onToggle: (item: EffectiveStudioItem) => void;
  onBackups: (item: EffectiveStudioItem) => void;
  onRemove: (item: EffectiveStudioItem) => void;
}) {
  const meta = KIND_META[item.kind];
  const Icon = meta.icon;
  return (
    <Card className="overflow-hidden">
      <CardContent className="flex flex-wrap items-center gap-3 p-3">
        <div className="flex h-8 w-8 items-center justify-center rounded-lg border border-zinc-800 bg-zinc-900/50 text-zinc-500">
          <Icon size={14} />
        </div>
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-1.5">
            <p className="truncate text-[14px] font-semibold text-zinc-100">
              {String((item.value as { name?: string }).name || item.id)}
            </p>
            <Badge variant="default">{meta.label}</Badge>
            <Badge
              variant={
                item.source_scope === "workspace"
                  ? "blue"
                  : item.source_scope === "ancestor"
                    ? "yellow"
                    : item.source_scope === "runtime"
                      ? "green"
                      : "purple"
              }
            >
              {item.source_scope}
            </Badge>
            {item.overridden ? <Badge variant="yellow">override</Badge> : null}
            {item.disabled ? <Badge variant="red">desativado</Badge> : null}
          </div>
          <p className="truncate font-mono text-[11.5px] text-zinc-500">
            {item.id}
            {(item.value as { description?: string }).description
              ? ` · ${(item.value as { description?: string }).description}`
              : ""}
          </p>
        </div>
        <div className="flex flex-wrap items-center gap-1.5">
          <Button
            variant="ghost"
            size="sm"
            aria-label={
              isFavorite(item.kind, favoriteKey(item))
                ? `Remover ${item.id} dos favoritos`
                : `Favoritar ${item.id}`
            }
            title={
              isFavorite(item.kind, favoriteKey(item))
                ? "Remover dos favoritos"
                : "Favoritar"
            }
            onClick={() =>
              toggleFavorite(item.kind, favoriteKey(item))
            }
            className={
              isFavorite(item.kind, favoriteKey(item))
                ? "h-8 px-2 text-amber-300"
                : "h-8 px-2 text-zinc-500"
            }
          >
            <Star
              size={13}
              fill={
                isFavorite(item.kind, favoriteKey(item))
                  ? "currentColor"
                  : "none"
              }
            />
          </Button>
          <Button
            variant="outline"
            size="sm"
            onClick={() => {
              if (item.source_scope !== "ancestor" && item.source_scope !== "runtime") {
                onOpenEditor(item.kind, item.source_scope, item.id);
              }
            }}
          >
            Editar
          </Button>
          {item.source_scope !== "ancestor" && item.source_scope !== "runtime" ? (
            <Button
              variant="outline"
              size="xs"
              onClick={() => onDuplicate(item)}
            >
              <Copy size={13} />
              duplicar
            </Button>
          ) : null}
          {item.source_scope !== "ancestor" && item.source_scope !== "runtime" ? (
            <Button
              variant="outline"
              size="xs"
              onClick={() => onCopyMove(item)}
            >
              <ArrowRightLeft size={13} />
              copiar/mover
            </Button>
          ) : null}
          {item.source_scope !== "ancestor" && item.source_scope !== "runtime" ? (
            <>
              <Button
                variant="outline"
                size="xs"
                onClick={() => onToggle(item)}
              >
                {item.disabled ? <ToggleLeft size={13} /> : <ToggleRight size={13} />}
                {item.disabled ? "ativar" : "desativar"}
              </Button>
              <Button
                variant="ghost"
                size="xs"
                onClick={() => onBackups(item)}
                title="Ver backups"
                aria-label="Ver backups"
              >
                <History size={13} />
              </Button>
              <Button
                variant="ghost"
                size="xs"
                className="text-red-300/80"
                onClick={() => onRemove(item)}
                title="Excluir item"
                aria-label="Excluir item"
              >
                <Trash2 size={13} />
              </Button>
            </>
          ) : null}
        </div>
        <details className="w-full rounded-lg border border-zinc-800/60 bg-zinc-950/30">
          <summary className="cursor-pointer select-none px-3 py-2 text-[13px] font-medium text-zinc-400 hover:text-zinc-200">
            ver detalhes, frontmatter e origem
          </summary>
          <div className="max-h-64 overflow-auto border-t border-zinc-800/60 p-3">
            <pre className="whitespace-pre-wrap break-words font-mono text-[13.5px] leading-relaxed text-zinc-400">
              {JSON.stringify(item.value, null, 2)}
            </pre>
          </div>
        </details>
      </CardContent>
    </Card>
  );
}

import { Star } from "lucide-react";
import { cn } from "@/lib/utils";
import type { WorkspaceTemplate } from "./pack-types";

export function WorkspaceTemplateCard({
  template,
  action,
  favorite = false,
  onClick,
  onToggleFavorite,
}: {
  template: WorkspaceTemplate;
  action?: string;
  favorite?: boolean;
  onClick: () => void;
  onToggleFavorite?: () => void;
}) {
  return (
    <div className="relative">
      <button
        type="button"
        onClick={onClick}
        className="w-full rounded-lg border border-zinc-800/70 bg-zinc-950/40 p-3 pr-9 text-left transition-colors hover:border-zinc-600"
      >
        <div className="flex items-center gap-2">
          <span
            className="h-2.5 w-2.5 rounded-full"
            style={{ background: template.color }}
          />
          <span className="text-[13.5px] font-medium text-zinc-200">
            {template.name}
          </span>
        </div>
        <p className="mt-1 text-[12px] leading-snug text-zinc-500">
          {template.description}
        </p>
        <p className="mt-2 text-[12px] text-zinc-500">
          {template.packs.length} pacotes · {template.folders.length} pastas
          {action ? ` · ${action}` : ""}
        </p>
        <p className="mt-1 truncate font-mono text-[11px] text-zinc-500">
          {template.packs.slice(0, 4).join(" · ")}
          {template.packs.length > 4 ? ` +${template.packs.length - 4}` : ""}
        </p>
      </button>
      {onToggleFavorite ? (
        <button
          type="button"
          aria-label={
            favorite
              ? `Remover ${template.name} dos favoritos`
              : `Favoritar ${template.name}`
          }
          title={favorite ? "Remover dos favoritos" : "Favoritar"}
          onClick={onToggleFavorite}
          className={cn(
            "absolute right-2 top-2 rounded-md p-1.5 transition-colors",
            favorite
              ? "text-amber-300 hover:bg-amber-500/10"
              : "text-zinc-500 hover:bg-zinc-800/60 hover:text-zinc-200",
          )}
        >
          <Star size={14} fill={favorite ? "currentColor" : "none"} />
        </button>
      ) : null}
    </div>
  );
}

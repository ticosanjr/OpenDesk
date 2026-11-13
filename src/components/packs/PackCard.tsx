import { CheckCircle2, Star } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";
import type { PackSummary } from "./pack-types";

export function PackCard({
  pack,
  favorite,
  selectable = false,
  selected = false,
  onOpen,
  onToggleFavorite,
  onSelect,
}: {
  pack: PackSummary;
  favorite: boolean;
  selectable?: boolean;
  selected?: boolean;
  onOpen: () => void;
  onToggleFavorite: () => void;
  onSelect?: () => void;
}) {
  return (
    <article
      className={cn(
        "relative rounded-xl border bg-zinc-950/40 transition-colors hover:border-zinc-600",
        pack.installed ? "border-emerald-500/30" : "border-zinc-800/70",
      )}
    >
      {selectable || selected ? (
        <span className="absolute left-2 top-2 z-10">
          <input
            type="checkbox"
            aria-label={`Selecionar ${pack.name}`}
            checked={selected}
            onChange={() => onSelect?.()}
            className="h-4 w-4 accent-blue-500"
          />
        </span>
      ) : null}
      <button
        type="button"
        onClick={selectable ? onSelect : onOpen}
        aria-pressed={selectable ? selected : undefined}
        className={cn(
          "block w-full rounded-xl p-3 text-left outline-none focus-visible:ring-2 focus-visible:ring-blue-500/50",
          selectable && "pl-7",
          selected && "bg-blue-500/[0.05]",
        )}
      >
        <div className="flex items-start gap-2 pr-7">
          <span
            className="mt-0.5 h-2.5 w-2.5 shrink-0 rounded-full"
            style={{ background: pack.color }}
          />
          <div className="min-w-0 flex-1">
            <div className="flex flex-wrap items-center gap-1.5">
              <span className="truncate text-[13.5px] font-medium text-zinc-100">
                {pack.name}
              </span>
              {pack.recommended ? <Badge variant="blue">recomendado</Badge> : null}
              {pack.installed ? (
                <Badge
                  variant={
                    pack.health.status === "partial"
                      ? "red"
                      : pack.update_available
                        ? "yellow"
                        : pack.health.status === "modified"
                          ? "purple"
                          : "green"
                  }
                  title="Este pacote está presente no workspace ativo"
                >
                  <CheckCircle2 size={9} />
                  {pack.health.status === "partial"
                    ? "parcial"
                    : pack.update_available
                      ? "atualização"
                      : pack.health.status === "modified"
                        ? "personalizado"
                        : "instalado"}
                </Badge>
              ) : null}
            </div>
            <p className="mt-1 line-clamp-2 text-[12px] leading-snug text-zinc-500">
              {pack.description}
            </p>
            <div className="mt-2 flex flex-wrap items-center gap-1 text-[11.5px] text-zinc-500">
              <span>{pack.counts.agents} agentes</span>
              <span>·</span>
              <span>{pack.counts.recipes} receitas</span>
              <span>·</span>
              <span>{pack.counts.skills} skills</span>
              {pack.counts.routines > 0 ? (
                <>
                  <span>·</span>
                  <span>{pack.counts.routines} rotinas</span>
                </>
              ) : null}
            </div>
            {pack.requires.tools.length || pack.requires.integrations.length ? (
              <div className="mt-1 flex flex-wrap gap-1">
                {pack.requires.tools.map((tool) => (
                  <Badge key={tool} variant="yellow">
                    {tool}
                  </Badge>
                ))}
                {pack.requires.integrations.map((integration) => (
                  <Badge key={integration} variant="purple">
                    {integration}
                  </Badge>
                ))}
              </div>
            ) : null}
          </div>
        </div>
      </button>
      <button
        type="button"
        aria-label={
          favorite
            ? `Remover ${pack.name} dos favoritos`
            : `Favoritar ${pack.name}`
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
        <Star size={15} fill={favorite ? "currentColor" : "none"} />
      </button>
    </article>
  );
}

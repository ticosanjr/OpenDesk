import { useMemo, useState } from "react";
import { Check, Copy, ExternalLink, Sparkles, Star } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { EmptyState } from "@/components/ui/empty-state";
import { RECOMMENDED_SKILLS, type RecommendedSkill } from "@/lib/recommended-skills";
import { cn } from "@/lib/utils";

export function RecommendedSkills({
  query,
  isFavorite,
  onToggleFavorite,
  onInstall,
}: {
  query: string;
  isFavorite: (id: string) => boolean;
  onToggleFavorite: (id: string) => void;
  onInstall: (skill: RecommendedSkill) => void;
}) {
  const [copied, setCopied] = useState<string | null>(null);
  const [onlyFavorites, setOnlyFavorites] = useState(false);
  const visible = useMemo(() => {
    const needle = query.trim().toLowerCase();
    return RECOMMENDED_SKILLS.filter((skill) => {
      if (onlyFavorites && !isFavorite(skill.id)) return false;
      if (!needle) return true;
      return `${skill.name} ${skill.description} ${skill.tags.join(" ")}`
        .toLowerCase()
        .includes(needle);
    });
  }, [isFavorite, onlyFavorites, query]);

  async function copyCommand(skill: RecommendedSkill) {
    if (!skill.installCommand) return;
    try {
      await navigator.clipboard.writeText(skill.installCommand);
      setCopied(skill.id);
      window.setTimeout(() => setCopied(null), 2000);
    } catch {
      setCopied(null);
    }
  }

  return (
    <Card>
      <CardHeader className="py-3">
        <div>
          <CardTitle className="flex items-center gap-2">
            <Sparkles size={15} className="text-blue-300" />
            Recomendados
          </CardTitle>
          <p className="mt-1 max-w-3xl text-[12px] leading-relaxed text-zinc-400">
            Skills externas curadas para engenharia, revisão, diagnóstico e
            design. A instalação usa o preview do Studio e só grava depois da
            sua confirmação.
          </p>
        </div>
        <div className="flex items-center gap-2">
          <button
            type="button"
            aria-pressed={onlyFavorites}
            onClick={() => setOnlyFavorites((current) => !current)}
            className={cn(
              "inline-flex h-8 items-center gap-1.5 rounded-lg border px-2.5 text-[12px] transition-colors",
              onlyFavorites
                ? "border-amber-500/40 bg-amber-500/10 text-amber-200"
                : "border-zinc-800 text-zinc-500 hover:border-zinc-700 hover:text-zinc-200",
            )}
          >
            <Star size={13} fill={onlyFavorites ? "currentColor" : "none"} />
            Favoritos
          </button>
          <Badge variant="blue">{visible.length}</Badge>
        </div>
      </CardHeader>
      <CardContent>
        {visible.length === 0 ? (
          <EmptyState
            icon={Sparkles}
            title="Nada encontrado"
            description="Ajuste a busca para ver as skills recomendadas."
            compact
            className="border-0 bg-transparent"
          />
        ) : (
          <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-3">
            {visible.map((skill) => {
              const favorite = isFavorite(skill.id);
              return (
                <article
                  key={skill.id}
                  className={cn(
                    "flex min-h-[210px] flex-col rounded-xl border bg-zinc-950/40 p-3 transition-colors",
                    favorite
                      ? "border-amber-500/35"
                      : "border-zinc-800/70 hover:border-zinc-700",
                  )}
                >
                  <div className="flex items-start gap-2">
                    <div className="min-w-0 flex-1">
                      <div className="flex flex-wrap items-center gap-1.5">
                        <p className="truncate text-[14px] font-semibold text-zinc-100">
                          {skill.name}
                        </p>
                        <Badge variant="default">skill</Badge>
                      </div>
                      <p className="mt-1 line-clamp-2 text-[12px] leading-relaxed text-zinc-400">
                        {skill.description}
                      </p>
                    </div>
                    <button
                      type="button"
                      aria-label={
                        favorite
                          ? `Remover ${skill.name} dos favoritos`
                          : `Favoritar ${skill.name}`
                      }
                      title={favorite ? "Remover dos favoritos" : "Favoritar"}
                      onClick={() => onToggleFavorite(skill.id)}
                      className={cn(
                        "rounded-md p-1.5 transition-colors",
                        favorite
                          ? "text-amber-300 hover:bg-amber-500/10"
                          : "text-zinc-500 hover:bg-zinc-800/60 hover:text-zinc-200",
                      )}
                    >
                      <Star size={15} fill={favorite ? "currentColor" : "none"} />
                    </button>
                  </div>

                  <div className="mt-2 flex flex-wrap gap-1">
                    {skill.tags.map((tag) => (
                      <span
                        key={tag}
                        className="rounded border border-zinc-800 bg-zinc-900/60 px-1.5 py-0.5 text-[11px] text-zinc-500"
                      >
                        {tag}
                      </span>
                    ))}
                  </div>

                  {skill.installCommand ? (
                    <div className="mt-2 flex items-center gap-1 rounded-md border border-zinc-800 bg-black/25 px-2 py-1">
                      <code
                        className="min-w-0 flex-1 truncate font-mono text-[11px] text-zinc-400"
                        title={skill.installCommand}
                      >
                        {skill.installCommand}
                      </code>
                      <button
                        type="button"
                        onClick={() => void copyCommand(skill)}
                        aria-label={`Copiar comando de instalação de ${skill.name}`}
                        title="Copiar comando"
                        className="rounded p-1 text-zinc-500 hover:bg-zinc-800 hover:text-zinc-200"
                      >
                        {copied === skill.id ? <Check size={12} /> : <Copy size={12} />}
                      </button>
                    </div>
                  ) : null}

                  {skill.note ? (
                    <p className="mt-2 text-[11px] leading-snug text-zinc-500">
                      {skill.note}
                    </p>
                  ) : null}

                  <div className="mt-auto flex items-center justify-between gap-2 pt-3">
                    <a
                      href={skill.url}
                      target="_blank"
                      rel="noreferrer"
                      className="inline-flex items-center gap-1.5 text-[12px] text-zinc-400 transition-colors hover:text-zinc-200"
                    >
                      <ExternalLink size={13} />
                      Abrir origem
                    </a>
                    {skill.installable === false ? (
                      <a
                        href={skill.url}
                        target="_blank"
                        rel="noreferrer"
                        className="inline-flex h-8 items-center rounded-lg border border-zinc-700/70 bg-zinc-900/30 px-2.5 text-[12.5px] font-medium text-zinc-200 transition-colors hover:border-zinc-600 hover:bg-zinc-800/60 hover:text-white"
                      >
                        Abrir instalador
                      </a>
                    ) : (
                      <Button size="sm" onClick={() => onInstall(skill)}>
                        Instalar
                      </Button>
                    )}
                  </div>
                </article>
              );
            })}
          </div>
        )}
      </CardContent>
    </Card>
  );
}

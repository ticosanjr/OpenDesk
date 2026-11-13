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
  favoritesOnly = false,
}: {
  query: string;
  isFavorite: (id: string) => boolean;
  onToggleFavorite: (id: string) => void;
  onInstall: (skill: RecommendedSkill) => void;
  favoritesOnly?: boolean;
}) {
  const [copied, setCopied] = useState<string | null>(null);
  const visible = useMemo(() => {
    const needle = query.trim().toLowerCase();
    return RECOMMENDED_SKILLS.filter((skill) => {
      if (favoritesOnly && !isFavorite(skill.id)) return false;
      if (!needle) return true;
      return `${skill.name} ${skill.description} ${skill.tags.join(" ")}`
        .toLowerCase()
        .includes(needle);
    });
  }, [favoritesOnly, isFavorite, query]);

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
          <p className="mt-1 max-w-3xl text-[13.5px] leading-relaxed text-zinc-400">
            Skills externas curadas para engenharia, revisão, diagnóstico e
            design. A instalação vira uma tarefa rastreável em Execuções, com
            origem, comando e critério de sucesso.
          </p>
        </div>
        <div className="flex items-center gap-2">
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
                      <p className="mt-1 line-clamp-2 text-[13.5px] leading-relaxed text-zinc-400">
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
                        className="rounded border border-zinc-800 bg-zinc-900/60 px-1.5 py-0.5 text-[13px] text-zinc-500"
                      >
                        {tag}
                      </span>
                    ))}
                  </div>

                  {skill.installCommand ? (
                    <div className="mt-2 flex items-center gap-1 rounded-md border border-zinc-800 bg-black/25 px-2 py-1">
                      <code
                        className="min-w-0 flex-1 truncate font-mono text-[13px] text-zinc-400"
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
                    <p className="mt-2 text-[13px] leading-snug text-zinc-500">
                      {skill.note}
                    </p>
                  ) : null}

                  <div className="mt-auto flex items-center justify-between gap-2 pt-3">
                    <a
                      href={skill.url}
                      target="_blank"
                      rel="noreferrer"
                      className="inline-flex items-center gap-1.5 text-[13.5px] text-zinc-400 transition-colors hover:text-zinc-200"
                    >
                      <ExternalLink size={13} />
                      Abrir origem
                    </a>
                    <Button size="sm" onClick={() => onInstall(skill)}>
                      Instalar via tarefa
                    </Button>
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

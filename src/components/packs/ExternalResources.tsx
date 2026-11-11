import { useMemo, useState } from "react";
import { ExternalLink, Sparkles } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { cn } from "@/lib/utils";
import {
  EXTERNAL_RESOURCES,
  RESOURCE_GROUPS,
  type ExternalResourceGroup,
} from "@/lib/external-resources";

export function ExternalResources({
  availableTemplateIds = [],
  onCreateWorkspace,
  query = "",
}: {
  availableTemplateIds?: string[];
  onCreateWorkspace?: (templateId: string) => void;
  query?: string;
}) {
  const [group, setGroup] = useState<ExternalResourceGroup | "all">("all");
  const visible = useMemo(() => {
    const needle = query.trim().toLowerCase();
    return EXTERNAL_RESOURCES.filter((resource) => {
      if (group !== "all" && resource.group !== group) return false;
      if (!needle) return true;
      return `${resource.name} ${resource.focus} ${resource.usage} ${resource.tags.join(" ")}`
        .toLowerCase()
        .includes(needle);
    });
  }, [group, query]);

  if (visible.length === 0) return null;

  return (
    <Card>
      <CardHeader className="py-3">
        <div>
          <CardTitle className="flex items-center gap-2 text-[12px]">
            <Sparkles size={14} className="text-blue-300" />
            Descobrir capacidades
          </CardTitle>
          <p className="mt-1 max-w-3xl text-[10px] leading-snug text-zinc-500">
            Fontes externas curadas para encontrar skills, agentes, comandos,
            MCPs e workflows. Nada é instalado automaticamente: revise a origem
            e importe apenas o que precisar.
          </p>
        </div>
      </CardHeader>
      <CardContent className="space-y-3">
        <div className="flex flex-wrap gap-1">
          {RESOURCE_GROUPS.map((item) => (
            <button
              key={item.id}
              type="button"
              aria-pressed={group === item.id}
              onClick={() => setGroup(item.id)}
              className={cn(
                "rounded-lg border px-2.5 py-1 text-[10px] transition-colors",
                group === item.id
                  ? "border-blue-500/40 bg-blue-500/10 text-blue-200"
                  : "border-zinc-800 text-zinc-500 hover:border-zinc-700 hover:text-zinc-300",
              )}
              title={item.description}
            >
              {item.label}
            </button>
          ))}
        </div>

        <div className="grid gap-2 md:grid-cols-2 xl:grid-cols-3">
          {visible.map((resource) => {
            const label =
              RESOURCE_GROUPS.find((item) => item.id === resource.group)?.label ??
              resource.group;
            return (
              <article
                key={resource.id}
                className="flex min-h-[154px] flex-col rounded-xl border border-zinc-800/70 bg-zinc-950/40 p-3"
              >
                <div className="flex items-start justify-between gap-2">
                  <div className="min-w-0">
                    <Badge variant="default">{label}</Badge>
                    <a
                      href={resource.url}
                      target="_blank"
                      rel="noreferrer"
                      className="mt-1.5 block truncate text-[12px] font-medium text-zinc-100 hover:text-blue-200"
                    >
                      {resource.name}
                    </a>
                  </div>
                  <a
                    href={resource.url}
                    target="_blank"
                    rel="noreferrer"
                    aria-label={`Abrir ${resource.name}`}
                    className="rounded p-1 text-zinc-500 hover:text-zinc-200 focus-visible:ring-2 focus-visible:ring-blue-500/60"
                  >
                    <ExternalLink size={13} />
                  </a>
                </div>
                <p className="mt-1 line-clamp-2 text-[10px] leading-snug text-zinc-500">
                  {resource.focus}
                </p>
                <p className="mt-1.5 line-clamp-2 text-[9px] leading-snug text-zinc-500">
                  {resource.usage}
                </p>
                <div className="mt-auto flex flex-wrap items-center gap-1 pt-2">
                  {resource.tags.map((tag) => (
                    <span
                      key={tag}
                      className="rounded border border-zinc-800 bg-zinc-900/60 px-1.5 py-0.5 font-mono text-[8px] text-zinc-500"
                    >
                      {tag}
                    </span>
                  ))}
                  {resource.templateId &&
                  availableTemplateIds.includes(resource.templateId) &&
                  onCreateWorkspace ? (
                    <button
                      type="button"
                      onClick={() => onCreateWorkspace(resource.templateId!)}
                      className="ml-auto rounded px-1.5 py-0.5 text-[9px] text-blue-300 hover:bg-blue-500/10 hover:text-blue-200"
                    >
                      Usar template recomendado
                    </button>
                  ) : null}
                </div>
              </article>
            );
          })}
        </div>
      </CardContent>
    </Card>
  );
}

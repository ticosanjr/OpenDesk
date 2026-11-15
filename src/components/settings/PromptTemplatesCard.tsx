import { useCallback, useEffect, useState } from "react";
import { Loader2, RotateCcw, Save, SlidersHorizontal } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { apiGet, apiPost, apiPut } from "@/lib/api";
import type {
  PromptTemplateField,
  PromptTemplates,
  PromptTemplatesResponse,
} from "@/lib/types";
import { useAppStore } from "@/store/app-store";
import { confirmAction } from "@/store/confirm-store";
import { toast } from "@/store/toast-store";
import { cn } from "@/lib/utils";
import { smallLabelClass as labelClass } from "@/lib/ui-classes";

export function PromptTemplatesCard() {
  const workspaceId = useAppStore((state) => state.activeWorkspaceId);
  const [fields, setFields] = useState<PromptTemplateField[]>([]);
  const [templates, setTemplates] = useState<PromptTemplates>({});
  const [defaults, setDefaults] = useState<PromptTemplates>({});
  const [selected, setSelected] = useState<string>("permanent");
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const query = workspaceId
    ? `?workspace_id=${encodeURIComponent(workspaceId)}`
    : "";

  const apply = useCallback((value: PromptTemplatesResponse) => {
    setFields(value.fields);
    setTemplates(value.templates);
    setDefaults(value.defaults);
    setSelected((current) =>
      value.fields.some((field) => field.id === current)
        ? current
        : (value.fields[0]?.id ?? "permanent"),
    );
  }, []);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      apply(await apiGet<PromptTemplatesResponse>(`/api/prompt/templates${query}`));
      setError(null);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setLoading(false);
    }
  }, [apply, query]);

  useEffect(() => {
    void load();
  }, [load]);

  const field = fields.find((item) => item.id === selected) ?? null;

  async function save() {
    setSaving(true);
    try {
      const value = await apiPut<PromptTemplatesResponse>(
        `/api/prompt/templates${query}`,
        { workspace_id: workspaceId, templates },
      );
      apply(value);
      toast({
        tone: "success",
        title: "Prompts salvos",
        description: "As próximas execuções usam os textos atualizados.",
      });
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setSaving(false);
    }
  }

  async function reset() {
    if (
      !(await confirmAction({
        title: "Restaurar todos os prompts padrão?",
        description:
          "Os textos personalizados deste workspace serão descartados. O preview volta a usar os padrões do OpenDesk.",
        confirmLabel: "Restaurar padrão",
        tone: "destructive",
      }))
    ) {
      return;
    }
    setSaving(true);
    try {
      const value = await apiPost<PromptTemplatesResponse>(
        `/api/prompt/templates/reset${query}`,
        { workspace_id: workspaceId },
      );
      apply(value);
      toast({ tone: "success", title: "Prompts restaurados" });
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setSaving(false);
    }
  }

  return (
    <Card>
      <CardHeader>
        <div>
          <CardTitle className="flex items-center gap-2">
            <SlidersHorizontal size={15} className="text-zinc-500" />
            Prompts automáticos
          </CardTitle>
          <p className="mt-1 text-xs text-zinc-500">
            Edite cada bloco que o OpenDesk injeta antes de enviar a tarefa ao
            modelo. Placeholders como{" "}
            <span className="font-mono">{"{{output_directory}}"}</span> são
            substituídos na hora da execução.
          </p>
        </div>
        <div className="flex items-center gap-1.5">
          <Button variant="outline" size="sm" onClick={() => void reset()} disabled={saving}>
            <RotateCcw size={13} />
            Restaurar
          </Button>
          <Button size="sm" onClick={() => void save()} disabled={saving || loading}>
            {saving ? (
              <Loader2 size={13} className="animate-spin" />
            ) : (
              <Save size={13} />
            )}
            Salvar
          </Button>
        </div>
      </CardHeader>
      <CardContent className="space-y-3">
        {error ? (
          <p className="rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[12px] text-red-300">
            {error}
          </p>
        ) : null}
        {loading ? (
          <p className="flex items-center gap-2 text-[12px] text-zinc-500">
            <Loader2 size={12} className="animate-spin" />
            Carregando prompts…
          </p>
        ) : (
          <div className="grid gap-3 lg:grid-cols-[220px_minmax(0,1fr)]">
            <div className="scroll-area max-h-[420px] space-y-0.5 overflow-y-auto rounded-lg border border-zinc-800/70 bg-zinc-950/30 p-1">
              {fields.map((item) => (
                <button
                  key={item.id}
                  type="button"
                  onClick={() => setSelected(item.id)}
                  className={cn(
                    "w-full rounded-md px-2 py-1.5 text-left text-[12.5px] transition-colors",
                    selected === item.id
                      ? "bg-zinc-800 text-zinc-100"
                      : "text-zinc-400 hover:bg-zinc-900 hover:text-zinc-200",
                  )}
                >
                  {item.label}
                </button>
              ))}
            </div>
            {field ? (
              <div className="min-w-0 space-y-2">
                <div>
                  <p className="text-[13px] font-medium text-zinc-200">{field.label}</p>
                  <p className="text-[11.5px] text-zinc-500">{field.description}</p>
                </div>
                {field.placeholders.length > 0 ? (
                  <div className="flex flex-wrap gap-1">
                    {field.placeholders.map((placeholder) => (
                      <Badge key={placeholder} variant="default">
                        {`{{${placeholder}}}`}
                      </Badge>
                    ))}
                  </div>
                ) : null}
                <label>
                  <span className={labelClass}>Texto</span>
                  <textarea
                    value={templates[field.id] ?? defaults[field.id] ?? ""}
                    onChange={(event) =>
                      setTemplates((current) => ({
                        ...current,
                        [field.id]: event.target.value,
                      }))
                    }
                    rows={12}
                    className="w-full resize-y rounded-lg border border-zinc-800 bg-zinc-950/60 px-3 py-2 font-mono text-[12px] leading-relaxed text-zinc-200 outline-none placeholder:text-zinc-500 focus:border-blue-500/50"
                  />
                </label>
                <p className="text-[11.5px] text-zinc-500">
                  Use “Restaurar” para voltar aos padrões. O preview abaixo usa o
                  texto salvo, não o rascunho atual.
                </p>
              </div>
            ) : null}
          </div>
        )}
      </CardContent>
    </Card>
  );
}

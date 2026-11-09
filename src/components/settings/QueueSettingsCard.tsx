import { useEffect, useState } from "react";
import { ListChecks, Loader2, Save } from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Switch } from "@/components/ui/switch";
import { apiGet, apiPut } from "@/lib/api";
import { toast } from "@/store/toast-store";
import { smallInputClass, smallLabelClass } from "@/lib/ui-classes";
import { cn } from "@/lib/utils";

interface DedupeSettings {
  enabled: boolean;
  cooldown_minutes: number;
}

export function QueueSettingsCard() {
  const [settings, setSettings] = useState<DedupeSettings | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    apiGet<DedupeSettings>("/api/settings/dedupe")
      .then(setSettings)
      .catch((cause) =>
        toast({
          tone: "error",
          title: "Não foi possível carregar a fila",
          description: cause instanceof Error ? cause.message : String(cause),
        }),
      )
      .finally(() => setLoading(false));
  }, []);

  async function save() {
    if (!settings) return;
    setSaving(true);
    try {
      const saved = await apiPut<DedupeSettings>(
        "/api/settings/dedupe",
        settings,
      );
      setSettings(saved);
      toast({ tone: "success", title: "Preferências da fila salvas" });
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível salvar",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setSaving(false);
    }
  }

  return (
    <Card id="settings-queue">
      <CardHeader className="flex flex-row items-center justify-between gap-2 py-3">
        <CardTitle className="flex items-center gap-2 text-[13px]">
          <ListChecks size={14} className="text-zinc-400" />
          Fila e duplicatas
        </CardTitle>
        {settings ? (
          <Badge variant={settings.enabled ? "green" : "default"}>
            {settings.enabled ? "ativo" : "desligado"}
          </Badge>
        ) : null}
      </CardHeader>
      <CardContent className="space-y-4 p-4">
        <p className="text-[11px] leading-relaxed text-zinc-500">
          O OpenDesk compara título, descrição, modo, agente, modelo, esforço e
          tags. Uma tarefa idêntica ativa é reutilizada; depois de concluída, a
          carência abaixo evita repetições em rajada. Forks e swarms ignoram a
          regra.
        </p>
        {loading || !settings ? (
          <div className="flex items-center gap-2 text-[11px] text-zinc-500">
            <Loader2 size={13} className="animate-spin" />
            Carregando…
          </div>
        ) : (
          <>
            <div className="flex items-start justify-between gap-3 rounded-lg border border-zinc-800/70 bg-zinc-950/30 px-3 py-2.5">
              <div>
                <p className="text-[11px] font-medium text-zinc-300">
                  Evitar tarefas duplicadas
                </p>
                <p className="mt-0.5 text-[10px] text-zinc-600">
                  Vale para tarefas manuais, receitas, rotinas e entradas do I/O.
                </p>
              </div>
              <Switch
                checked={settings.enabled}
                onCheckedChange={(enabled) =>
                  setSettings({ ...settings, enabled })
                }
                label="Evitar tarefas duplicadas"
              />
            </div>
            <label>
              <span className={smallLabelClass}>
                Carência após conclusão (minutos)
              </span>
              <input
                type="number"
                min={0}
                max={1440}
                value={settings.cooldown_minutes}
                disabled={!settings.enabled}
                onChange={(event) =>
                  setSettings({
                    ...settings,
                    cooldown_minutes: Number(event.target.value) || 0,
                  })
                }
                className={cn(smallInputClass, "max-w-[160px] font-mono")}
                aria-label="Carência após conclusão em minutos"
              />
              <span className="mt-1 block text-[9px] text-zinc-600">
                0 desliga a carência. O padrão é 5 minutos.
              </span>
            </label>
            <div className="flex justify-end">
              <Button size="sm" onClick={() => void save()} disabled={saving}>
                {saving ? (
                  <Loader2 size={13} className="animate-spin" />
                ) : (
                  <Save size={13} />
                )}
                Salvar preferências
              </Button>
            </div>
          </>
        )}
      </CardContent>
    </Card>
  );
}

import { useEffect, useState } from "react";
import { ListChecks, Loader2, Save } from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Switch } from "@/components/ui/switch";
import { apiGet, apiPut } from "@/lib/api";
import { toast } from "@/store/toast-store";
import type { QueuePolicy } from "@/lib/types";
import { smallInputClass, smallLabelClass } from "@/lib/ui-classes";
import { cn } from "@/lib/utils";

interface DedupeSettings {
  enabled: boolean;
  cooldown_minutes: number;
}

export function QueueSettingsCard() {
  const [settings, setSettings] = useState<DedupeSettings | null>(null);
  const [policy, setPolicy] = useState<QueuePolicy | null>(null);
  const [autostart, setAutostart] = useState(true);
  const [missedPolicy, setMissedPolicy] = useState("ASK");
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    Promise.all([
      apiGet<DedupeSettings>("/api/settings/dedupe"),
      apiGet<boolean>("/api/settings/tasks_autostart").catch(() => true),
      apiGet<string>("/api/settings/missed_run_policy").catch(() => "ASK"),
      apiGet<QueuePolicy>("/api/queue/config"),
    ])
      .then(([dedupe, start, missed, queuePolicy]) => {
        setSettings(dedupe);
        setAutostart(start !== false);
        setMissedPolicy(
          ["ASK", "RUN", "SKIP"].includes(missed) ? missed : "ASK",
        );
        setPolicy(queuePolicy);
      })
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
      const [saved, , , savedPolicy] = await Promise.all([
        apiPut<DedupeSettings>("/api/settings/dedupe", settings),
        apiPut<boolean>("/api/settings/tasks_autostart", autostart),
        apiPut<string>("/api/settings/missed_run_policy", missedPolicy),
        policy
          ? apiPut<QueuePolicy>("/api/queue/config", policy)
          : Promise.resolve(null),
      ]);
      setSettings(saved);
      if (savedPolicy) setPolicy(savedPolicy);
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
        <CardTitle className="flex items-center gap-2 text-[14px]">
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
        <p className="text-[13px] leading-relaxed text-zinc-500">
          O OpenDesk compara título, descrição, modo, agente, modelo, esforço e
          tags. Uma tarefa idêntica ativa é reutilizada; depois de concluída, a
          carência abaixo evita repetições em rajada. Forks e swarms ignoram a
          regra.
        </p>
        {loading || !settings ? (
          <div className="flex items-center gap-2 text-[13px] text-zinc-500">
            <Loader2 size={13} className="animate-spin" />
            Carregando…
          </div>
        ) : (
          <>
            <div className="flex items-start justify-between gap-3 rounded-lg border border-zinc-800/70 bg-zinc-950/30 px-3 py-2.5">
              <div>
                <p className="text-[13px] font-medium text-zinc-300">
                  Evitar tarefas duplicadas
                </p>
                <p className="mt-0.5 text-[12px] text-zinc-500">
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
            <div className="flex items-start justify-between gap-3 rounded-lg border border-zinc-800/70 bg-zinc-950/30 px-3 py-2.5">
              <div>
                <p className="text-[13px] font-medium text-zinc-300">
                  Iniciar tarefas automaticamente
                </p>
                <p className="mt-0.5 text-[12px] text-zinc-500">
                  Tarefas novas e itens na Fila entram na execução assim que o
                  motor estiver online. Use “Em espera” para segurar uma tarefa.
                </p>
              </div>
              <Switch
                checked={autostart}
                onCheckedChange={setAutostart}
                label="Iniciar tarefas automaticamente"
              />
            </div>
            <label className="block">
              <span className={smallLabelClass}>Quando uma automação perder o horário</span>
              <select
                value={missedPolicy}
                onChange={(event) => setMissedPolicy(event.target.value)}
                className={cn(smallInputClass, "max-w-[280px]")}
                aria-label="Política para execuções perdidas"
              >
                <option value="ASK">Perguntar antes de executar</option>
                <option value="RUN">Executar automaticamente</option>
                <option value="SKIP">Ignorar e seguir o próximo horário</option>
              </select>
              <span className="mt-1 block text-[11.5px] text-zinc-500">
                Vale quando o app/engine estava offline ou o horário passou.
                O padrão é perguntar.
              </span>
            </label>
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
              <span className="mt-1 block text-[11.5px] text-zinc-500">
                0 desliga a carência. O padrão é 5 minutos.
              </span>
            </label>
            {policy ? (
              <div className="space-y-3 rounded-lg border border-zinc-800/70 bg-zinc-950/30 p-3">
                <div>
                  <p className="text-[13px] font-medium text-zinc-300">
                    Capacidade da fila
                  </p>
                  <p className="mt-0.5 text-[12px] text-zinc-500">
                    Slots globais, teto da fila e limites por grupo. Mudanças
                    valem imediatamente para a próxima reivindicação.
                  </p>
                </div>
                <div className="grid gap-3 sm:grid-cols-3">
                  <label>
                    <span className={smallLabelClass}>Concorrência máxima</span>
                    <input
                      type="number"
                      min={1}
                      max={32}
                      value={policy.max_concurrency}
                      onChange={(event) =>
                        setPolicy({
                          ...policy,
                          max_concurrency: Number(event.target.value) || 1,
                        })
                      }
                      className={cn(smallInputClass, "font-mono")}
                      aria-label="Concorrência máxima"
                    />
                  </label>
                  <label>
                    <span className={smallLabelClass}>Tamanho máximo da fila</span>
                    <input
                      type="number"
                      min={1}
                      max={10000}
                      value={policy.max_queue_size}
                      onChange={(event) =>
                        setPolicy({
                          ...policy,
                          max_queue_size: Number(event.target.value) || 1,
                        })
                      }
                      className={cn(smallInputClass, "font-mono")}
                      aria-label="Tamanho máximo da fila"
                    />
                  </label>
                  <label>
                    <span className={smallLabelClass}>Quota padrão por agente</span>
                    <input
                      type="number"
                      min={1}
                      max={32}
                      value={policy.default_agent_concurrency}
                      onChange={(event) =>
                        setPolicy({
                          ...policy,
                          default_agent_concurrency:
                            Number(event.target.value) || 1,
                        })
                      }
                      className={cn(smallInputClass, "font-mono")}
                      aria-label="Quota padrão por agente"
                    />
                  </label>
                </div>
                <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
                  {Object.keys(policy.group_limits)
                    .sort()
                    .map((group) => (
                      <label key={group}>
                        <span className={smallLabelClass}>{group}</span>
                        <input
                          type="number"
                          min={1}
                          max={32}
                          value={policy.group_limits[group] ?? 1}
                          onChange={(event) =>
                            setPolicy({
                              ...policy,
                              group_limits: {
                                ...policy.group_limits,
                                [group]: Number(event.target.value) || 1,
                              },
                            })
                          }
                          className={cn(smallInputClass, "font-mono")}
                          aria-label={`Limite do grupo ${group}`}
                        />
                      </label>
                    ))}
                </div>
              </div>
            ) : null}
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

import { useCallback, useEffect, useMemo, useState } from "react";
import {
  CircleDollarSign,
  Gauge,
  GitBranch,
  Loader2,
  Plus,
  RefreshCw,
  Route,
  Save,
  ShieldAlert,
  Trash2,
} from "lucide-react";
import { PageBody, PageHeader } from "@/components/layout/PageHeader";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Modal } from "@/components/ui/modal";
import { EmptyState } from "@/components/ui/empty-state";
import { useAppStore } from "@/store/app-store";
import { apiDelete, apiGet, apiPatch, apiPost, apiPut } from "@/lib/api";
import type { BudgetSettings, BudgetStatus, RouterRule } from "@/lib/types";
import { ROUTER_POLICY_LABELS } from "@/lib/types";
import { formatUsd } from "@/lib/format";
import { cn } from "@/lib/utils";

interface RouterConfig {
  workspace_id: string;
  policy: string;
  rules: RouterRule[];
  budget: BudgetStatus;
}

export function RouterPage() {
  const activeWorkspaceId = useAppStore((state) => state.activeWorkspaceId);
  const workspaces = useAppStore((state) => state.workspaces);
  const refresh = useAppStore((state) => state.refresh);
  const workspace = workspaces.find((item) => item.id === activeWorkspaceId);

  const [config, setConfig] = useState<RouterConfig | null>(null);
  const [settings, setSettings] = useState<BudgetSettings | null>(null);
  const [loading, setLoading] = useState(false);
  const [savingBudget, setSavingBudget] = useState(false);
  const [ruleOpen, setRuleOpen] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const query = useMemo(
    () => (activeWorkspaceId ? `?workspace_id=${encodeURIComponent(activeWorkspaceId)}` : ""),
    [activeWorkspaceId],
  );

  const load = useCallback(async () => {
    if (!activeWorkspaceId) {
      setConfig(null);
      setSettings(null);
      return;
    }
    setLoading(true);
    try {
      const [routerConfig, budgetSettings] = await Promise.all([
        apiGet<RouterConfig>(`/api/router/config${query}`),
        apiGet<BudgetSettings>("/api/budget"),
      ]);
      setConfig(routerConfig);
      setSettings(budgetSettings);
      setError(null);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setLoading(false);
    }
  }, [activeWorkspaceId, query]);

  useEffect(() => {
    void load();
  }, [load]);

  async function changePolicy(policy: string) {
    if (!activeWorkspaceId) return;
    setError(null);
    try {
      await apiPatch(`/api/workspaces/${activeWorkspaceId}`, {
        default_router_policy: policy,
      });
      await load();
      await refresh();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  }

  async function saveBudget() {
    if (!settings) return;
    setSavingBudget(true);
    setError(null);
    try {
      const updated = await apiPut<BudgetSettings>("/api/budget", {
        daily_limit_usd: settings.daily_limit_usd,
        monthly_limit_usd: settings.monthly_limit_usd,
        soft_alert_threshold_pct: settings.soft_alert_threshold_pct,
        hard_stop_enabled: settings.hard_stop_enabled,
        opencode_go_5h_token_limit: settings.opencode_go_5h_token_limit,
      });
      setSettings(updated);
      await load();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setSavingBudget(false);
    }
  }

  async function removeRule(id: string) {
    if (!window.confirm("Excluir esta regra?")) return;
    try {
      await apiDelete(`/api/router/rules/${id}`);
      await load();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  }

  if (!activeWorkspaceId) {
    return (
      <PageBody>
        <PageHeader eyebrow="governança" title="Roteador & Custos" />
        <EmptyState
          icon={Route}
          title="Nenhum workspace ativo"
          description="O roteador usa a política e as regras do workspace ativo."
        />
      </PageBody>
    );
  }

  const policy = config?.policy ?? workspace?.default_router_policy ?? "COST_BENEFIT";

  return (
    <PageBody>
      <PageHeader
        eyebrow="governança"
        title="Roteador & Custos"
        description={`${workspace?.name ?? "Workspace"} · política, regras condicionais e limites de gasto que pausam a fila.`}
        actions={
          <Button variant="outline" size="sm" onClick={() => void load()}>
            <RefreshCw size={13} className={loading ? "animate-spin" : undefined} />
            Atualizar
          </Button>
        }
      />

      {error ? (
        <p className="rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[11px] text-red-300">
          {error}
        </p>
      ) : null}

      <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-5">
        {Object.entries(ROUTER_POLICY_LABELS).map(([id, label]) => {
          const active = id === policy;
          return (
            <button
              key={id}
              type="button"
              onClick={() => void changePolicy(id)}
              className={cn(
                "rounded-xl border p-4 text-left transition-colors",
                active
                  ? "border-blue-500/40 bg-blue-500/[0.06]"
                  : "border-zinc-800/80 bg-zinc-900/25 hover:border-zinc-700",
              )}
            >
              <div className="flex items-center justify-between">
                <Gauge size={15} className={active ? "text-blue-400" : "text-zinc-600"} />
                {active ? <Badge variant="blue">atual</Badge> : null}
              </div>
              <p className="mt-3 text-[12px] font-medium text-zinc-200">{label}</p>
              <p className="mt-1 font-mono text-[9px] text-zinc-600">{id}</p>
            </button>
          );
        })}
      </div>

      <div className="grid gap-3 lg:grid-cols-3">
        <Card>
          <CardHeader>
            <div>
              <CardTitle className="flex items-center gap-2">
                <CircleDollarSign size={15} className="text-zinc-500" />
                Budget Guard
              </CardTitle>
              <p className="mt-1 text-xs text-zinc-500">
                {config?.budget.hard_stop ? "hard stop ativo" : "dentro do orçamento"}
              </p>
            </div>
            <Button size="sm" onClick={() => void saveBudget()} disabled={savingBudget || !settings}>
              {savingBudget ? <Loader2 size={14} className="animate-spin" /> : <Save size={14} />}
              Salvar
            </Button>
          </CardHeader>
          <CardContent className="space-y-3">
            <BudgetInput
              label="Limite diário (USD)"
              value={settings?.daily_limit_usd ?? 5}
              onChange={(value) =>
                settings && setSettings({ ...settings, daily_limit_usd: value })
              }
            />
            <BudgetInput
              label="Limite mensal (USD)"
              value={settings?.monthly_limit_usd ?? 50}
              onChange={(value) =>
                settings && setSettings({ ...settings, monthly_limit_usd: value })
              }
            />
            <BudgetInput
              label="Alerta soft (%)"
              value={settings?.soft_alert_threshold_pct ?? 80}
              onChange={(value) =>
                settings && setSettings({ ...settings, soft_alert_threshold_pct: value })
              }
            />
            <BudgetInput
              label="Limite 5h opencode-go (tokens)"
              value={settings?.opencode_go_5h_token_limit ?? 5_000_000}
              onChange={(value) =>
                settings &&
                setSettings({ ...settings, opencode_go_5h_token_limit: Math.round(value) })
              }
            />
            <label className="flex items-center gap-2 text-[11px] text-zinc-400">
              <input
                type="checkbox"
                checked={settings?.hard_stop_enabled ?? true}
                onChange={(event) =>
                  settings &&
                  setSettings({ ...settings, hard_stop_enabled: event.target.checked })
                }
                className="accent-blue-500"
              />
              hard stop habilitado
            </label>
            <div className="grid grid-cols-2 gap-2 rounded-lg border border-zinc-800/70 bg-zinc-950/40 p-2.5 text-[10px]">
              <span className="text-zinc-600">gasto hoje</span>
              <span className="text-right font-mono text-zinc-300">
                {formatUsd(config?.budget.daily_spent ?? 0)} /{" "}
                {formatUsd(config?.budget.daily_limit ?? 0)}
              </span>
              <span className="text-zinc-600">gasto no mês</span>
              <span className="text-right font-mono text-zinc-300">
                {formatUsd(config?.budget.monthly_spent ?? 0)} /{" "}
                {formatUsd(config?.budget.monthly_limit ?? 0)}
              </span>
              <span className="text-zinc-600">janela 5h</span>
              <span className="text-right font-mono text-zinc-300">
                {config?.budget.five_hour_tokens ?? 0} tokens
              </span>
            </div>
            {config?.budget.hard_stop ? (
              <p className="rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[10px] leading-relaxed text-red-300">
                Hard cap atingido: a fila é pausada e notificada. Ajuste o limite ou aguarde o
                próximo período.
              </p>
            ) : config?.budget.soft_alert ? (
              <p className="rounded-lg border border-amber-500/20 bg-amber-500/10 px-3 py-2 text-[10px] leading-relaxed text-amber-300">
                Alerta soft: o orçamento passou de {settings?.soft_alert_threshold_pct ?? 80}%.
              </p>
            ) : null}
          </CardContent>
        </Card>

        <Card className="lg:col-span-2">
          <CardHeader>
            <div>
              <CardTitle className="flex items-center gap-2">
                <GitBranch size={15} className="text-zinc-500" />
                Regras IF-THEN
              </CardTitle>
              <p className="mt-1 text-xs text-zinc-500">
                Avaliadas em ordem de prioridade; a primeira que casar define fase, agente, modelo
                e effort.
              </p>
            </div>
            <Button size="sm" onClick={() => setRuleOpen(true)}>
              <Plus size={14} />
              Nova regra
            </Button>
          </CardHeader>
          <CardContent>
            {!config || config.rules.length === 0 ? (
              <EmptyState
                icon={Route}
                title="Nenhuma regra customizada"
                description='Exemplo: {"field":"tags","op":"contains","value":"roteiro"} → build/@roteirista/glm-5.3/high.'
              />
            ) : (
              <div className="space-y-2">
                {config.rules.map((rule) => (
                  <div
                    key={rule.id}
                    className="flex flex-wrap items-center gap-2 rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2.5"
                  >
                    <Badge variant={rule.is_enabled ? "green" : "default"}>
                      {rule.is_enabled ? "on" : "off"}
                    </Badge>
                    <div className="min-w-0 flex-1">
                      <p className="text-[11px] font-medium text-zinc-200">{rule.name}</p>
                      <p className="truncate font-mono text-[9px] text-zinc-600">
                        {rule.condition_json}
                      </p>
                    </div>
                    <Badge variant="purple">{rule.target_mode}</Badge>
                    <Badge variant="default">@{rule.target_agent ?? "build"}</Badge>
                    <span className="font-mono text-[9px] text-zinc-600">
                      {rule.target_model} · {rule.target_effort}
                    </span>
                    <Button
                      variant="destructive"
                      size="sm"
                      className="h-6 px-1.5 text-[9px]"
                      onClick={() => void removeRule(rule.id)}
                    >
                      <Trash2 size={10} />
                    </Button>
                  </div>
                ))}
              </div>
            )}
          </CardContent>
        </Card>
      </div>

      <Card>
        <CardContent className="flex items-start gap-3 p-4">
          <ShieldAlert size={16} className="mt-0.5 text-amber-400/70" />
          <p className="text-[11px] leading-relaxed text-zinc-500">
            O Budget Guard consulta o SQLite antes de cada job. Hard cap pausa a fila; o alerta
            soft apenas notifica. A janela de 5h usa tokens de entrada + saída dos runs.
          </p>
        </CardContent>
      </Card>

      <CreateRuleModal
        open={ruleOpen}
        workspaceId={activeWorkspaceId}
        onClose={() => setRuleOpen(false)}
        onCreated={async () => {
          setRuleOpen(false);
          await load();
        }}
      />
    </PageBody>
  );
}

function BudgetInput({
  label,
  value,
  onChange,
}: {
  label: string;
  value: number;
  onChange: (value: number) => void;
}) {
  return (
    <label className="block">
      <span className="mb-1 block text-[9px] uppercase tracking-[0.12em] text-zinc-600">
        {label}
      </span>
      <input
        type="number"
        min={0}
        step={label.includes("tokens") ? 1000 : 0.01}
        value={value}
        onChange={(event) => onChange(Number(event.target.value))}
        className="h-8 w-full rounded-lg border border-zinc-800 bg-zinc-900/50 px-2.5 font-mono text-[11px] text-zinc-100 outline-none focus:border-blue-500/60"
      />
    </label>
  );
}

function CreateRuleModal({
  open,
  workspaceId,
  onClose,
  onCreated,
}: {
  open: boolean;
  workspaceId: string;
  onClose: () => void;
  onCreated: () => Promise<void>;
}) {
  const [name, setName] = useState("");
  const [condition, setCondition] = useState(
    '{"field":"tags","op":"contains","value":"roteiro"}',
  );
  const [mode, setMode] = useState("build");
  const [agent, setAgent] = useState("build");
  const [model, setModel] = useState("opencode-go/glm-5.3");
  const [effort, setEffort] = useState("high");
  const [priority, setPriority] = useState(10);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function submit() {
    if (!name.trim()) {
      setError("Informe o nome da regra.");
      return;
    }
    try {
      JSON.parse(condition);
    } catch (cause) {
      setError(`condition_json inválido: ${cause instanceof Error ? cause.message : cause}`);
      return;
    }
    setSaving(true);
    setError(null);
    try {
      await apiPost("/api/router/rules", {
        workspace_id: workspaceId,
        name: name.trim(),
        priority_order: priority,
        condition_json: condition,
        target_mode: mode,
        target_agent: agent.trim() || null,
        target_model: model.trim(),
        target_effort: effort,
        is_enabled: true,
      });
      setName("");
      await onCreated();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setSaving(false);
    }
  }

  return (
    <Modal
      open={open}
      onClose={onClose}
      title="Nova regra do roteador"
      description="A condição usa field/op/value. Combine com all/any para regras compostas."
      className="max-w-2xl"
    >
      <div className="grid gap-3 md:grid-cols-2">
        <label className="md:col-span-2">
          <span className={labelClass}>Nome</span>
          <input value={name} onChange={(event) => setName(event.target.value)} className={inputClass} />
        </label>
        <label className="md:col-span-2">
          <span className={labelClass}>Condição JSON</span>
          <textarea
            value={condition}
            onChange={(event) => setCondition(event.target.value)}
            rows={3}
            className={cn(inputClass, "h-auto resize-y py-2 font-mono text-[10px]")}
          />
        </label>
        <label>
          <span className={labelClass}>Prioridade</span>
          <input
            type="number"
            value={priority}
            onChange={(event) => setPriority(Number(event.target.value))}
            className={inputClass}
          />
        </label>
        <label>
          <span className={labelClass}>Modo alvo</span>
          <select value={mode} onChange={(event) => setMode(event.target.value)} className={inputClass}>
            <option value="plan">plan</option>
            <option value="build">build</option>
            <option value="plan_then_build">plan_then_build</option>
          </select>
        </label>
        <label>
          <span className={labelClass}>Agente</span>
          <input value={agent} onChange={(event) => setAgent(event.target.value)} className={inputClass} />
        </label>
        <label>
          <span className={labelClass}>Modelo</span>
          <input value={model} onChange={(event) => setModel(event.target.value)} className={inputClass} />
        </label>
        <label>
          <span className={labelClass}>Effort</span>
          <select value={effort} onChange={(event) => setEffort(event.target.value)} className={inputClass}>
            <option value="low">low</option>
            <option value="medium">medium</option>
            <option value="high">high</option>
          </select>
        </label>
      </div>
      {error ? (
        <p className="mt-3 rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[11px] text-red-300">
          {error}
        </p>
      ) : null}
      <div className="mt-4 flex justify-end gap-2 border-t border-zinc-800/70 pt-4">
        <Button variant="ghost" size="sm" onClick={onClose}>
          Cancelar
        </Button>
        <Button size="sm" onClick={() => void submit()} disabled={saving}>
          {saving ? <Loader2 size={14} className="animate-spin" /> : <Plus size={14} />}
          Criar regra
        </Button>
      </div>
    </Modal>
  );
}

const inputClass =
  "h-9 w-full rounded-lg border border-zinc-800 bg-zinc-900/50 px-2.5 text-[12px] text-zinc-100 outline-none placeholder:text-zinc-600 focus:border-blue-500/60";
const labelClass =
  "mb-1.5 block text-[10px] font-medium uppercase tracking-[0.12em] text-zinc-500";

import { useCallback, useEffect, useMemo, useState } from "react";
import { useSearchParams } from "react-router-dom";
import {
  ChartNoAxesCombined,
  ListChecks,
  RefreshCw,
  Route,
  ShieldAlert,
  SlidersHorizontal,
  Wallet,
} from "lucide-react";
import { PageBody, PageHeader } from "@/components/layout/PageHeader";
import { SectionTabs } from "@/components/ui/section-tabs";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { EmptyState } from "@/components/ui/empty-state";
import { RouterBudgetCard } from "@/components/router/RouterBudgetCard";
import { RouterCostCard } from "@/components/router/RouterCostCard";
import { RouterModelsCard } from "@/components/router/RouterModelsCard";
import { RouterPolicyPicker } from "@/components/router/RouterPolicyPicker";
import { RouterPolicyTestCard } from "@/components/router/RouterPolicyTestCard";
import { RouterRuleModal } from "@/components/router/RouterRuleModal";
import { RouterRulesCard } from "@/components/router/RouterRulesCard";
import type {
  RouterAnalysis,
  RouterConfig,
  RouterTestResult,
} from "@/components/router/router-types";
import { useAppStore } from "@/store/app-store";
import { apiDelete, apiGet, apiPatch, apiPost, apiPut } from "@/lib/api";
import type { BudgetSettings, RouterRule } from "@/lib/types";
import { ROUTER_POLICY_LABELS } from "@/lib/types";
import { confirmAction } from "@/store/confirm-store";

export function RouterPage() {
  const [params, setParams] = useSearchParams();
  const requestedTab = params.get("tab") ?? "models";
  const tab = ["models", "rules", "budget", "analysis"].includes(requestedTab)
    ? requestedTab
    : "models";
  function setTab(next: string) {
    setParams({ tab: next }, { replace: true });
  }
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
  const [routerModel, setRouterModel] = useState("");
  const [costBlend, setCostBlend] = useState(0.7);
  const [savingBlend, setSavingBlend] = useState(false);
  const [savingModel, setSavingModel] = useState(false);
  const [analysis, setAnalysis] = useState<RouterAnalysis | null>(null);
  const [analyzing, setAnalyzing] = useState(false);
  const [editingRule, setEditingRule] = useState<RouterRule | null>(null);
  const [testTitle, setTestTitle] = useState("Produza um relatório executivo");
  const [testDescription, setTestDescription] = useState(
    "Analise os dados e destaque riscos e próximos passos.",
  );
  const [testTags, setTestTags] = useState("relatorio");
  const [defaultModel, setDefaultModel] = useState("");
  const [manualModel, setManualModel] = useState("");
  const [savingModels, setSavingModels] = useState(false);
  const [policyResults, setPolicyResults] = useState<
    Record<string, RouterTestResult | undefined>
  >({});
  const [testingPolicy, setTestingPolicy] = useState<string | null>(null);

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
      setDefaultModel(routerConfig.default_model ?? "");
      setManualModel(routerConfig.manual_model ?? "");
      setRouterModel(routerConfig.router_model ?? "");
      setCostBlend(routerConfig.cost_blend ?? 0.7);
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
      await refresh();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setSavingBudget(false);
    }
  }

  async function saveRouterModel() {
    setSavingModel(true);
    setError(null);
    try {
      await apiPut("/api/router/model", { model: routerModel || null });
      await load();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setSavingModel(false);
    }
  }

  async function saveBlend() {
    setSavingBlend(true);
    setError(null);
    try {
      await apiPut("/api/router/blend", { blend: costBlend });
      await load();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setSavingBlend(false);
    }
  }

  async function analyze() {
    if (!activeWorkspaceId) return;
    setAnalyzing(true);
    setError(null);
    try {
      const result = await apiPost<RouterAnalysis>("/api/router/analyze", {
        workspace_id: activeWorkspaceId,
      });
      setAnalysis(result);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setAnalyzing(false);
    }
  }

  async function saveModels() {
    if (!activeWorkspaceId) return;
    setSavingModels(true);
    setError(null);
    try {
      await apiPut("/api/router/models", {
        workspace_id: activeWorkspaceId,
        default_model: defaultModel.trim() || null,
        manual_model: manualModel.trim() || null,
      });
      await load();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setSavingModels(false);
    }
  }

  async function testPolicy(policyId: string) {
    if (!activeWorkspaceId) return;
    setTestingPolicy(policyId);
    setError(null);
    try {
      const result = await apiPost<RouterTestResult>("/api/router/test", {
        workspace_id: activeWorkspaceId,
        policy: policyId,
        default_model: defaultModel.trim() || null,
        manual_model: manualModel.trim() || null,
        title: testTitle,
        description: testDescription,
        tags: testTags
          .split(",")
          .map((tag) => tag.trim())
          .filter(Boolean),
      });
      setPolicyResults((current) => ({ ...current, [policyId]: result }));
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setTestingPolicy(null);
    }
  }

  async function testAllPolicies() {
    if (!activeWorkspaceId) return;
    setError(null);
    for (const policyId of Object.keys(ROUTER_POLICY_LABELS)) {
      await testPolicy(policyId);
    }
  }

  function editRule(rule: RouterRule) {
    setEditingRule(rule);
    setRuleOpen(true);
  }

  async function removeRule(id: string) {
    if (!(await confirmAction({ title: "Excluir regra do roteador?", description: "A regra deixa de ser avaliada imediatamente.", confirmLabel: "Excluir regra", tone: "destructive" }))) return;
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
        <PageHeader title="Roteador & Custos" />
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
        <p className="rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[13px] text-red-300">
          {error}
        </p>
      ) : null}

      <SectionTabs
        value={tab}
        onChange={setTab}
        items={[
          { id: "models", label: "Políticas & Modelos", icon: SlidersHorizontal },
          { id: "rules", label: "Regras", icon: ListChecks },
          { id: "budget", label: "Budget Guard", icon: Wallet },
          { id: "analysis", label: "Análise de Custos", icon: ChartNoAxesCombined },
        ]}
      />

      {tab === "models" ? (
        <>
          <RouterPolicyPicker policy={policy} onChange={(id) => void changePolicy(id)} />
          <RouterModelsCard
            defaultModel={defaultModel}
            manualModel={manualModel}
            saving={savingModels}
            onDefaultModelChange={(value) => {
              setDefaultModel(value);
              setPolicyResults({});
            }}
            onManualModelChange={(value) => {
              setManualModel(value);
              setPolicyResults({});
            }}
            onSave={() => void saveModels()}
          />
          <RouterPolicyTestCard
            policy={policy}
            workspaceId={activeWorkspaceId}
            title={testTitle}
            description={testDescription}
            tags={testTags}
            results={policyResults}
            testingPolicy={testingPolicy}
            onTitleChange={setTestTitle}
            onDescriptionChange={setTestDescription}
            onTagsChange={setTestTags}
            onTest={(id) => void testPolicy(id)}
            onTestAll={() => void testAllPolicies()}
          />
          {policy === "MANUAL" ? (
            <Card>
              <CardContent className="flex items-start gap-3 p-4 text-[13px] text-zinc-400">
                <ShieldAlert size={16} className="mt-0.5 text-amber-300" />
                <p>
                  Em <strong>Manual</strong>, o roteador usa o modelo do modo Manual
                  salvo neste workspace. Se a tarefa tiver um modelo próprio, ele
                  vence. Sem modelo manual e sem override, o roteador usa o modelo
                  padrão do OpenDesk.
                </p>
              </CardContent>
            </Card>
          ) : null}
        </>
      ) : null}

      {tab === "rules" ? (
        <RouterRulesCard
          config={config}
          onNew={() => setRuleOpen(true)}
          onEdit={editRule}
          onRemove={(id) => void removeRule(id)}
        />
      ) : null}

      {tab === "budget" ? (
        <>
          <RouterBudgetCard
            config={config}
            settings={settings}
            saving={savingBudget}
            onSettingsChange={setSettings}
            onSave={() => void saveBudget()}
          />
          <Card>
            <CardContent className="flex items-start gap-3 p-4">
              <ShieldAlert size={16} className="mt-0.5 text-amber-400/70" />
              <p className="text-[13px] leading-relaxed text-zinc-500">
                O Budget Guard consulta o SQLite antes de cada job. Hard cap pausa a
                fila; o alerta soft apenas notifica. A janela de 5h usa tokens de
                entrada + saída dos runs.
              </p>
            </CardContent>
          </Card>
        </>
      ) : null}

      {tab === "analysis" ? (
        <RouterCostCard
          routerModel={routerModel}
          savingModel={savingModel}
          costBlend={costBlend}
          savingBlend={savingBlend}
          analysis={analysis}
          analyzing={analyzing}
          onModelChange={setRouterModel}
          onSaveModel={() => void saveRouterModel()}
          onBlendChange={setCostBlend}
          onSaveBlend={() => void saveBlend()}
          onAnalyze={() => void analyze()}
        />
      ) : null}

      <RouterRuleModal
        open={ruleOpen}
        workspaceId={activeWorkspaceId}
        rule={editingRule}
        onClose={() => {
          setRuleOpen(false);
          setEditingRule(null);
        }}
        onCreated={async () => {
          setRuleOpen(false);
          setEditingRule(null);
          await load();
        }}
      />
    </PageBody>
  );
}

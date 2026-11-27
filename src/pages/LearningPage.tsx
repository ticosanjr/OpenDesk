import { useCallback, useEffect, useMemo, useState } from "react";
import {
  Activity,
  CheckCircle2,
  Lightbulb,
  RefreshCw,
  ScanLine,
  Sparkles,
  Brain,
} from "lucide-react";
import { useSearchParams } from "react-router-dom";
import { PageBody, PageHeader } from "@/components/layout/PageHeader";
import { SectionTabs } from "@/components/ui/section-tabs";
import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/ui/empty-state";
import { LoadingState } from "@/components/ui/loading-state";
import { apiGet, apiPost, apiPut } from "@/lib/api";
import { formatSuccessRate } from "@/lib/learning";
import type {
  LearningLesson,
  LearningOverview,
  LearningProposal,
  LearningScanResult,
  LearningSettings,
} from "@/lib/types";
import { PersonalContextCard } from "@/components/learning/PersonalContextCard";
import { SkillEvalCard } from "@/components/learning/SkillEvalCard";
import { StructuredMemoryCard } from "@/components/learning/StructuredMemoryCard";
import { useAppStore } from "@/store/app-store";
import { useUiStore } from "@/store/ui-store";
import { toast } from "@/store/toast-store";
import { LearningGuidanceCard } from "@/pages/learning/LearningGuidanceCard";
import { LearningProposalsSection } from "@/pages/learning/LearningProposalsSection";
import { LearningSettingsCard } from "@/pages/learning/LearningSettingsCard";
import { LessonCard } from "@/pages/learning/LessonCard";
import { LessonsToolbar, type LessonFilter } from "@/pages/learning/LessonsToolbar";
import { StatCard } from "@/pages/learning/StatCard";

export function LearningPage() {
  const [params, setParams] = useSearchParams();
  const requestedTab = params.get("tab") ?? "lessons";
  const tab = ["lessons", "context", "memory", "eval"].includes(requestedTab)
    ? requestedTab
    : requestedTab === "proposals"
      ? "eval"
      : "lessons";
  function setTab(next: string) {
    setParams({ tab: next }, { replace: true });
  }
  const activeWorkspaceId = useAppStore((state) => state.activeWorkspaceId);
  const workspaces = useAppStore((state) => state.workspaces);
  const lastRunEvent = useAppStore((state) => state.lastRunEvent);
  const lastProposalEvent = useAppStore((state) => state.lastProposalEvent);
  const setWorkspaceDialogOpen = useUiStore(
    (state) => state.setWorkspaceDialogOpen,
  );
  const workspace = workspaces.find((item) => item.id === activeWorkspaceId);

  const [data, setData] = useState<LearningOverview | null>(null);
  const [loading, setLoading] = useState(false);
  const [scanning, setScanning] = useState(false);
  const [savingSettings, setSavingSettings] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [filter, setFilter] = useState<LessonFilter>("OPEN");

  const query = useMemo(
    () =>
      activeWorkspaceId
        ? `?workspace_id=${encodeURIComponent(activeWorkspaceId)}`
        : "",
    [activeWorkspaceId],
  );

  const load = useCallback(async () => {
    if (!activeWorkspaceId) {
      setData(null);
      return;
    }
    setLoading(true);
    try {
      const overview = await apiGet<LearningOverview>(`/api/learning${query}`);
      setData(overview);
      setError(null);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setLoading(false);
    }
  }, [activeWorkspaceId, query]);

  useEffect(() => {
    void load();
  }, [load, lastRunEvent?.run_id, lastRunEvent?.status, lastProposalEvent]);

  async function saveSettings(next: LearningSettings) {
    if (!activeWorkspaceId) return;
    setSavingSettings(true);
    try {
      const saved = await apiPut<LearningSettings>(
        `/api/learning/settings${query}`,
        next,
      );
      setData((current) => (current ? { ...current, settings: saved } : current));
      toast({ tone: "success", title: "Preferências de aprendizado salvas" });
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível salvar",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setSavingSettings(false);
    }
  }

  async function scan() {
    if (!activeWorkspaceId) return;
    setScanning(true);
    try {
      const result = await apiPost<LearningScanResult>(
        `/api/learning/scan${query}`,
      );
      await load();
      toast({
        tone: result.lessons > 0 ? "success" : "info",
        title:
          result.lessons > 0
            ? `${result.lessons} lição(ões) atualizada(s)`
            : "Nenhuma novidade nas execuções recentes",
        description: `${result.analyzed} execução(ões) analisada(s).`,
      });
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível analisar as execuções",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setScanning(false);
    }
  }

  async function promote(lesson: LearningLesson) {
    setBusy(`promote-${lesson.id}`);
    try {
      await apiPost(`/api/learning/lessons/${lesson.id}/promote${query}`);
      await load();
      toast({
        tone: "success",
        title: "Proposta criada",
        description:
          "Revise e aprove abaixo. Nada é gravado no workspace sem aprovação.",
      });
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível promover a lição",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setBusy(null);
    }
  }

  async function rememberLesson(lesson: LearningLesson) {
    setBusy(`remember-${lesson.id}`);
    try {
      await apiPost(`/api/learning/lessons/${lesson.id}/remember${query}`);
      await load();
      toast({
        tone: "success",
        title: "Lição adicionada à MEMORY.md",
        description: "Ela passa a entrar no contexto das próximas execuções.",
      });
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível memorizar a lição",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setBusy(null);
    }
  }

  async function dismiss(lesson: LearningLesson) {
    setBusy(`dismiss-${lesson.id}`);
    try {
      await apiPost(`/api/learning/lessons/${lesson.id}/dismiss${query}`);
      await load();
      toast({ tone: "info", title: "Lição descartada" });
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível descartar",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setBusy(null);
    }
  }

  async function restore(lesson: LearningLesson) {
    setBusy(`restore-${lesson.id}`);
    try {
      await apiPost(`/api/learning/lessons/${lesson.id}/restore${query}`);
      await load();
      toast({
        tone: "success",
        title: "Lição reaberta",
        description: "Ela voltou para a lista de lições abertas.",
      });
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível reabrir",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setBusy(null);
    }
  }

  async function forget(lesson: LearningLesson) {
    setBusy(`forget-${lesson.id}`);
    try {
      await apiPost(`/api/learning/lessons/${lesson.id}/forget${query}`);
      await load();
      toast({
        tone: "success",
        title: "Removida da memória",
        description: "O bloco foi retirado de .opencode/MEMORY.md.",
      });
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível remover da memória",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setBusy(null);
    }
  }

  async function resolveProposal(proposal: LearningProposal, approved: boolean) {
    setBusy(`proposal-${proposal.id}`);
    try {
      await apiPost(`/api/proposals/${proposal.id}/resolve`, { approved });
      await load();
      toast({
        tone: approved ? "success" : "info",
        title: approved ? "Skill aplicada ao workspace" : "Proposta rejeitada",
      });
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível resolver a proposta",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setBusy(null);
    }
  }

  if (!activeWorkspaceId) {
    return (
      <PageBody>
        <PageHeader title="Aprendizado" />
        <EmptyState
          icon={Brain}
          title="Nenhum workspace ativo"
          description="Selecione um workspace para acumular lições das execuções."
          action={
            <Button size="sm" onClick={() => setWorkspaceDialogOpen(true)}>
              Criar workspace
            </Button>
          }
        />
      </PageBody>
    );
  }

  const settings = data?.settings;
  const stats = data?.stats;
  const lessons = data?.lessons ?? [];
  const filteredLessons =
    filter === "ALL"
      ? lessons
      : filter === "MEMORIZED"
        ? lessons.filter((lesson) => Boolean(lesson.remembered_at))
        : lessons.filter((lesson) => lesson.status === filter);
  const learningProposals = (data?.proposals ?? []).filter(
    (proposal) => proposal.requested_by === "learning",
  );

  return (
    <PageBody>
      <PageHeader
        title="Aprendizado"
        description={`${workspace?.name ?? "Workspace"} · lições extraídas de execuções reais. Nada vira skill sem sua aprovação.`}
        actions={
          <>
            <Button
              variant="outline"
              size="sm"
              onClick={() => void load()}
              disabled={loading}
            >
              <RefreshCw size={13} className={loading ? "animate-spin" : undefined} />
              Atualizar
            </Button>
            <Button size="sm" onClick={() => void scan()} disabled={scanning}>
              <ScanLine size={13} className={scanning ? "animate-pulse" : undefined} />
              {scanning ? "Analisando…" : "Analisar execuções"}
            </Button>
          </>
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
          { id: "lessons", label: "Lições & Loop", icon: Brain },
          { id: "context", label: "Contexto Pessoal", icon: Sparkles },
          { id: "memory", label: "Memória Estruturada", icon: Activity },
          { id: "eval", label: "Avaliação de Skills", icon: CheckCircle2 },
        ]}
      />

      {tab === "lessons" && stats ? (
        <div className="grid grid-cols-2 gap-2 lg:grid-cols-4">
          <StatCard
            icon={<Lightbulb size={13} />}
            label="Lições abertas"
            value={String(stats.open)}
            hint={`${stats.total} no total`}
          />
          <StatCard
            icon={<Activity size={13} />}
            label="Execuções · 30 dias"
            value={String(stats.runs_analyzed)}
            hint={`${stats.failed_runs} falha(s)`}
          />
          <StatCard
            icon={<CheckCircle2 size={13} />}
            label="Taxa de sucesso"
            value={formatSuccessRate(stats.success_rate)}
            hint="execuções concluídas"
          />
          <StatCard
            icon={<Sparkles size={13} />}
            label="Promovidas"
            value={String(stats.promoted)}
            hint={`${stats.dismissed} descartada(s)`}
          />
        </div>
      ) : null}

      {tab === "context" ? <PersonalContextCard /> : null}

      {tab === "memory" && activeWorkspaceId ? (
        <StructuredMemoryCard workspaceId={activeWorkspaceId} />
      ) : null}

      {tab === "eval" && activeWorkspaceId ? (
        <div className="space-y-4">
          <SkillEvalCard workspaceId={activeWorkspaceId} />
          {learningProposals.length > 0 ? (
            <LearningProposalsSection
              proposals={learningProposals}
              busy={busy}
              workspaceId={activeWorkspaceId}
              onResolve={(proposal, approved) =>
                void resolveProposal(proposal, approved)
              }
            />
          ) : null}
        </div>
      ) : null}

      {tab === "lessons" ? (
        <>
          {settings ? (
            <LearningSettingsCard
              settings={settings}
              savingSettings={savingSettings}
              onSave={(next) => void saveSettings(next)}
            />
          ) : null}

          <LearningGuidanceCard />

          <section className="space-y-2">
            <LessonsToolbar filter={filter} onFilterChange={setFilter} />

            {loading && !data ? (
              <LoadingState rows={3} label="Carregando lições…" />
            ) : filteredLessons.length === 0 ? (
              <EmptyState
                icon={Brain}
                title="Nenhuma lição nesta visão"
                description="Rode “Analisar execuções” para extrair padrões das execuções recentes. Falhas repetidas viram lições; você decide o que promover."
              />
            ) : (
              <div className="space-y-1.5">
                {filteredLessons.map((lesson) => (
                  <LessonCard
                    key={lesson.id}
                    lesson={lesson}
                    settings={settings}
                    busy={busy}
                    workspaceId={activeWorkspaceId}
                    onPromote={() => void promote(lesson)}
                    onRemember={() => void rememberLesson(lesson)}
                    onForget={() => void forget(lesson)}
                    onRestore={() => void restore(lesson)}
                    onDismiss={() => void dismiss(lesson)}
                  />
                ))}
              </div>
            )}
          </section>
        </>
      ) : null}
    </PageBody>
  );
}

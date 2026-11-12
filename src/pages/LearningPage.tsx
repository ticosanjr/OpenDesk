import type { ReactNode } from "react";
import { useCallback, useEffect, useMemo, useState } from "react";
import {
  Activity,
  AlertTriangle,
  ArrowUpRight,
  BookmarkPlus,
  Brain,
  CheckCircle2,
  Eye,
  EyeOff,
  Lightbulb,
  RefreshCw,
  RotateCcw,
  ScanLine,
  Sparkles,
  Trash2,
  XCircle,
} from "lucide-react";
import { Link } from "react-router-dom";
import { PageBody, PageHeader } from "@/components/layout/PageHeader";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { EmptyState } from "@/components/ui/empty-state";
import { LoadingState } from "@/components/ui/loading-state";
import { Switch } from "@/components/ui/switch";
import { apiGet, apiPost, apiPut } from "@/lib/api";
import { relativeTime } from "@/lib/format";
import {
  canPromote,
  canRemember,
  formatSuccessRate,
  lessonCategoryLabel,
  lessonStatusLabel,
  lessonStatusTone,
  occurrencesLabel,
  proposalSkillId,
  readyToPromote,
} from "@/lib/learning";
import { links } from "@/lib/links";
import type {
  LearningLesson,
  LearningOverview,
  LearningProposal,
  LearningScanResult,
  LearningSettings,
} from "@/lib/types";
import { smallInputClass, smallLabelClass } from "@/lib/ui-classes";
import { useAppStore } from "@/store/app-store";
import { useUiStore } from "@/store/ui-store";
import { toast } from "@/store/toast-store";
import { cn } from "@/lib/utils";

type LessonFilter = "OPEN" | "PROMOTED" | "MEMORIZED" | "DISMISSED" | "ALL";

const FILTERS: Array<{ id: LessonFilter; label: string }> = [
  { id: "OPEN", label: "Abertas" },
  { id: "PROMOTED", label: "Promovidas" },
  { id: "MEMORIZED", label: "Memorizadas" },
  { id: "DISMISSED", label: "Descartadas" },
  { id: "ALL", label: "Todas" },
];

export function LearningPage() {
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
        <p className="rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[11px] text-red-300">
          {error}
        </p>
      ) : null}

      {stats ? (
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

      {settings ? (
        <Card>
          <CardContent className="space-y-3 p-4">
            <div className="flex flex-wrap items-start justify-between gap-3">
              <div>
                <p className="text-[12px] font-medium text-zinc-200">
                  Captura leve e controlada
                </p>
                <p className="mt-0.5 max-w-2xl text-[10px] leading-relaxed text-zinc-500">
                  O OpenDesk observa o resultado das execuções e agrupa padrões
                  repetidos. Sem embeddings, sem pipeline pesado e sem prompts
                  completos.
                </p>
              </div>
              {savingSettings ? (
                <Badge variant="default">salvando…</Badge>
              ) : null}
            </div>
            <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
              <SettingToggle
                label="Ativar aprendizado"
                description="Observa execuções concluídas."
                checked={settings.enabled}
                onCheckedChange={(enabled) =>
                  void saveSettings({ ...settings, enabled })
                }
              />
              <SettingToggle
                label="Capturar sucessos"
                description="Também aprende com execuções bem-sucedidas."
                checked={settings.capture_successes}
                disabled={!settings.enabled}
                onCheckedChange={(capture_successes) =>
                  void saveSettings({ ...settings, capture_successes })
                }
              />
              <SettingToggle
                label="Propor automaticamente"
                description="Cria proposta ao atingir o mínimo."
                checked={settings.auto_propose}
                disabled={!settings.enabled}
                onCheckedChange={(auto_propose) =>
                  void saveSettings({ ...settings, auto_propose })
                }
              />
              <label>
                <span className={smallLabelClass}>Mínimo de ocorrências</span>
                <input
                  type="number"
                  min={1}
                  max={20}
                  value={settings.min_occurrences}
                  disabled={!settings.enabled || savingSettings}
                  onChange={(event) =>
                    void saveSettings({
                      ...settings,
                      min_occurrences: Number(event.target.value) || 1,
                    })
                  }
                  className={cn(smallInputClass, "font-mono")}
                  aria-label="Mínimo de ocorrências"
                />
              </label>
            </div>
          </CardContent>
        </Card>
      ) : null}

      <Card className="border-violet-500/20 bg-violet-500/[0.025]">
        <CardContent className="grid gap-3 p-4 text-[10px] leading-relaxed text-zinc-400 sm:grid-cols-2">
          <div>
            <p className="text-[11px] font-medium text-violet-200">
              Promover = criar uma proposta
            </p>
            <p className="mt-1">
              Gera uma proposta de skill no Studio. Nada é gravado no workspace
              até você revisar e aprovar a proposta.
            </p>
          </div>
          <div>
            <p className="text-[11px] font-medium text-emerald-200">
              Memorizar = salvar na memória
            </p>
            <p className="mt-1">
              Adiciona um bloco em <span className="font-mono">.opencode/MEMORY.md</span>.
              Esse conteúdo entra no contexto das próximas execuções e pode ser
              removido depois pelo botão “Remover da memória”.
            </p>
          </div>
        </CardContent>
      </Card>

      {learningProposals.length > 0 ? (
        <section className="space-y-2">
          <div className="flex items-center gap-2">
            <Sparkles size={13} className="text-violet-300" />
            <h2 className="text-[12px] font-medium text-zinc-200">
              Propostas aguardando aprovação
            </h2>
            <Badge variant="purple">{learningProposals.length}</Badge>
          </div>
          <div className="space-y-1.5">
            {learningProposals.map((proposal) => {
              const skillId = proposalSkillId(proposal);
              return (
                <Card key={proposal.id} className="border-violet-500/20">
                  <CardContent className="flex flex-wrap items-center gap-3 p-3">
                    <div className="min-w-0 flex-1">
                      <div className="flex flex-wrap items-center gap-1.5">
                        <Badge variant="purple">{proposal.kind}</Badge>
                        <Badge variant="default">{proposal.action}</Badge>
                        <p className="truncate text-[11px] text-zinc-200">
                          {proposal.title}
                        </p>
                      </div>
                      <p className="mt-1 text-[9px] text-zinc-500">
                        {proposal.description ??
                          "Revise antes de gravar no workspace."}
                      </p>
                    </div>
                    <div className="flex items-center gap-1.5">
                      {skillId ? (
                        <Link
                          to={links.skill(skillId, "workspace", activeWorkspaceId)}
                          className="inline-flex h-8 items-center gap-1 rounded-lg border border-zinc-800 px-2.5 text-[10px] text-zinc-300 hover:bg-zinc-900"
                        >
                          Ver skill
                          <ArrowUpRight size={11} />
                        </Link>
                      ) : null}
                      <Button
                        size="sm"
                        disabled={busy === `proposal-${proposal.id}`}
                        onClick={() => void resolveProposal(proposal, true)}
                      >
                        <CheckCircle2 size={12} />
                        Aprovar
                      </Button>
                      <Button
                        size="sm"
                        variant="outline"
                        className="border-red-500/30 text-red-300"
                        disabled={busy === `proposal-${proposal.id}`}
                        onClick={() => void resolveProposal(proposal, false)}
                      >
                        <XCircle size={12} />
                        Rejeitar
                      </Button>
                    </div>
                  </CardContent>
                </Card>
              );
            })}
          </div>
        </section>
      ) : null}

      <section className="space-y-2">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <div className="flex items-center gap-2">
            <Brain size={13} className="text-zinc-400" />
            <h2 className="text-[12px] font-medium text-zinc-200">Lições</h2>
          </div>
          <div className="flex flex-wrap gap-1">
            {FILTERS.map((option) => (
              <button
                key={option.id}
                type="button"
                aria-pressed={filter === option.id}
                onClick={() => setFilter(option.id)}
                className={cn(
                  "rounded-md px-2 py-1 text-[10px] transition-colors",
                  filter === option.id
                    ? "bg-zinc-800 text-zinc-100"
                    : "text-zinc-500 hover:text-zinc-200",
                )}
              >
                {option.label}
              </button>
            ))}
          </div>
        </div>

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
    </PageBody>
  );
}

function StatCard({
  icon,
  label,
  value,
  hint,
}: {
  icon: ReactNode;
  label: string;
  value: string;
  hint: string;
}) {
  return (
    <Card>
      <CardContent className="p-3">
        <div className="flex items-center gap-1.5 text-zinc-500">
          {icon}
          <span className="text-[9px] uppercase tracking-[0.12em]">{label}</span>
        </div>
        <p className="mt-1 font-mono text-lg text-zinc-100">{value}</p>
        <p className="text-[9px] text-zinc-500">{hint}</p>
      </CardContent>
    </Card>
  );
}

function SettingToggle({
  label,
  description,
  checked,
  disabled = false,
  onCheckedChange,
}: {
  label: string;
  description: string;
  checked: boolean;
  disabled?: boolean;
  onCheckedChange: (checked: boolean) => void;
}) {
  return (
    <div className="flex items-start justify-between gap-2 rounded-lg border border-zinc-800/70 bg-zinc-950/30 px-2.5 py-2">
      <div className="min-w-0">
        <p className="text-[10px] font-medium text-zinc-300">{label}</p>
        <p className="mt-0.5 text-[9px] leading-snug text-zinc-500">
          {description}
        </p>
      </div>
      <Switch
        checked={checked}
        onCheckedChange={onCheckedChange}
        disabled={disabled}
        label={label}
      />
    </div>
  );
}

function LessonCard({
  lesson,
  settings,
  busy,
  workspaceId,
  onPromote,
  onRemember,
  onForget,
  onRestore,
  onDismiss,
}: {
  lesson: LearningLesson;
  settings?: LearningSettings;
  busy: string | null;
  workspaceId: string;
  onPromote: () => void;
  onRemember: () => void;
  onForget: () => void;
  onRestore: () => void;
  onDismiss: () => void;
}) {
  const promoted = lesson.status === "PROMOTED";
  const promotable = settings ? canPromote(lesson, settings) : false;
  const ready = settings ? readyToPromote(lesson, settings) : false;
  const rememberable = canRemember(lesson);
  const remembered = Boolean(lesson.remembered_at);
  return (
    <Card>
      <CardContent className="p-3">
        <div className="flex flex-wrap items-start gap-2">
          <div className="mt-0.5 text-amber-300">
            {lesson.category === "failure_pattern" ? (
              <AlertTriangle size={13} />
            ) : (
              <CheckCircle2 size={13} />
            )}
          </div>
          <div className="min-w-0 flex-1">
            <div className="flex flex-wrap items-center gap-1.5">
              <p className="text-[11px] font-medium text-zinc-200">
                {lesson.title}
              </p>
              <Badge variant={lessonStatusTone(lesson.status)}>
                {lessonStatusLabel(lesson.status)}
              </Badge>
              <Badge variant="default">
                {lessonCategoryLabel(lesson.category)}
              </Badge>
              <span className="font-mono text-[9px] text-zinc-500">
                {occurrencesLabel(lesson.occurrences)}
              </span>
              {promotable && !ready ? (
                <span className="text-[9px] text-zinc-500">
                  abaixo do mínimo
                </span>
              ) : null}
            </div>
            <p className="mt-1 whitespace-pre-line text-[10px] leading-relaxed text-zinc-500 line-clamp-3">
              {lesson.detail}
            </p>
            <div className="mt-1.5 flex flex-wrap items-center gap-2 text-[9px] text-zinc-500">
              <span>
                {lesson.agent_name ?? "router"}
                {lesson.model_id ? ` · ${lesson.model_id}` : ""}
              </span>
              <span>· {relativeTime(lesson.last_seen_at)}</span>
              {lesson.run_id ? (
                <Link
                  to={links.run(lesson.run_id)}
                  className="inline-flex items-center gap-0.5 text-zinc-500 hover:text-zinc-300"
                >
                  <Eye size={9} /> run
                </Link>
              ) : null}
            </div>
          </div>
          <div className="flex shrink-0 items-center gap-1.5">
            {promotable ? (
              <Button
                size="sm"
                variant={ready ? "default" : "outline"}
                disabled={busy === `promote-${lesson.id}`}
                onClick={onPromote}
                title="Criar uma proposta de skill no Studio; nada é gravado no workspace até a aprovação"
              >
                <Sparkles size={11} />
                {ready ? "Criar proposta" : "Criar proposta mesmo assim"}
              </Button>
            ) : null}
            {promoted && lesson.proposal_id ? (
              <Badge variant="purple">proposta criada</Badge>
            ) : null}
            {rememberable ? (
              <Button
                size="sm"
                variant="outline"
                disabled={busy === `remember-${lesson.id}`}
                onClick={onRemember}
                title="Adicionar um bloco em .opencode/MEMORY.md; entra no contexto dos próximos runs e pode ser removido depois"
              >
                <BookmarkPlus size={11} />
                Salvar na memória
              </Button>
            ) : remembered ? (
              <>
                <Link
                  to={links.file(".opencode/MEMORY.md")}
                  title="Abrir .opencode/MEMORY.md"
                  className="inline-flex items-center gap-1 rounded-md border border-emerald-500/20 bg-emerald-500/10 px-1.5 py-0.5 font-mono text-[9px] uppercase tracking-wide text-emerald-300 hover:text-emerald-200"
                >
                  <BookmarkPlus size={10} />
                  na memória
                </Link>
                <Button
                  size="sm"
                  variant="outline"
                  className="h-7 text-[10px]"
                  disabled={busy === `forget-${lesson.id}`}
                  onClick={onForget}
                  title="Remover este bloco de .opencode/MEMORY.md"
                >
                  Remover da memória
                </Button>
              </>
            ) : null}
            {lesson.status === "DISMISSED" ? (
              <Button
                size="sm"
                variant="outline"
                className="h-7 text-[10px]"
                disabled={busy === `restore-${lesson.id}`}
                onClick={onRestore}
                title="Voltar esta lição para a lista de abertas"
              >
                <RotateCcw size={11} />
                Reabrir
              </Button>
            ) : null}
            {lesson.status === "OPEN" ? (
              <Button
                size="sm"
                variant="ghost"
                className="text-zinc-500 hover:text-red-300"
                disabled={busy === `dismiss-${lesson.id}`}
                onClick={onDismiss}
                aria-label={`Descartar lição ${lesson.title}`}
              >
                {busy === `dismiss-${lesson.id}` ? (
                  <EyeOff size={12} />
                ) : (
                  <Trash2 size={12} />
                )}
              </Button>
            ) : null}
          </div>
        </div>
        {promoted && lesson.proposal_id ? (
          <Link
            to={`/studio?workspace_id=${encodeURIComponent(workspaceId)}`}
            className="mt-2 inline-flex items-center gap-1 text-[9px] text-violet-300 hover:text-violet-200"
          >
            Abrir Studio para revisar
            <ArrowUpRight size={10} />
          </Link>
        ) : null}
      </CardContent>
    </Card>
  );
}

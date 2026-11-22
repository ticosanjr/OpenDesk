import {
  AlertTriangle,
  ArrowUpRight,
  BookmarkPlus,
  CheckCircle2,
  Eye,
  EyeOff,
  RotateCcw,
  Sparkles,
  Trash2,
} from "lucide-react";
import { Link } from "react-router-dom";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { relativeTime } from "@/lib/format";
import {
  canPromote,
  canRemember,
  lessonCategoryLabel,
  lessonStatusLabel,
  lessonStatusTone,
  occurrencesLabel,
  readyToPromote,
} from "@/lib/learning";
import { links } from "@/lib/links";
import type { LearningLesson, LearningSettings } from "@/lib/types";

export function LessonCard({
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
              <p className="text-[13px] font-medium text-zinc-200">
                {lesson.title}
              </p>
              <Badge variant={lessonStatusTone(lesson.status)}>
                {lessonStatusLabel(lesson.status)}
              </Badge>
              <Badge variant="default">
                {lessonCategoryLabel(lesson.category)}
              </Badge>
              <span className="font-mono text-[11.5px] text-zinc-500">
                {occurrencesLabel(lesson.occurrences)}
              </span>
              {promotable && !ready ? (
                <span className="text-[11.5px] text-zinc-500">
                  abaixo do mínimo
                </span>
              ) : null}
            </div>
            <p className="mt-1 whitespace-pre-line text-[12px] leading-relaxed text-zinc-500 line-clamp-3">
              {lesson.detail}
            </p>
            <div className="mt-1.5 flex flex-wrap items-center gap-2 text-[11.5px] text-zinc-500">
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
                  className="inline-flex items-center gap-1 rounded-md border border-emerald-500/20 bg-emerald-500/10 px-1.5 py-0.5 font-mono text-[11.5px] uppercase tracking-wide text-emerald-300 hover:text-emerald-200"
                >
                  <BookmarkPlus size={10} />
                  na memória
                </Link>
                <Button
                  size="sm"
                  variant="outline"
                  className="h-7 text-[12px]"
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
                className="h-7 text-[12px]"
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
            className="mt-2 inline-flex items-center gap-1 text-[11.5px] text-violet-300 hover:text-violet-200"
          >
            Abrir Studio para revisar
            <ArrowUpRight size={10} />
          </Link>
        ) : null}
      </CardContent>
    </Card>
  );
}

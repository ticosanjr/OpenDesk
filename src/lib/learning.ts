import type { LearningLesson, LearningSettings } from "./types";

export const LESSON_CATEGORY_LABELS: Record<string, string> = {
  failure_pattern: "Falha recorrente",
  success_pattern: "Padrão eficaz",
};

export const LESSON_STATUS_LABELS: Record<string, string> = {
  OPEN: "Aberta",
  PROMOTED: "Promovida",
  DISMISSED: "Descartada",
};

export function lessonCategoryLabel(category: string): string {
  return LESSON_CATEGORY_LABELS[category] ?? category;
}

export function lessonStatusLabel(status: string): string {
  return LESSON_STATUS_LABELS[status] ?? status;
}

export type LessonTone = "default" | "green" | "purple" | "red";

export function lessonStatusTone(status: string): LessonTone {
  if (status === "PROMOTED") return "purple";
  if (status === "DISMISSED") return "red";
  if (status === "OPEN") return "green";
  return "default";
}

export function formatSuccessRate(rate: number): string {
  if (!Number.isFinite(rate) || rate <= 0) return "0%";
  return `${Math.round(rate * 100)}%`;
}

export function occurrencesLabel(count: number): string {
  return `${count} ${count === 1 ? "observação" : "observações"}`;
}

export function canPromote(
  lesson: LearningLesson,
  settings: LearningSettings,
): boolean {
  return settings.enabled && lesson.status === "OPEN" && !lesson.proposal_id;
}

export function canRemember(lesson: LearningLesson): boolean {
  return !lesson.remembered_at && lesson.status !== "DISMISSED";
}

export function proposalSkillId(
  proposal: { payload_json?: string | null },
): string | null {
  if (!proposal.payload_json) return null;
  try {
    const payload: unknown = JSON.parse(proposal.payload_json);
    if (
      payload &&
      typeof payload === "object" &&
      "id" in payload &&
      typeof (payload as { id?: unknown }).id === "string"
    ) {
      return (payload as { id: string }).id;
    }
    return null;
  } catch {
    return null;
  }
}

export function readyToPromote(
  lesson: LearningLesson,
  settings: LearningSettings,
): boolean {
  return canPromote(lesson, settings) && lesson.occurrences >= settings.min_occurrences;
}

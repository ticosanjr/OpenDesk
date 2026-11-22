import { apiDelete, apiGet, apiPost } from "@/lib/api";
import { buildInstallTask } from "@/lib/install-task";
import type { RecommendedSkill } from "@/lib/recommended-skills";
import type { Task } from "@/lib/types";
import type {
  Category,
  InstallPlan,
  InstallResult,
  ItemInstallResult,
  PackItem,
  PackSummary,
  WorkspaceTemplate,
} from "@/components/packs/pack-types";

export async function fetchStarterPackCatalog(workspaceId: string | null) {
  const suffix = workspaceId
    ? `?workspace_id=${encodeURIComponent(workspaceId)}`
    : "";
  const [catalog, templates] = await Promise.all([
    apiGet<{ categories: Category[]; packs: PackSummary[] }>(
      `/api/starter-packs${suffix}`,
    ),
    apiGet<WorkspaceTemplate[]>("/api/starter-packs/templates"),
  ]);
  return { catalog, templates };
}

export async function fetchPackItems(
  packId: string,
  workspaceId: string | null,
): Promise<PackItem[]> {
  const list = await apiGet<PackItem[]>(
    `/api/starter-packs/${packId}/items?workspace_id=${encodeURIComponent(
      workspaceId ?? "",
    )}`,
  );
  return Array.isArray(list) ? list : [];
}

export function previewPackInstall(
  packId: string,
  workspaceId: string,
): Promise<InstallPlan> {
  return apiPost<InstallPlan>(`/api/starter-packs/${packId}/preview`, {
    workspace_id: workspaceId,
  });
}

export function installPackItemRequest(
  packId: string,
  workspaceId: string,
  kind: string,
  id: string,
): Promise<ItemInstallResult> {
  return apiPost<ItemInstallResult>(
    `/api/starter-packs/${packId}/items/install`,
    {
      workspace_id: workspaceId,
      kind,
      id,
      overwrite: false,
    },
  );
}

export function installPackRequest(
  packId: string,
  workspaceId: string,
  overwrite: string[],
): Promise<InstallResult> {
  return apiPost<InstallResult>(`/api/starter-packs/${packId}/install`, {
    workspace_id: workspaceId,
    overwrite,
    skip_conflicts: true,
  });
}

export interface RemovePackResult {
  removed: number;
  kept: string[];
  routines_removed: number;
  routines_kept: string[];
}

export function removePackRequest(
  packId: string,
  workspaceId: string,
): Promise<RemovePackResult> {
  return apiDelete<RemovePackResult>(
    `/api/starter-packs/${packId}?workspace_id=${encodeURIComponent(workspaceId)}`,
  );
}

export function createTemplateWorkspaceRequest(payload: {
  templateId: string;
  name: string;
  rootPath: string | null;
  color: string;
}) {
  return apiPost<{
    workspace: { id: string; name: string };
    installed: string[];
    errors: string[];
  }>("/api/starter-packs/templates/create", {
    template_id: payload.templateId,
    name: payload.name,
    root_path: payload.rootPath,
    color: payload.color,
  });
}

export interface UpdatePacksSummary {
  total: number;
  written: number;
  skipped: number;
  failed: number;
  lastError: string | null;
}

export async function updateInstalledPacks(
  packs: PackSummary[],
  workspaceId: string,
): Promise<UpdatePacksSummary> {
  const targets = packs.filter((pack) => pack.installed && pack.update_available);
  let written = 0;
  let skipped = 0;
  let failed = 0;
  let lastError: string | null = null;
  for (const pack of targets) {
    try {
      const result = await installPackRequest(pack.id, workspaceId, []);
      written += result.written;
      skipped += result.skipped_conflicts;
    } catch (cause) {
      failed += 1;
      lastError = cause instanceof Error ? cause.message : String(cause);
    }
  }
  return { total: targets.length, written, skipped, failed, lastError };
}

export interface RemovePacksSummary {
  removed: number;
  kept: number;
  failed: number;
  lastError: string | null;
}

export async function removeInstalledPacks(
  ids: Iterable<string>,
  workspaceId: string,
): Promise<RemovePacksSummary> {
  let removed = 0;
  let kept = 0;
  let failed = 0;
  let lastError: string | null = null;
  for (const id of ids) {
    try {
      const result = await removePackRequest(id, workspaceId);
      removed += result.removed;
      kept += result.kept.length;
    } catch (cause) {
      failed += 1;
      lastError = cause instanceof Error ? cause.message : String(cause);
    }
  }
  return { removed, kept, failed, lastError };
}

export async function createSkillInstallTask(
  skill: RecommendedSkill,
  workspaceId: string,
  workspaceName: string | null,
): Promise<Task> {
  const payload = buildInstallTask({
    kind: "skill",
    name: skill.name,
    source: skill.source,
    sourceType: skill.sourceType,
    url: skill.url,
    installCommand: skill.installCommand,
    skillId: skill.skillId,
    note: skill.note,
    workspaceName,
  });
  const task = await apiPost<Task>(
    `/api/tasks?workspace_id=${encodeURIComponent(workspaceId)}`,
    {
      title: payload.title,
      description: payload.description,
      execution_mode: "AUTO_DSH",
      priority: 2,
      care_level: "standard",
      tags: payload.tags,
      origin: "MANUAL",
    },
  );
  if (!task.active_job_status) {
    await apiPost(`/api/tasks/${task.id}/dispatch`, {}).catch(() => {
      // The task is queued for manual execution when the engine is offline.
    });
  }
  return task;
}

export function installPackDescription(result: InstallResult): string {
  return `${result.written} arquivo(s), ${result.routines.length} rotina(s)${
    result.skipped_conflicts
      ? `, ${result.skipped_conflicts} conflito(s) mantido(s)`
      : ""
  }.`;
}

export function updatePacksDescription(summary: UpdatePacksSummary): string {
  return `${summary.total - summary.failed}/${summary.total} pacote(s) · ${summary.written} arquivo(s)${
    summary.skipped ? ` · ${summary.skipped} conflito(s) preservado(s)` : ""
  }${summary.lastError ? ` · ${summary.lastError}` : ""}.`;
}

export function removePacksDescription(summary: RemovePacksSummary): string {
  return `${summary.removed} arquivo(s) removido(s)${
    summary.kept ? ` · ${summary.kept} preservado(s)` : ""
  }${summary.failed ? ` · ${summary.failed} falha(s)` : ""}${
    summary.lastError ? ` · ${summary.lastError}` : ""
  }.`;
}

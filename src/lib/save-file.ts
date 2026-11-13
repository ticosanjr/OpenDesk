import { save } from "@tauri-apps/plugin-dialog";
import { apiBlob, apiPost } from "./api";
import { downloadBlob } from "./download";
import { loadFilePreferences } from "./file-preferences";
import { joinPath } from "./folder";

export interface SaveFileRequest {
  filename: string;
  workspaceId?: string | null;
  path?: string | null;
  artifactId?: string | null;
  /** Used only when the native dialog is unavailable (browser preview). */
  fallbackUrl?: string | null;
}

/**
 * Save a workspace file or artifact to the configured download folder, or ask
 * for a destination with the native save dialog.
 */
export async function saveLocalFile(request: SaveFileRequest): Promise<boolean> {
  const preferences = await loadFilePreferences();
  let destination = "";
  if (!preferences.downloadAsk && preferences.downloadDir.trim()) {
    destination = joinPath(preferences.downloadDir.trim(), request.filename);
  }
  if (!destination) {
    try {
      const selected = await save({
        title: "Salvar arquivo",
        defaultPath: preferences.downloadDir
          ? joinPath(preferences.downloadDir, request.filename)
          : request.filename,
      });
      if (!selected) return false;
      destination = selected;
    } catch {
      // Browser-only preview: fall back to the normal browser download.
      if (request.fallbackUrl) {
        downloadBlob(await apiBlob(request.fallbackUrl), request.filename);
        return true;
      }
      return false;
    }
  }

  if (request.artifactId) {
    await apiPost(
      `/api/artifacts/${encodeURIComponent(request.artifactId)}/save`,
      { destination },
    );
    return true;
  }
  if (request.workspaceId && request.path) {
    await apiPost("/api/files/save", {
      workspace_id: request.workspaceId,
      path: request.path,
      destination,
    });
    return true;
  }
  return false;
}

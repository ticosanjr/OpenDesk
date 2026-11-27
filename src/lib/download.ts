import { apiBlob } from "./api";
import { saveExportedData, type SavedExport } from "./import-export";

export type ExportFormat = "md" | "html" | "json";

export function exportFilename(runId: string, format: ExportFormat): string {
  const safe = runId.replace(/[^a-zA-Z0-9._-]+/g, "-").slice(0, 80) || "run";
  return `opendesk-run-${safe}.${format}`;
}

export function downloadBlob(blob: Blob, filename: string): void {
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = filename;
  document.body.appendChild(anchor);
  anchor.click();
  anchor.remove();
  window.setTimeout(() => URL.revokeObjectURL(url), 1000);
}

export async function downloadRun(runId: string, format: ExportFormat): Promise<SavedExport> {
  const blob = await apiBlob(
    `/api/runs/${encodeURIComponent(runId)}/export?format=${format}`,
  );
  return saveExportedData({
    filename: exportFilename(runId, format),
    content: blob,
    mime:
      format === "html"
        ? "text/html"
        : format === "json"
          ? "application/json"
          : "text/markdown",
    filters: [{ name: format.toUpperCase(), extensions: [format] }],
  });
}

/** Fetch any local API file with auth and save it through the browser. */
export async function downloadUrl(url: string, filename: string): Promise<void> {
  await saveExportedData({
    filename,
    content: await apiBlob(url),
    mime: "application/octet-stream",
  });
}

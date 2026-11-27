import { save } from "@tauri-apps/plugin-dialog";
import { apiPost } from "@/lib/api";
import { downloadBlob } from "@/lib/download";
import { loadFilePreferences } from "@/lib/file-preferences";
import { joinPath } from "@/lib/folder";
import { isTauriRuntime } from "@/lib/runtime-env";

export const BUNDLE_SCHEMA_VERSION = 1;

export type BundleKind =
  | "personal_context"
  | "structured_memory"
  | "prompt_templates"
  | "starter_pack"
  | "packs_bundle"
  | "workspaces_bundle"
  | "script_tools";

export type ImportMode = "merge" | "replace";

export interface BundleEnvelope<T = unknown> {
  schema_version: number;
  kind: BundleKind;
  exported_at: string;
  source_workspace?: string | null;
  items: T;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/** Fail-closed envelope validation shared by every import button. */
export function validateBundleEnvelope<T>(
  value: unknown,
  expectedKind: BundleKind,
): BundleEnvelope<T> {
  if (!isRecord(value)) {
    throw new Error("O arquivo precisa ser um objeto JSON.");
  }
  if (value.schema_version !== BUNDLE_SCHEMA_VERSION) {
    throw new Error(
      `schema_version ${String(value.schema_version)} não suportado (esperado ${BUNDLE_SCHEMA_VERSION}).`,
    );
  }
  if (value.kind !== expectedKind) {
    throw new Error(
      `kind incompatível: o arquivo é '${String(value.kind)}', esperado '${expectedKind}'.`,
    );
  }
  if (!("items" in value) || value.items === null || value.items === undefined) {
    throw new Error("O bundle não contém items.");
  }
  return value as unknown as BundleEnvelope<T>;
}

export async function exportJsonBundle<T>(
  kind: BundleKind,
  items: T,
  filename: string,
  sourceWorkspace?: string | null,
): Promise<string | null> {
  const bundle: BundleEnvelope<T> = {
    schema_version: BUNDLE_SCHEMA_VERSION,
    kind,
    exported_at: new Date().toISOString(),
    source_workspace: sourceWorkspace ?? null,
    items,
  };
  const result = await saveExportedData({
    filename,
    content: JSON.stringify(bundle, null, 2),
    mime: "application/json",
    filters: [{ name: "JSON", extensions: ["json"] }],
  });
  return result.saved ? filename : null;
}

function blobToBase64(blob: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      const value = String(reader.result ?? "");
      const comma = value.indexOf(",");
      resolve(comma >= 0 ? value.slice(comma + 1) : value);
    };
    reader.onerror = () => reject(reader.error ?? new Error("falha ao ler o arquivo"));
    reader.readAsDataURL(blob);
  });
}

export interface SavedExport {
  saved: boolean;
  path?: string;
}

/** Unified policy for every export/import in the app. */
export async function saveExportedData(options: {
  filename: string;
  content: string | Blob;
  mime: string;
  filters?: Array<{ name: string; extensions: string[] }>;
}): Promise<SavedExport> {
  const preferences = await loadFilePreferences();
  const blob =
    options.content instanceof Blob
      ? options.content
      : new Blob([options.content], { type: options.mime });

  if (!isTauriRuntime()) {
    downloadBlob(blob, options.filename);
    return { saved: true };
  }

  const autoSave =
    !preferences.importExportAsk && preferences.downloadDir.trim() !== "";
  if (autoSave) {
    const destination = joinPath(preferences.downloadDir.trim(), options.filename);
    try {
      await apiPost("/api/files/save-bytes", {
        destination,
        content_base64: await blobToBase64(blob),
      });
      return { saved: true, path: destination };
    } catch (cause) {
      // The configured folder may have been removed; fall through to the
      // native dialog instead of losing the export.
      console.warn("OpenDesk: auto export failed, asking for a destination", cause);
    }
  }

  let selected: string | null;
  try {
    selected = await save({
      title: "Salvar arquivo",
      defaultPath: preferences.downloadDir.trim()
        ? joinPath(preferences.downloadDir.trim(), options.filename)
        : options.filename,
      filters: options.filters,
    });
  } catch {
    downloadBlob(blob, options.filename);
    return { saved: true };
  }
  if (!selected) return { saved: false };
  await apiPost("/api/files/save-bytes", {
    destination: selected,
    content_base64: await blobToBase64(blob),
  });
  return { saved: true, path: selected };
}

export async function pickImportFile(options: {
  title: string;
  extensions: string[];
}): Promise<{ name: string; text: string } | null> {
  return new Promise((resolve, reject) => {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = options.extensions.map((extension) => `.${extension}`).join(",");
    input.style.display = "none";
    document.body.appendChild(input);
    let settled = false;
    const cleanup = () => input.remove();
    input.addEventListener("change", () => {
      if (settled) return;
      const file = input.files?.[0];
      if (!file) {
        settled = true;
        cleanup();
        resolve(null);
        return;
      }
      settled = true;
      void file
        .text()
        .then((text) => {
          cleanup();
          resolve({ name: file.name, text });
        })
        .catch((cause: unknown) => {
          cleanup();
          reject(cause);
        });
    });
    input.addEventListener("cancel", () => {
      if (settled) return;
      settled = true;
      cleanup();
      resolve(null);
    });
    input.click();
  });
}

export async function readJsonBundle<T>(
  file: File,
  expectedKind: BundleKind,
): Promise<BundleEnvelope<T>> {
  let parsed: unknown;
  try {
    parsed = JSON.parse(await file.text()) as unknown;
  } catch {
    throw new Error("JSON inválido. Escolha um arquivo exportado pelo OpenDesk.");
  }
  return validateBundleEnvelope<T>(parsed, expectedKind);
}

/** Opens the native file picker and validates the chosen envelope. */
export async function pickJsonBundle<T>(
  expectedKind: BundleKind,
): Promise<{ bundle: BundleEnvelope<T>; fileName: string } | null> {
  const picked = await pickImportFile({
    title: "Importar JSON",
    extensions: ["json"],
  });
  if (!picked) return null;
  let parsed: unknown;
  try {
    parsed = JSON.parse(picked.text) as unknown;
  } catch {
    throw new Error("JSON inválido. Escolha um arquivo exportado pelo OpenDesk.");
  }
  return {
    bundle: validateBundleEnvelope<T>(parsed, expectedKind),
    fileName: picked.name,
  };
}

export function importJsonBundle(
  path: string,
  bundle: BundleEnvelope,
  workspaceId: string | null,
  mode: ImportMode,
): Promise<unknown> {
  return apiPost(path, {
    workspace_id: workspaceId,
    mode,
    bundle,
  });
}

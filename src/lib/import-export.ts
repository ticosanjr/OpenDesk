import { apiPost } from "@/lib/api";
import { downloadBlob } from "@/lib/download";

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

export function exportJsonBundle<T>(
  kind: BundleKind,
  items: T,
  filename: string,
  sourceWorkspace?: string | null,
): string {
  const bundle: BundleEnvelope<T> = {
    schema_version: BUNDLE_SCHEMA_VERSION,
    kind,
    exported_at: new Date().toISOString(),
    source_workspace: sourceWorkspace ?? null,
    items,
  };
  downloadBlob(
    new Blob([JSON.stringify(bundle, null, 2)], { type: "application/json" }),
    filename,
  );
  return filename;
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
export function pickJsonBundle<T>(
  expectedKind: BundleKind,
): Promise<{ bundle: BundleEnvelope<T>; fileName: string } | null> {
  return new Promise((resolve, reject) => {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = "application/json,.json";
    input.style.display = "none";
    document.body.appendChild(input);
    let settled = false;
    const cleanup = () => {
      input.remove();
    };
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
      void readJsonBundle<T>(file, expectedKind)
        .then((bundle) => {
          cleanup();
          resolve({ bundle, fileName: file.name });
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

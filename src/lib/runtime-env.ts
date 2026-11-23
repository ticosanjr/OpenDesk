import { invoke } from "@tauri-apps/api/core";
import { openPath, openUrl } from "@tauri-apps/plugin-opener";

export interface OpenLocalPathResult {
  ok: boolean;
  fallback?: string;
}

/** Tauri exposes this bridge in both the desktop webview and dev mode. */
export function isTauriRuntime(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

function externalUrl(raw: string): string {
  let parsed: URL;
  try {
    parsed = new URL(raw);
  } catch {
    throw new Error("URL externa inválida");
  }
  if (parsed.protocol !== "https:" && parsed.protocol !== "http:") {
    throw new Error("Apenas URLs http(s) podem ser abertas");
  }
  return parsed.toString();
}

/** Opens an external URL in the desktop runtime or in a browser/PWA tab. */
export async function openExternalUrl(raw: string): Promise<void> {
  const url = externalUrl(raw);
  if (isTauriRuntime()) {
    await openUrl(url);
    return;
  }
  const opened = window.open(url, "_blank", "noopener,noreferrer");
  if (!opened) {
    throw new Error("O navegador bloqueou a nova aba");
  }
}

/**
 * Opens a filesystem path with the OS handler. Browser/PWA mode cannot access
 * local files, so callers receive the path to display instead of an exception.
 */
export async function openLocalPath(path: string): Promise<OpenLocalPathResult> {
  if (!isTauriRuntime()) {
    return { ok: false, fallback: path };
  }
  try {
    await openPath(path);
    return { ok: true };
  } catch {
    return { ok: false, fallback: path };
  }
}

/** Opens a workspace path through the Rust allowlist command. */
export async function openWorkspacePath(path: string): Promise<void> {
  if (!isTauriRuntime()) {
    throw new Error("Abrir arquivos locais exige o aplicativo desktop");
  }
  await invoke("open_workspace_path", { path });
}

/** Hides the Quick Capture window when running in Tauri; returns false in PWA. */
export async function hideWindowIfTauri(): Promise<boolean> {
  if (!isTauriRuntime()) return false;
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  await getCurrentWindow().hide();
  return true;
}

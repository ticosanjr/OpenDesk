import { open } from "@tauri-apps/plugin-dialog";

/** Native folder picker. Returns null in browser-only mode or on cancel. */
export async function pickFolder(
  title: string,
  defaultPath?: string,
): Promise<string | null> {
  try {
    const selected = await open({
      directory: true,
      multiple: false,
      title,
      defaultPath: defaultPath || undefined,
    });
    return typeof selected === "string" ? selected : null;
  } catch (error) {
    console.warn("OpenDesk: native folder picker unavailable", error);
    return null;
  }
}

/** Native file picker. Returns null in browser-only mode or on cancel. */
export async function pickFile(
  title: string,
  defaultPath?: string,
): Promise<string | null> {
  try {
    const selected = await open({
      directory: false,
      multiple: false,
      title,
      defaultPath: defaultPath || undefined,
    });
    return typeof selected === "string" ? selected : null;
  } catch (error) {
    console.warn("OpenDesk: native file picker unavailable", error);
    return null;
  }
}

export function slugifyWorkspace(name: string): string {
  const normalized = name
    .normalize("NFD")
    .replace(/[\u0300-\u036f]/g, "")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 64);

  return normalized || `workspace-${Date.now()}`;
}

export function joinPath(parent: string, child: string): string {
  if (!parent) return child;
  return `${parent.replace(/[\\/]+$/, "")}/${child}`;
}

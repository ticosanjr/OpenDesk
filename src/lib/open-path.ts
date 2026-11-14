import { invoke } from "@tauri-apps/api/core";

/** Opens a workspace/app path through the Rust allowlist, bypassing the
 * static opener plugin scope so configurable workspace roots work. */
export async function openWorkspacePath(path: string): Promise<void> {
  await invoke("open_workspace_path", { path });
}

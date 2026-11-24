import { apiPost } from "@/lib/api";
import { downloadBlob } from "@/lib/download";

export const CONFIG_BUNDLE_PRESETS: Record<string, string[]> = {
  all: [],
  memory: ["memory_md", "structured_memory", "personal_context", "learning"],
  prompts: ["prompt_templates"],
  ops: ["router", "budget", "queue", "automation_routes", "governance"],
};

export async function downloadConfigBundle(
  workspaceId: string,
  sections: string[] = [],
): Promise<string> {
  const data = await apiPost<unknown>("/api/config-bundle/export", {
    workspace_id: workspaceId,
    sections,
  });
  const name = `opendesk-config-${new Date().toISOString().slice(0, 10)}.json`;
  downloadBlob(
    new Blob([JSON.stringify(data, null, 2)], { type: "application/json" }),
    name,
  );
  return name;
}

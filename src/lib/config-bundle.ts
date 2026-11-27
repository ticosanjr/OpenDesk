import { apiPost } from "@/lib/api";
import { saveExportedData } from "@/lib/import-export";

export const CONFIG_BUNDLE_PRESETS: Record<string, string[]> = {
  all: [],
  memory: ["memory_md", "structured_memory", "personal_context", "learning"],
  prompts: ["prompt_templates"],
  ops: ["router", "budget", "queue", "automation_routes", "governance"],
};

export async function downloadConfigBundle(
  workspaceId: string,
  sections: string[] = [],
): Promise<string | null> {
  const data = await apiPost<unknown>("/api/config-bundle/export", {
    workspace_id: workspaceId,
    sections,
  });
  const name = `opendesk-config-${new Date().toISOString().slice(0, 10)}.json`;
  const result = await saveExportedData({
    filename: name,
    content: JSON.stringify(data, null, 2),
    mime: "application/json",
    filters: [{ name: "JSON", extensions: ["json"] }],
  });
  return result.saved ? name : null;
}

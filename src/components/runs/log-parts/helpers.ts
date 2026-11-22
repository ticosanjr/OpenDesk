import type { TimelineEvent } from "@/lib/types";

const FAILED_TOOL_STATUS = new Set(["error", "failed", "failure", "timeout"]);

export function isFailedTool(event: TimelineEvent): boolean {
  const status = event.tool?.status?.toLowerCase() ?? "";
  return event.kind === "tool" && FAILED_TOOL_STATUS.has(status);
}

export function relativeToolPath(
  path: string | null,
  root?: string | null,
): string | null {
  if (!path) return null;
  if (!path.startsWith("/") && !path.includes(":\\")) return path;
  if (root) {
    const normalizedRoot = root.replace(/\/$/, "");
    if (path.startsWith(normalizedRoot + "/")) {
      return path.slice(normalizedRoot.length + 1);
    }
  }
  return null;
}

export function toolPath(input: string): string | null {
  try {
    const value = JSON.parse(input) as Record<string, unknown>;
    const path = value.path ?? value.filePath ?? value.file_path;
    return typeof path === "string" ? path : null;
  } catch {
    return null;
  }
}

export function pretty(value: string): string {
  try {
    return JSON.stringify(JSON.parse(value), null, 2);
  } catch {
    return value;
  }
}

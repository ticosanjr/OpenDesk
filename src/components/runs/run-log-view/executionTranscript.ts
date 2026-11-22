import type { RunTimeline, TaskMessage } from "@/lib/types";

/**
 * Plain-text rendering of a run thread, matching what "copiar execução" puts
 * on the clipboard.
 */
export function executionTranscript(
  messages: TaskMessage[],
  timeline: RunTimeline | null,
  summary: string | null | undefined,
): string {
  const lines: string[] = [];
  for (const message of messages) {
    lines.push(`Você: ${message.content}`);
  }
  for (const event of timeline?.events ?? []) {
    if (event.kind === "comment" && event.text.trim()) {
      lines.push(`Assistente: ${event.text.trim()}`);
    } else if (event.kind === "tool" && event.tool) {
      lines.push(`[ferramenta] ${event.tool.summary} (${event.tool.status})`);
    }
  }
  const result = timeline?.result.text?.trim() || summary;
  if (result) lines.push(`Resultado: ${result}`);
  return lines.join("\n\n");
}

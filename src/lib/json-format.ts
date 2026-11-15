/** Formats valid JSON and JSONL without pulling a formatter dependency. */
export function formatJsonContent(content: string): {
  text: string;
  formatted: boolean;
} {
  const trimmed = content.trim();
  if (!trimmed) return { text: content, formatted: false };

  try {
    return { text: JSON.stringify(JSON.parse(trimmed), null, 2), formatted: true };
  } catch {
    // Try JSON Lines: one complete JSON value per non-empty line.
  }

  const lines = content.split("\n");
  const values = lines.filter((line) => line.trim().length > 0);
  if (values.length > 1) {
    try {
      const formatted = values.map((line) => JSON.stringify(JSON.parse(line)));
      return { text: formatted.join("\n"), formatted: true };
    } catch {
      // Not JSONL either; keep the original text.
    }
  }
  return { text: content, formatted: false };
}

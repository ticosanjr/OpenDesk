export type DiffLineKind = "add" | "del" | "context" | "hunk" | "meta";

export interface DiffLine {
  kind: DiffLineKind;
  text: string;
  oldLine?: number;
  newLine?: number;
}

export interface ParsedDiff {
  lines: DiffLine[];
  additions: number;
  deletions: number;
}

const HUNK = /^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@/;

export function parseUnifiedDiff(patch: string): ParsedDiff {
  const lines: DiffLine[] = [];
  let oldLine: number | undefined;
  let newLine: number | undefined;
  let additions = 0;
  let deletions = 0;

  const rawLines = patch.replace(/\r\n/g, "\n").split("\n");
  if (rawLines[rawLines.length - 1] === "") rawLines.pop();
  for (const text of rawLines) {
    if (!text && lines.length === 0) continue;
    if (text.startsWith("diff --git") || text.startsWith("index ")) {
      lines.push({ kind: "meta", text });
      oldLine = undefined;
      newLine = undefined;
      continue;
    }
    if (text.startsWith("--- ") || text.startsWith("+++ ")) {
      lines.push({ kind: "meta", text });
      continue;
    }
    const hunk = HUNK.exec(text);
    if (hunk) {
      oldLine = Number(hunk[1]);
      newLine = Number(hunk[2]);
      lines.push({ kind: "hunk", text });
      continue;
    }
    if (text.startsWith("+")) {
      lines.push({ kind: "add", text: text.slice(1), newLine });
      additions += 1;
      if (newLine !== undefined) newLine += 1;
      continue;
    }
    if (text.startsWith("-")) {
      lines.push({ kind: "del", text: text.slice(1), oldLine });
      deletions += 1;
      if (oldLine !== undefined) oldLine += 1;
      continue;
    }
    if (text.startsWith("\\ No newline")) {
      lines.push({ kind: "meta", text });
      continue;
    }
    const content = text.startsWith(" ") ? text.slice(1) : text;
    lines.push({ kind: "context", text: content, oldLine, newLine });
    if (oldLine !== undefined) oldLine += 1;
    if (newLine !== undefined) newLine += 1;
  }

  return { lines, additions, deletions };
}

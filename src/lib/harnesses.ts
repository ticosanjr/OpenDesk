export interface HarnessOption {
  id: string;
  label: string;
  description: string;
}

/** Built-in harnesses. The backend is the source of truth; this list drives the
 * default labels in forms and selector fallbacks. */
export const HARNESS_OPTIONS: HarnessOption[] = [
  {
    id: "opencode",
    label: "OpenCode V2",
    description: "daemon local, sessões, permissões e diffs",
  },
  {
    id: "claude-code",
    label: "Claude Code",
    description: "CLI não interativa com stream-json",
  },
  { id: "codex", label: "Codex CLI", description: "codex exec --json" },
  { id: "pi", label: "Pi", description: "CLI JSON-lines" },
  { id: "hermes", label: "Hermes", description: "CLI JSON-lines" },
];

export function harnessLabel(id: string | null | undefined): string {
  if (!id) return "opencode";
  return HARNESS_OPTIONS.find((option) => option.id === id)?.label ?? id;
}

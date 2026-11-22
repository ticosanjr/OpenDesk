export function artifactActionLabel(action: string): string {
  if (action === "CREATED") return "criado";
  if (action === "DELETED") return "excluído";
  if (action === "MODIFIED") return "alterado";
  return action;
}

export interface ArtifactContent {
  path: string;
  content: string;
  truncated: boolean;
  binary: boolean;
  mime: string;
  size: number;
}

export function containingFolder(path: string): string {
  const index = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  return index > 0 ? path.slice(0, index) : path;
}

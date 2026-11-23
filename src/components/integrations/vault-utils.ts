import type { VaultEntry } from "./VaultCard";

export function domainsOf(entry: VaultEntry): string[] {
  try {
    const value = JSON.parse(entry.allowed_domains_json);
    return Array.isArray(value) ? value.filter((item) => typeof item === "string") : [];
  } catch {
    return [];
  }
}

export function parseAcls(value: string) {
  return value
    .split(/[\n,]/)
    .map((line) => line.trim())
    .filter(Boolean)
    .map((line) => {
      const [subjectKind, subjectId] = line.split(":").map((part) => part.trim());
      return {
        subject_kind: subjectKind || "agent",
        subject_id: subjectId || "",
        can_inject: true,
      };
    })
    .filter((acl) => acl.subject_id.length > 0);
}

/**
 * Canonical OpenCode model reference: provider/model or provider/model#variant.
 * Empty variants are dropped instead of becoming `provider/model#`.
 */
export function normalizeModelRef(value: string): string | null {
  const trimmed = value.trim();
  if (!trimmed) return null;
  const hash = trimmed.indexOf("#");
  const base = (hash === -1 ? trimmed : trimmed.slice(0, hash)).trim();
  const variant = hash === -1 ? "" : trimmed.slice(hash + 1).trim();
  const slash = base.indexOf("/");
  if (slash <= 0 || slash === base.length - 1) return null;
  const provider = base.slice(0, slash).trim();
  const id = base.slice(slash + 1).trim();
  if (!provider || !id) return null;
  return variant ? `${provider}/${id}#${variant}` : `${provider}/${id}`;
}

export function isValidModelRef(value: string): boolean {
  return !value.trim() || normalizeModelRef(value) !== null;
}

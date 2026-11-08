/**
 * A model choice is either `null` (automatic router) or a concrete
 * `provider/model` reference. An empty string is never a valid selection.
 */
export type ModelSelection = string | null;

export function toModelSelection(value?: string | null): ModelSelection {
  const trimmed = value?.trim();
  return trimmed ? trimmed : null;
}

export function isRouterSelection(value: ModelSelection): boolean {
  return toModelSelection(value) === null;
}

export function modelSelectionLabel(value: ModelSelection): string {
  return toModelSelection(value) ?? "Roteador automático";
}

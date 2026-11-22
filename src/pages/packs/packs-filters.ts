import type { FavoriteKind } from "@/lib/favorites";
import type { PackSummary, WorkspaceTemplate } from "@/components/packs/pack-types";

const ONBOARDING_KEY = "opendesk.starter-packs.onboarding-dismissed";

export function readOnboardingDismissed(): boolean {
  try {
    return localStorage.getItem(ONBOARDING_KEY) === "1";
  } catch {
    return false;
  }
}

export function writeOnboardingDismissed(): void {
  try {
    localStorage.setItem(ONBOARDING_KEY, "1");
  } catch {
    // storage can be unavailable; dismiss for this session only
  }
}

export function filterPacks(
  packs: PackSummary[],
  filters: {
    query: string;
    category: string;
    installFilter: "all" | "installed" | "available" | "favorites";
    isFavorite: (kind: FavoriteKind, id: string) => boolean;
  },
): PackSummary[] {
  const needle = filters.query.trim().toLowerCase();
  return packs.filter((pack) => {
    if (filters.category !== "all" && pack.category !== filters.category) return false;
    if (filters.installFilter === "installed" && !pack.installed) return false;
    if (filters.installFilter === "available" && pack.installed) return false;
    if (
      filters.installFilter === "favorites" &&
      !filters.isFavorite("pack", pack.id)
    ) {
      return false;
    }
    if (!needle) return true;
    return `${pack.name} ${pack.description} ${pack.id}`
      .toLowerCase()
      .includes(needle);
  });
}

export function filterTemplates(
  templates: WorkspaceTemplate[],
  filters: {
    query: string;
    favoritesOnly: boolean;
    isFavorite: (kind: FavoriteKind, id: string) => boolean;
  },
): WorkspaceTemplate[] {
  const needle = filters.query.trim().toLowerCase();
  return templates.filter((template) => {
    if (filters.favoritesOnly && !filters.isFavorite("template", template.id)) {
      return false;
    }
    if (!needle) return true;
    return `${template.name} ${template.description} ${template.packs.join(" ")}`
      .toLowerCase()
      .includes(needle);
  });
}

import { describe, expect, it } from "vitest";
import { EXTERNAL_RESOURCES, RESOURCE_GROUPS } from "./external-resources";

describe("external resources", () => {
  it("has unique ids, valid groups and https links", () => {
    const ids = new Set<string>();
    for (const resource of EXTERNAL_RESOURCES) {
      expect(ids.has(resource.id)).toBe(false);
      ids.add(resource.id);
      expect(resource.url.startsWith("https://")).toBe(true);
      expect(RESOURCE_GROUPS.some((group) => group.id === resource.group)).toBe(true);
    }
    expect(EXTERNAL_RESOURCES.length).toBeGreaterThanOrEqual(15);
  });

  it("offers at least one recommendation for each workspace template", () => {
    const templates = new Set(
      EXTERNAL_RESOURCES.map((resource) => resource.templateId).filter(Boolean),
    );
    expect(templates.has("fabrica-capacidades")).toBe(true);
    expect(templates.has("gtm-clientes")).toBe(true);
    expect(templates.has("conteudo-midia")).toBe(true);
  });
});

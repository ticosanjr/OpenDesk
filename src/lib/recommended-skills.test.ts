import { describe, expect, it } from "vitest";
import { RECOMMENDED_SKILLS } from "./recommended-skills";

describe("recommended skills", () => {
  it("includes the curated list with unique ids", () => {
    const ids = RECOMMENDED_SKILLS.map((skill) => skill.id);
    expect(ids).toEqual([
      "caveman",
      "ponytail",
      "implement",
      "code-review",
      "diagnosing-bugs",
      "codebase-design",
      "grilling",
      "tdd",
      "improve-codebase-architecture",
      "grill-me",
      "impeccable",
    ]);
    expect(new Set(ids).size).toBe(ids.length);
  });

  it("points every recommendation at a real source", () => {
    for (const skill of RECOMMENDED_SKILLS) {
      expect(skill.url).toMatch(/^https:\/\//);
      expect(skill.source).toMatch(/^(https:\/\/\S+|[\w.-]+\/[\w.-]+)$/);
      expect(skill.skillId.length).toBeGreaterThan(0);
      expect(skill.description.length).toBeGreaterThan(20);
    }
    expect(
      RECOMMENDED_SKILLS.filter((skill) => skill.installCommand?.includes("--skill")),
    ).toHaveLength(8);
    expect(
      RECOMMENDED_SKILLS.find((skill) => skill.id === "impeccable")?.installable,
    ).toBe(false);
  });
});

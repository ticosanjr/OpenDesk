import { describe, expect, it } from "vitest";
import type { LearningLesson, LearningSettings } from "./types";
import {
  canPromote,
  canRemember,
  formatSuccessRate,
  lessonCategoryLabel,
  lessonStatusLabel,
  lessonStatusTone,
  occurrencesLabel,
  proposalSkillId,
  readyToPromote,
} from "./learning";

const settings: LearningSettings = {
  enabled: true,
  capture_successes: false,
  auto_propose: false,
  min_occurrences: 2,
};

function lesson(part: Partial<LearningLesson> = {}): LearningLesson {
  return {
    id: "l1",
    workspace_id: "ws",
    category: "failure_pattern",
    severity: "warning",
    title: "Falha recorrente",
    detail: "detalhe",
    evidence_json: "{}",
    fingerprint: "failure:timeout",
    occurrences: 1,
    status: "OPEN",
    last_seen_at: "2026-01-01T00:00:00Z",
    created_at: "2026-01-01T00:00:00Z",
    updated_at: "2026-01-01T00:00:00Z",
    ...part,
  };
}

describe("learning helpers", () => {
  it("labels categories and statuses with a readable fallback", () => {
    expect(lessonCategoryLabel("failure_pattern")).toBe("Falha recorrente");
    expect(lessonCategoryLabel("custom")).toBe("custom");
    expect(lessonStatusLabel("OPEN")).toBe("Aberta");
    expect(lessonStatusLabel("OTHER")).toBe("OTHER");
    expect(lessonStatusTone("PROMOTED")).toBe("purple");
    expect(lessonStatusTone("DISMISSED")).toBe("red");
    expect(lessonStatusTone("OPEN")).toBe("green");
  });

  it("formats success rate and occurrences", () => {
    expect(formatSuccessRate(0.923)).toBe("92%");
    expect(formatSuccessRate(Number.NaN)).toBe("0%");
    expect(occurrencesLabel(1)).toBe("1 observação");
    expect(occurrencesLabel(3)).toBe("3 observações");
  });

  it("only promotes open lessons without a proposal after the threshold", () => {
    expect(canPromote(lesson(), settings)).toBe(true);
    expect(canPromote(lesson({ proposal_id: "p1" }), settings)).toBe(false);
    expect(canPromote(lesson({ status: "DISMISSED" }), settings)).toBe(false);
    expect(canPromote(lesson(), { ...settings, enabled: false })).toBe(false);
    expect(readyToPromote(lesson(), settings)).toBe(false);
    expect(readyToPromote(lesson({ occurrences: 2 }), settings)).toBe(true);
  });

  it("only remembers lessons that are not dismissed or already stored", () => {
    expect(canRemember(lesson())).toBe(true);
    expect(canRemember(lesson({ remembered_at: "2026-01-01" }))).toBe(false);
    expect(canRemember(lesson({ status: "DISMISSED" }))).toBe(false);
  });

  it("reads the skill id from a proposal payload", () => {
    expect(proposalSkillId({ payload_json: JSON.stringify({ id: "skill-x" }) })).toBe(
      "skill-x",
    );
    expect(proposalSkillId({ payload_json: "not json" })).toBeNull();
    expect(proposalSkillId({})).toBeNull();
  });
});

import { describe, expect, it } from "vitest";
import {
  hitlConsequences,
  hitlSecondsLeft,
  hitlTypeLabel,
  parseHitlContext,
  parseHitlOptions,
} from "./hitl";

describe("HITL helpers", () => {
  it("parses options and context safely", () => {
    expect(parseHitlOptions('["Sim","Não"]')).toEqual(["Sim", "Não"]);
    expect(parseHitlOptions('["Sim",1,null]')).toEqual(["Sim"]);
    expect(parseHitlOptions("not json")).toEqual([]);
    expect(parseHitlContext('{"next_step":"Build"}')).toEqual({
      next_step: "Build",
    });
    expect(parseHitlContext("[]")).toEqual({});
    expect(parseHitlContext("not json")).toEqual({});
  });

  it("explains consequences using the next step and timeout policy", () => {
    const consequences = hitlConsequences({
      interaction_type: "BINARY_GATE",
      on_timeout: "PAUSE_AND_ALERT",
      context_json: '{"next_step":"Build"}',
    });
    expect(consequences.approve).toContain("Build");
    expect(consequences.reject).toContain("Encerra este run");
    expect(consequences.timeout).toContain("pausa");
  });

  it("covers acknowledge and timeout policies", () => {
    const acknowledge = hitlConsequences({
      interaction_type: "ACKNOWLEDGE",
      on_timeout: "AUTO_APPROVE",
      context_json: null,
    });
    expect(acknowledge.approve).toContain("ciência");
    expect(acknowledge.reject).toContain("não interrompe");
    expect(acknowledge.timeout).toContain("aprova automaticamente");

    const rejected = hitlConsequences({
      interaction_type: "BINARY_GATE",
      on_timeout: "AUTO_REJECT",
      context_json: '{"next_step":"  "}',
    });
    expect(rejected.approve).toContain("a próxima etapa");
    expect(rejected.timeout).toContain("rejeitado");
  });

  it("labels types and clamps countdowns", () => {
    expect(hitlTypeLabel("MULTI_CHOICE")).toBe("Escolha de opção");
    expect(hitlTypeLabel("OTHER")).toBe("OTHER");
    expect(hitlSecondsLeft("2026-01-01T00:01:00Z", Date.parse("2026-01-01T00:00:00Z"))).toBe(
      60,
    );
    expect(hitlSecondsLeft("2026-01-01T00:00:00Z", Date.parse("2026-01-01T00:01:00Z"))).toBe(
      0,
    );
    expect(hitlSecondsLeft("not-a-date")).toBe(0);
    expect(hitlSecondsLeft()).toBe(0);
  });
});

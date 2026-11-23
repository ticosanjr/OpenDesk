import { describe, expect, it } from "vitest";
import { commit, normalizeFlow } from "./recipe-editor-model";
import type { Dictionary } from "../recipe-types";

describe("recipe editor DAG round-trip", () => {
  it("preserves all nine node types and unknown fields", () => {
    const recipe: Dictionary = {
      flow: [
        { type: "step", id: "s1", name: "S", prompt: "x", custom: "keep" },
        { type: "branch", id: "b1", when: "true", then: [], else: [] },
        { type: "parallel", id: "p1", join: "all", branches: [] },
        { type: "map", id: "m1", items: "[]", flow: [] },
        { type: "recipe_call", id: "c1", recipe: "outra", with: {} },
        { type: "loop", id: "l1", max_iterations: 2, flow: [] },
        { type: "switch", id: "sw1", cases: [], default: [] },
        { type: "hitl", id: "h1", interaction_type: "BINARY_GATE" },
        { type: "sink", id: "k1", sink_type: "DOC_COMPILER" },
      ],
    };
    const flow = normalizeFlow(recipe);
    expect(flow.map((item) => item.type)).toEqual([
      "step",
      "branch",
      "parallel",
      "map",
      "recipe_call",
      "loop",
      "switch",
      "hitl",
      "sink",
    ]);
    expect(flow[0].custom).toBe("keep");

    let next: Dictionary = {};
    commit(recipe, flow, [], (value) => {
      next = value;
    });
    const committed = next.flow as Dictionary[];
    expect(committed).toHaveLength(9);
    expect(committed[1].then).toEqual([]);
    expect(committed[4].recipe).toBe("outra");
  });
});

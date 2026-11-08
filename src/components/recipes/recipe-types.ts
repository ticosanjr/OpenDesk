export type Dictionary = Record<string, unknown>;
export type FlowEntry = Dictionary & { type: "step" | "hitl" | "sink" };

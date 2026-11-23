export type Dictionary = Record<string, unknown>;
export type FlowNodeType =
  | "step"
  | "hitl"
  | "sink"
  | "branch"
  | "parallel"
  | "map"
  | "recipe_call"
  | "loop"
  | "switch";

export type FlowEntry = Dictionary & { type: FlowNodeType };

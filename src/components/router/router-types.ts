import type { BudgetStatus, RouterRule } from "@/lib/types";

export interface RouterConfig {
  workspace_id: string;
  policy: string;
  default_model?: string | null;
  manual_model?: string | null;
  router_model?: string | null;
  default_harness?: string;
  cost_blend: number;
  rules: RouterRule[];
  budget: BudgetStatus;
}

export interface ModelCost {
  model: string;
  provider: string;
  name: string;
  input_cost: number;
  output_cost: number;
  blended_cost: number;
}

export interface RouterAnalysis {
  workspace_id: string;
  router_model?: string | null;
  cost_blend: number;
  recommendation: string;
  models: ModelCost[];
  cheapest?: string | null;
  generated_at: string;
}

export interface RouterTestResult {
  policy: string;
  default_model?: string | null;
  manual_model?: string | null;
  engine_online: boolean;
  model_available: boolean;
  available_models: number;
  decision: {
    phase: string;
    selected_agent: string;
    selected_model: string;
    harness: string;
    effort: string;
    rationale: string;
    origin: string;
    policy_applied: string;
    rule_name?: string | null;
  };
}

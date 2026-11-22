import type { RunAttempt, TaskRun } from "@/lib/types";

export type RunLogTotals = {
  input: number;
  output: number;
  tokens: number;
  cost: number;
};

export function runLogTotals(
  attempts: RunAttempt[],
  run: TaskRun | null | undefined,
): RunLogTotals {
  const totalTokens = attempts.length
    ? attempts.reduce(
        (total, attempt) =>
          total + attempt.tokens_input + attempt.tokens_output,
        0,
      )
    : (run?.tokens_input ?? 0) + (run?.tokens_output ?? 0);
  const totalInput = attempts.length
    ? attempts.reduce((total, attempt) => total + attempt.tokens_input, 0)
    : run?.tokens_input ?? 0;
  const totalOutput = attempts.length
    ? attempts.reduce((total, attempt) => total + attempt.tokens_output, 0)
    : run?.tokens_output ?? 0;
  const totalCost = attempts.length
    ? attempts.reduce((total, attempt) => total + attempt.cost_usd, 0)
    : run?.estimated_cost_usd ?? 0;
  return {
    input: totalInput,
    output: totalOutput,
    tokens: totalTokens,
    cost: totalCost,
  };
}

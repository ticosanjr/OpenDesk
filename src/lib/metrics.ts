import type { DailyMetrics } from "./types";

export const FAILURE_LABELS: Record<string, string> = {
  engine_offline: "motor offline",
  auth_failed: "autenticação",
  model_not_found: "modelo indisponível",
  agent_not_found: "agente ausente",
  rate_limited: "limite do provedor",
  timeout: "timeout",
  budget_blocked: "budget bloqueado",
  binary_missing: "binário ausente",
  cancelled: "cancelado",
  run_failed: "erro de execução",
};

export function failureLabel(code: string): string {
  return FAILURE_LABELS[code] ?? code;
}

export function formatMs(value: number): string {
  if (!Number.isFinite(value) || value <= 0) return "—";
  if (value < 1000) return `${Math.round(value)} ms`;
  if (value < 60_000) return `${(value / 1000).toFixed(1)} s`;
  const minutes = Math.floor(value / 60_000);
  const seconds = Math.round((value % 60_000) / 1000);
  return `${minutes}m ${String(seconds).padStart(2, "0")}s`;
}

export function successTone(rate: number): "green" | "yellow" | "red" {
  if (rate >= 0.9) return "green";
  if (rate >= 0.7) return "yellow";
  return "red";
}

export function dailyMax(daily: DailyMetrics[]): number {
  return Math.max(1, ...daily.map((item) => item.runs));
}

export function barPercent(value: number, max: number): number {
  if (!Number.isFinite(value) || !Number.isFinite(max) || max <= 0) return 0;
  return Math.min(100, Math.max(0, (value / max) * 100));
}

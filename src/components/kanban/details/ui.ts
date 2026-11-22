export const labelClass =
  "mb-1.5 block text-[12px] font-medium uppercase tracking-[0.12em] text-zinc-500";

export function runVariant(status: string): "green" | "red" | "blue" | "yellow" | "default" {
  if (status === "SUCCESS") return "green";
  if (status === "FAILED") return "red";
  if (status === "RUNNING") return "blue";
  if (status === "CANCELLED") return "default";
  return "default";
}

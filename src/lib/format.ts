import { formatDistanceToNow, parseISO } from "date-fns";
import { ptBR } from "date-fns/locale";

export function formatUsd(value: number | null | undefined): string {
  const amount = Number.isFinite(value ?? 0) ? (value ?? 0) : 0;
  return new Intl.NumberFormat("pt-BR", {
    style: "currency",
    currency: "USD",
    minimumFractionDigits: 2,
    maximumFractionDigits: 4,
  }).format(amount);
}

export function formatNumber(value: number | null | undefined): string {
  return new Intl.NumberFormat("pt-BR").format(value ?? 0);
}

export function formatPercent(value: number | null | undefined): string {
  return `${(value ?? 0).toFixed(1)}%`;
}

export function relativeTime(value: string | null | undefined): string {
  if (!value) return "—";
  try {
    return formatDistanceToNow(parseISO(value), { addSuffix: true, locale: ptBR });
  } catch {
    return value;
  }
}

export function shortPath(value: string, max = 48): string {
  if (value.length <= max) return value;
  const head = value.slice(0, Math.ceil(max / 2) - 2);
  const tail = value.slice(-Math.floor(max / 2) + 1);
  return `${head}…${tail}`;
}

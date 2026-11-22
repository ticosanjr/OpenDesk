const ROUTE_LABELS: Array<[string, string]> = [
  ["/search", "Buscar"],
  ["/kanban", "Kanban"],
  ["/calendar", "Calendário"],
  ["/runs/", "Execução"],
  ["/runs", "Execuções"],
  ["/chat", "Chat"],
  ["/artifacts", "Artefatos"],
  ["/files", "Arquivos"],
  ["/routines", "Rotinas"],
  ["/recipes", "Receitas"],
  ["/packs", "Pacotes"],
  ["/studio", "Studio"],
  ["/governance", "Governança"],
  ["/observability", "Observabilidade"],
  ["/learning", "Aprendizado"],
  ["/router", "Roteador"],
  ["/integrations", "Integrações"],
  ["/settings", "Configurações"],
];

export function routeLabel(pathname: string): string {
  if (pathname === "/") return "Dashboard";
  return ROUTE_LABELS.find(([prefix]) => pathname.startsWith(prefix))?.[1] ?? "OpenDesk";
}

const FALLBACK_ACCENT = "#3b82f6";

/** Accept only #rgb/#rrggbb so a bad workspace value cannot break CSS vars. */
export function safeHexColor(value?: string | null): string {
  if (typeof value !== "string") return FALLBACK_ACCENT;
  const trimmed = value.trim();
  if (/^#(?:[0-9a-f]{3}|[0-9a-f]{6})$/i.test(trimmed)) return trimmed;
  return FALLBACK_ACCENT;
}

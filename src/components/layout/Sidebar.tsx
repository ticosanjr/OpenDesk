import type { ReactNode } from "react";
import type { LucideIcon } from "lucide-react";
import { Link, NavLink } from "react-router-dom";
import {
  Activity,
  Archive,
  Brain,
  Loader2,
  CalendarClock,
  CalendarDays,
  ChartNoAxesCombined,
  FolderTree,
  Gauge,
  KanbanSquare,
  MessageSquare,
  Network,
  Package,
  PanelLeftClose,
  PanelLeftOpen,
  Plug,
  Route,
  Search,
  Settings,
  SlidersHorizontal,
  Wand2,
  Workflow,
} from "lucide-react";
import { cn } from "@/lib/utils";
import { queueBadgeLabel, queueCounts, workQueue } from "@/lib/queue";
import { useAppStore } from "@/store/app-store";
import { useUiStore } from "@/store/ui-store";
import { StatusDot, statusTone } from "@/components/ui/status-dot";

interface NavItem {
  to: string;
  label: string;
  icon: LucideIcon;
  end?: boolean;
}

const NAV_GROUPS: Array<{ label: string; items: NavItem[] }> = [
  {
    label: "Operação",
    items: [
      { to: "/", label: "Dashboard", icon: Gauge, end: true },
      { to: "/search", label: "Buscar", icon: Search },
      { to: "/kanban", label: "Kanban & Fila", icon: KanbanSquare },
      { to: "/calendar", label: "Calendário", icon: CalendarDays },
      { to: "/runs", label: "Execuções", icon: Activity },
      { to: "/chat", label: "Chat & Criador", icon: MessageSquare },
      { to: "/routines", label: "Rotinas", icon: CalendarClock },
      { to: "/recipes", label: "Receitas", icon: Workflow },
    ],
  },
  {
    label: "Biblioteca",
    items: [
      { to: "/artifacts", label: "Artefatos", icon: Archive },
      { to: "/files", label: "Arquivos", icon: FolderTree },
      { to: "/packs", label: "Pacotes", icon: Package },
      { to: "/studio", label: "OpenCode Studio", icon: Wand2 },
    ],
  },
  {
    label: "Governança",
    items: [
      { to: "/governance", label: "Agentes & Governança", icon: Network },
      { to: "/observability", label: "Observabilidade", icon: ChartNoAxesCombined },
      { to: "/learning", label: "Aprendizado", icon: Brain },
      { to: "/router", label: "Roteador & Custos", icon: Route },
      { to: "/integrations", label: "Integrações", icon: Plug },
      { to: "/settings", label: "Configurações", icon: Settings },
    ],
  },
];

export const NAV_ITEMS: NavItem[] = NAV_GROUPS.flatMap((group) => group.items);

export function Sidebar() {
  const collapsed = useUiStore((state) => state.sidebarCollapsed);
  const toggleSidebar = useUiStore((state) => state.toggleSidebar);
  const daemon = useAppStore((state) => state.daemon);
  const connected = useAppStore((state) => state.connected);
  const queue = useAppStore((state) => state.queue);
  const counts = queueCounts(workQueue(queue));
  const badge = queueBadgeLabel(counts);

  return (
    <aside
      className={cn(
        "app-sidebar hidden h-full shrink-0 flex-col border-r workspace-divider bg-[#0b0b0d] transition-[width] duration-200 sm:flex",
        collapsed ? "w-[64px]" : "w-[236px]",
      )}
    >
      <div className="sidebar-brand flex h-14 items-center gap-2.5 border-b border-zinc-800/60 px-4">
        <img src="/opendesk.svg" alt="OpenDesk" className="h-7 w-7 shrink-0" />
        {!collapsed ? (
          <div className="min-w-0">
            <p className="truncate text-sm font-semibold tracking-tight text-zinc-100">
              OpenDesk
            </p>
            <p className="truncate font-mono text-[12px] uppercase tracking-[0.14em] text-zinc-500">
              control plane
            </p>
          </div>
        ) : null}
      </div>

      <nav className="sidebar-nav scroll-area flex-1 py-2">
        {NAV_GROUPS.map((group, groupIndex) => (
          <div key={group.label} className="mb-1">
            {!collapsed ? (
              <SidebarSectionLabel>{group.label}</SidebarSectionLabel>
            ) : groupIndex > 0 ? (
              <div className="mx-2 mb-1 mt-2 border-t border-zinc-800/70" />
            ) : null}
            <div className="space-y-0.5">
              {group.items.map((item) => (
                <NavLink
                  key={item.to}
                  to={item.to}
                  end={"end" in item ? item.end : false}
                  title={collapsed ? item.label : undefined}
                  className={({ isActive }) =>
                    cn(
                      "sidebar-nav-item group mx-2 flex items-center gap-2.5 rounded-lg px-2.5 py-1.5 text-[14px] font-medium transition-colors",
                      isActive
                        ? "workspace-nav-active text-zinc-100"
                        : "text-zinc-500 hover:bg-zinc-900/70 hover:text-zinc-200",
                      collapsed && "justify-center px-0",
                    )
                  }
                >
                  <span className="relative shrink-0">
                    <item.icon size={17} strokeWidth={1.8} />
                    {item.to === "/kanban" && counts.running > 0 ? (
                      <span className="absolute -right-1 -top-1 h-2 w-2 animate-pulse rounded-full bg-blue-400" />
                    ) : null}
                  </span>
                  {!collapsed ? <span className="truncate">{item.label}</span> : null}
                  {!collapsed && item.to === "/kanban" && counts.running > 0 ? (
                    <span
                      className="ml-auto inline-flex items-center gap-1 font-mono text-[11px] text-blue-300"
                      title={`${counts.running} execução(ões) em andamento`}
                    >
                      <Loader2 size={10} className="animate-spin" />
                      {counts.running}
                    </span>
                  ) : null}
                </NavLink>
              ))}
            </div>
          </div>
        ))}
      </nav>

      <div className="sidebar-footer border-t border-zinc-800/60 p-2.5">
        {!collapsed ? (
          <div className="sidebar-status-card mb-2 rounded-lg border border-zinc-800/70 bg-zinc-900/30 p-2.5">
            <div className="flex items-center gap-2">
              <StatusDot tone={statusTone(daemon?.online ?? false, daemon?.state)} />
              <span className="text-[13px] font-medium text-zinc-300">
                {daemon?.online ? `OpenCode :${daemon.port}` : "OpenCode offline"}
              </span>
            </div>
            <div className="mt-2 flex items-center justify-between font-mono text-[12px] text-zinc-500">
              <span>{daemon?.version ? `v${daemon.version}` : "—"}</span>
              <span className="inline-flex items-center gap-1">
                <span
                  className={cn(
                    "h-1.5 w-1.5 rounded-full",
                    connected ? "bg-emerald-400" : "bg-zinc-600",
                  )}
                />
                ws
              </span>
            </div>
            {badge ? (
              <Link
                to="/kanban?view=queue"
                className="mt-2 flex flex-col gap-0.5 rounded-md border border-blue-500/20 bg-blue-500/[0.06] px-2 py-1 text-[11.5px] text-blue-200"
                title={badge}
              >
                {counts.waiting > 0 ? (
                  <span className="truncate">
                    {counts.waiting} aguardando
                  </span>
                ) : null}
                {counts.running > 0 ? (
                  <span className="flex min-w-0 items-center gap-1 text-blue-300">
                    <Loader2 size={10} className="shrink-0 animate-spin" />
                    <span className="truncate">
                      {counts.running} executando
                    </span>
                  </span>
                ) : null}
              </Link>
            ) : null}
          </div>
        ) : null}

        <button
          type="button"
          onClick={toggleSidebar}
          className="flex w-full items-center justify-center gap-2 rounded-lg px-2 py-1.5 text-[13px] text-zinc-500 transition-colors hover:bg-zinc-900 hover:text-zinc-300"
          title={collapsed ? "Expandir sidebar" : "Recolher sidebar"}
          aria-label={collapsed ? "Expandir sidebar" : "Recolher sidebar"}
        >
          {collapsed ? <PanelLeftOpen size={16} /> : <PanelLeftClose size={16} />}
          {!collapsed ? <span>Recolher</span> : null}
        </button>
      </div>
    </aside>
  );
}

export function SidebarSectionLabel({ children }: { children: ReactNode }) {
  return (
    <p className="sidebar-section-label px-2.5 pb-1 pt-2 font-mono text-[11.5px] uppercase tracking-[0.16em] text-zinc-500">
      {children}
    </p>
  );
}

export function SettingsShortcut() {
  return (
    <NavLink
      to="/router"
      className="flex items-center gap-2 rounded-lg px-2.5 py-2 text-[14px] font-medium text-zinc-500 transition-colors hover:bg-zinc-900/70 hover:text-zinc-200"
    >
      <SlidersHorizontal size={17} strokeWidth={1.8} />
      Configurações
    </NavLink>
  );
}

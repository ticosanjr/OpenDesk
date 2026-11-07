import type { ReactNode } from "react";
import type { LucideIcon } from "lucide-react";
import { NavLink } from "react-router-dom";
import {
  Activity,
  Archive,
  CalendarClock,
  FolderTree,
  Gauge,
  KanbanSquare,
  MessageSquare,
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
import { useAppStore } from "@/store/app-store";
import { useUiStore } from "@/store/ui-store";
import { StatusDot, statusTone } from "@/components/ui/status-dot";
import { Badge } from "@/components/ui/badge";

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

  return (
    <aside
      className={cn(
        "flex h-full shrink-0 flex-col border-r workspace-divider bg-[#0b0b0d] transition-[width] duration-200",
        collapsed ? "w-[64px]" : "w-[236px]",
      )}
    >
      <div className="flex h-14 items-center gap-2.5 border-b border-zinc-800/60 px-4">
        <img src="/opendesk.svg" alt="OpenDesk" className="h-7 w-7 shrink-0" />
        {!collapsed ? (
          <div className="min-w-0">
            <p className="truncate text-sm font-semibold tracking-tight text-zinc-100">
              OpenDesk
            </p>
            <p className="truncate font-mono text-[10px] uppercase tracking-[0.14em] text-zinc-600">
              control plane
            </p>
          </div>
        ) : null}
      </div>

      <nav className="flex-1 overflow-y-auto p-2">
        {NAV_GROUPS.map((group) => (
          <div key={group.label} className="mb-1">
            {!collapsed ? <SidebarSectionLabel>{group.label}</SidebarSectionLabel> : null}
            <div className="space-y-0.5">
              {group.items.map((item) => (
                <NavLink
                  key={item.to}
                  to={item.to}
                  end={"end" in item ? item.end : false}
                  title={collapsed ? item.label : undefined}
                  className={({ isActive }) =>
                    cn(
                      "group flex items-center gap-2.5 rounded-lg px-2.5 py-2 text-[13px] font-medium transition-colors",
                      isActive
                        ? "workspace-nav-active text-zinc-100"
                        : "text-zinc-500 hover:bg-zinc-900/70 hover:text-zinc-200",
                      collapsed && "justify-center px-0",
                    )
                  }
                >
                  <item.icon size={17} strokeWidth={1.8} className="shrink-0" />
                  {!collapsed ? <span className="truncate">{item.label}</span> : null}
                </NavLink>
              ))}
            </div>
          </div>
        ))}
      </nav>

      <div className="border-t border-zinc-800/60 p-2.5">
        {!collapsed ? (
          <div className="mb-2 rounded-lg border border-zinc-800/70 bg-zinc-900/30 p-2.5">
            <div className="flex items-center gap-2">
              <StatusDot tone={statusTone(daemon?.online ?? false, daemon?.state)} />
              <span className="text-[11px] font-medium text-zinc-300">
                {daemon?.online ? `OpenCode :${daemon.port}` : "OpenCode offline"}
              </span>
            </div>
            <div className="mt-2 flex items-center justify-between font-mono text-[10px] text-zinc-600">
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
            {queue.length > 0 ? (
              <div className="mt-2">
                <Badge variant="blue">{queue.length} na fila</Badge>
              </div>
            ) : null}
          </div>
        ) : null}

        <button
          type="button"
          onClick={toggleSidebar}
          className="flex w-full items-center justify-center gap-2 rounded-lg px-2 py-1.5 text-[11px] text-zinc-600 transition-colors hover:bg-zinc-900 hover:text-zinc-300"
          title={collapsed ? "Expandir sidebar" : "Recolher sidebar"}
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
    <p className="px-2.5 pb-1 pt-3 font-mono text-[9px] uppercase tracking-[0.16em] text-zinc-700">
      {children}
    </p>
  );
}

export function SettingsShortcut() {
  return (
    <NavLink
      to="/router"
      className="flex items-center gap-2 rounded-lg px-2.5 py-2 text-[13px] font-medium text-zinc-500 transition-colors hover:bg-zinc-900/70 hover:text-zinc-200"
    >
      <SlidersHorizontal size={17} strokeWidth={1.8} />
      Configurações
    </NavLink>
  );
}

import { Activity, Camera, Inbox, MessageSquare } from "lucide-react";
import { NavLink } from "react-router-dom";
import { cn } from "@/lib/utils";

const ITEMS = [
  { to: "/kanban", label: "Caixa", icon: Inbox, end: true },
  { to: "/chat", label: "Chat", icon: MessageSquare, end: false },
  { to: "/capture", label: "Capturar", icon: Camera, end: false },
  { to: "/runs", label: "Runs", icon: Activity, end: false },
];

/// Mobile-first navigation: four destinations that cover inbox/HITL, chat and
/// voice, capture and execution. Hidden from tablet/desktop where the sidebar
/// is available.
export function MobileNav() {
  return (
    <nav
      aria-label="Navegação mobile"
      className="fixed inset-x-0 bottom-0 z-40 border-t border-zinc-800/80 bg-[#0b0b0d]/95 pb-[env(safe-area-inset-bottom)] backdrop-blur sm:hidden"
      data-testid="mobile-nav"
    >
      <div className="grid h-16 grid-cols-4">
        {ITEMS.map((item) => (
          <NavLink
            key={item.to}
            to={item.to}
            end={item.end}
            className={({ isActive }) =>
              cn(
                "flex min-w-0 flex-col items-center justify-center gap-1 text-[11.5px] font-medium transition-colors",
                isActive
                  ? "text-blue-300"
                  : "text-zinc-500 hover:text-zinc-300",
              )
            }
          >
            {({ isActive }) => (
              <>
                <span
                  className={cn(
                    "flex h-7 w-10 items-center justify-center rounded-full",
                    isActive ? "bg-blue-500/15" : undefined,
                  )}
                >
                  <item.icon size={19} strokeWidth={isActive ? 2.2 : 1.8} />
                </span>
                <span className="truncate">{item.label}</span>
              </>
            )}
          </NavLink>
        ))}
      </div>
    </nav>
  );
}

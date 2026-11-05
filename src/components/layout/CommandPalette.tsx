import { useEffect } from "react";
import { Command } from "cmdk";
import { useNavigate } from "react-router-dom";
import {
  Archive,
  CalendarClock,
  FolderTree,
  Gauge,
  KanbanSquare,
  MessageSquare,
  Plug,
  Plus,
  RefreshCw,
  Route,
  RotateCw,
  Search,
  Wand2,
} from "lucide-react";
import { useUiStore } from "@/store/ui-store";
import { useAppStore } from "@/store/app-store";
import { NAV_ITEMS } from "@/components/layout/Sidebar";
import { cn } from "@/lib/utils";

const icons = {
  "/": Gauge,
  "/kanban": KanbanSquare,
  "/chat": MessageSquare,
  "/artifacts": Archive,
  "/files": FolderTree,
  "/routines": CalendarClock,
  "/studio": Wand2,
  "/router": Route,
  "/integrations": Plug,
} as const;

export function CommandPalette() {
  const open = useUiStore((state) => state.commandOpen);
  const setOpen = useUiStore((state) => state.setCommandOpen);
  const setWorkspaceDialogOpen = useUiStore(
    (state) => state.setWorkspaceDialogOpen,
  );
  const refresh = useAppStore((state) => state.refresh);
  const restartDaemon = useAppStore((state) => state.restartDaemon);
  const navigate = useNavigate();

  useEffect(() => {
    if (!open) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") setOpen(false);
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [open, setOpen]);

  if (!open) return null;

  function run(action: () => void) {
    setOpen(false);
    action();
  }

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center px-4 pt-[15vh]">
      <button
        type="button"
        aria-label="Fechar command bar"
        className="absolute inset-0 bg-black/70 backdrop-blur-[2px]"
        onClick={() => setOpen(false)}
      />
      <Command
        label="Command Menu"
        className="relative w-full max-w-xl overflow-hidden rounded-xl border border-zinc-800 bg-zinc-950 shadow-2xl"
      >
        <div className="flex items-center gap-2 border-b border-zinc-800/70 px-3">
          <Search size={15} className="text-zinc-500" />
          <Command.Input
            autoFocus
            placeholder="Ir para, criar workspace ou reiniciar o engine…"
            className="h-11 flex-1 bg-transparent text-sm text-zinc-100 outline-none placeholder:text-zinc-600"
          />
          <span className="rounded border border-zinc-800 bg-zinc-900 px-1.5 py-0.5 font-mono text-[9px] text-zinc-600">
            ESC
          </span>
        </div>

        <Command.List className="max-h-[340px] overflow-y-auto p-2">
          <Command.Empty className="px-3 py-8 text-center text-xs text-zinc-500">
            Nenhum resultado.
          </Command.Empty>

          <Command.Group
            heading="Navegação"
            className="[&_[cmdk-group-heading]]:px-2 [&_[cmdk-group-heading]]:py-1.5 [&_[cmdk-group-heading]]:font-mono [&_[cmdk-group-heading]]:text-[9px] [&_[cmdk-group-heading]]:uppercase [&_[cmdk-group-heading]]:tracking-[0.16em] [&_[cmdk-group-heading]]:text-zinc-600"
          >
            {NAV_ITEMS.map((item) => {
              const Icon = icons[item.to as keyof typeof icons] ?? Gauge;
              return (
                <Command.Item
                  key={item.to}
                  value={item.label}
                  onSelect={() => run(() => navigate(item.to))}
                  className={cn(
                    "flex cursor-pointer items-center gap-2.5 rounded-lg px-2.5 py-2 text-[13px] text-zinc-300 outline-none",
                    "data-[selected=true]:bg-zinc-800/80 data-[selected=true]:text-zinc-100",
                  )}
                >
                  <Icon size={15} className="text-zinc-500" />
                  {item.label}
                </Command.Item>
              );
            })}
          </Command.Group>

          <Command.Group
            heading="Ações"
            className="mt-2 [&_[cmdk-group-heading]]:px-2 [&_[cmdk-group-heading]]:py-1.5 [&_[cmdk-group-heading]]:font-mono [&_[cmdk-group-heading]]:text-[9px] [&_[cmdk-group-heading]]:uppercase [&_[cmdk-group-heading]]:tracking-[0.16em] [&_[cmdk-group-heading]]:text-zinc-600"
          >
            <Command.Item
              value="Novo workspace"
              onSelect={() =>
                run(() => setWorkspaceDialogOpen(true))
              }
              className="flex cursor-pointer items-center gap-2.5 rounded-lg px-2.5 py-2 text-[13px] text-zinc-300 outline-none data-[selected=true]:bg-zinc-800/80 data-[selected=true]:text-zinc-100"
            >
              <Plus size={15} className="text-zinc-500" />
              Novo workspace
            </Command.Item>
            <Command.Item
              value="Reiniciar OpenCode Daemon"
              onSelect={() => run(() => void restartDaemon())}
              className="flex cursor-pointer items-center gap-2.5 rounded-lg px-2.5 py-2 text-[13px] text-zinc-300 outline-none data-[selected=true]:bg-zinc-800/80 data-[selected=true]:text-zinc-100"
            >
              <RotateCw size={15} className="text-zinc-500" />
              Reiniciar OpenCode Daemon
            </Command.Item>
            <Command.Item
              value="Atualizar dados"
              onSelect={() => run(() => void refresh())}
              className="flex cursor-pointer items-center gap-2.5 rounded-lg px-2.5 py-2 text-[13px] text-zinc-300 outline-none data-[selected=true]:bg-zinc-800/80 data-[selected=true]:text-zinc-100"
            >
              <RefreshCw size={15} className="text-zinc-500" />
              Atualizar dados
            </Command.Item>
          </Command.Group>
        </Command.List>

        <div className="flex items-center justify-between border-t border-zinc-800/70 px-3 py-2 font-mono text-[9px] text-zinc-600">
          <span>↑↓ navegar · ↵ selecionar</span>
          <span>OpenDesk</span>
        </div>
      </Command>
    </div>
  );
}

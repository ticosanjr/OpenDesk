import { useEffect } from "react";
import { Outlet } from "react-router-dom";
import { AlertTriangle, RefreshCw } from "lucide-react";
import { Sidebar } from "@/components/layout/Sidebar";
import { Topbar } from "@/components/layout/Topbar";
import { CommandPalette } from "@/components/layout/CommandPalette";
import { WorkspaceDialog } from "@/components/layout/WorkspaceDialog";
import { Button } from "@/components/ui/button";
import { useAppStore } from "@/store/app-store";
import { useUiStore } from "@/store/ui-store";

export function AppShell() {
  const boot = useAppStore((state) => state.boot);
  const loading = useAppStore((state) => state.loading);
  const error = useAppStore((state) => state.error);
  const setCommandOpen = useUiStore((state) => state.setCommandOpen);

  useEffect(() => {
    void boot();
  }, [boot]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        setCommandOpen(!useUiStore.getState().commandOpen);
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [setCommandOpen]);

  return (
    <div className="flex h-screen w-screen overflow-hidden bg-[#09090b] text-zinc-100">
      <Sidebar />
      <div className="flex min-w-0 flex-1 flex-col">
        <Topbar />
        {loading ? (
          <div className="h-0.5 w-full overflow-hidden bg-zinc-900">
            <div className="h-full w-1/3 animate-[loading_1.2s_ease-in-out_infinite] bg-blue-500/70" />
          </div>
        ) : null}
        {error ? (
          <div className="flex items-center gap-3 border-b border-red-500/20 bg-red-500/10 px-4 py-2 text-[11px] text-red-300">
            <AlertTriangle size={14} />
            <span className="flex-1">{error}</span>
            <Button
              variant="destructive"
              size="sm"
              onClick={() => void boot()}
              className="h-7"
            >
              <RefreshCw size={12} />
              Tentar novamente
            </Button>
          </div>
        ) : null}
        <main className="min-h-0 flex-1 overflow-y-auto">
          <Outlet />
        </main>
      </div>
      <CommandPalette />
      <WorkspaceDialog />
    </div>
  );
}

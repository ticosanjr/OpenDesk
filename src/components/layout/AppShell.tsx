import { Suspense, useEffect, useMemo, useRef, useState, type CSSProperties } from "react";
import { Outlet, useLocation, useNavigate } from "react-router-dom";
import { Sidebar } from "@/components/layout/Sidebar";
import { MobileNav } from "@/components/layout/MobileNav";
import { Topbar } from "@/components/layout/Topbar";
import { CommandPalette } from "@/components/layout/CommandPalette";
import { WorkspaceDialog } from "@/components/layout/WorkspaceDialog";
import { ConfirmDialog } from "@/components/ui/confirm-dialog";
import { GlobalHitlModal } from "@/components/interactions/GlobalHitlModal";
import { ErrorBoundary } from "@/components/ui/error-boundary";
import { Toaster } from "@/components/ui/toaster";
import { useAppStore } from "@/store/app-store";
import { useUiStore } from "@/store/ui-store";
import { isEditableTarget } from "@/lib/keyboard";
import { AppBanners } from "@/components/layout/app-shell/AppBanners";
import { ShortcutsModal } from "@/components/layout/app-shell/ShortcutsModal";
import { routeLabel, safeHexColor } from "@/components/layout/app-shell/helpers";
import { isTauriRuntime } from "@/lib/runtime-env";
import { useAppNotifications } from "@/components/layout/app-shell/use-app-notifications";

export { safeHexColor } from "@/components/layout/app-shell/helpers";

export function AppShell() {
  const mainRef = useRef<HTMLElement | null>(null);
  const autoCollapsedRef = useRef(false);
  const boot = useAppStore((state) => state.boot);
  const loading = useAppStore((state) => state.loading);
  const error = useAppStore((state) => state.error);
  const workspaces = useAppStore((state) => state.workspaces);
  const activeWorkspaceId = useAppStore((state) => state.activeWorkspaceId);
  const daemon = useAppStore((state) => state.daemon);
  const restartDaemon = useAppStore((state) => state.restartDaemon);
  const refresh = useAppStore((state) => state.refresh);
  const setCommandOpen = useUiStore((state) => state.setCommandOpen);
  const setSidebarCollapsed = useUiStore((state) => state.setSidebarCollapsed);
  const setTaskDialogOpen = useUiStore((state) => state.setTaskDialogOpen);
  const navigate = useNavigate();
  const budgetAlert = useAppStore((state) => state.budgetAlert);
  const location = useLocation();
  const [dismissedBudget, setDismissedBudget] = useState<string | null>(null);
  const [retryingEngine, setRetryingEngine] = useState(false);
  const [shortcutsOpen, setShortcutsOpen] = useState(false);

  useAppNotifications();

  useEffect(() => {
    setDismissedBudget(null);
  }, [budgetAlert?.message]);

  // At the app's compact width the fixed sidebar eats too much of the view.
  // Collapse automatically when entering that width; the manual toggle keeps
  // working and the persisted preference is not overwritten.
  useEffect(() => {
    const query = window.matchMedia("(max-width: 1199px)");
    const apply = () => {
      if (query.matches) {
        if (!useUiStore.getState().sidebarCollapsed) {
          autoCollapsedRef.current = true;
          setSidebarCollapsed(true);
        }
      } else if (autoCollapsedRef.current) {
        autoCollapsedRef.current = false;
        setSidebarCollapsed(false);
      }
    };
    apply();
    query.addEventListener("change", apply);
    return () => query.removeEventListener("change", apply);
  }, [setSidebarCollapsed]);

  useEffect(() => {
    mainRef.current?.scrollTo({ top: 0 });
  }, [location.pathname]);

  const engineOffline =
    daemon !== null &&
    !daemon.online &&
    !["starting", "restarting"].includes(daemon.state);

  useEffect(() => {
    void boot();
  }, [boot]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.defaultPrevented) return;
      const editable = isEditableTarget(event.target);
      const modifier = event.metaKey || event.ctrlKey;
      if (modifier && event.key.toLowerCase() === "k") {
        event.preventDefault();
        setCommandOpen(!useUiStore.getState().commandOpen);
        return;
      }
      // Never steal printable keys (?, /, letters, space) while the user is
      // typing in a field, textarea, select or contenteditable.
      if (editable) return;
      if (modifier && event.key.toLowerCase() === "n") {
        event.preventDefault();
        navigate("/kanban");
        setTaskDialogOpen(true);
        return;
      }
      if (
        modifier &&
        event.shiftKey &&
        event.code === "Space" &&
        !isTauriRuntime()
      ) {
        event.preventDefault();
        navigate("/capture");
        return;
      }
      if (
        modifier &&
        event.shiftKey &&
        event.key.toLowerCase() === "r"
      ) {
        event.preventDefault();
        void refresh();
        return;
      }
      if (event.key === "?" && !modifier && !event.altKey) {
        event.preventDefault();
        setShortcutsOpen((current) => !current);
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [navigate, refresh, setCommandOpen, setTaskDialogOpen]);

  const activeWorkspace = workspaces.find(
    (workspace) => workspace.id === activeWorkspaceId,
  );
  const accent = safeHexColor(activeWorkspace?.color_hex);
  const workspaceStyle = useMemo(
    () =>
      ({
        "--workspace-accent": accent,
        "--workspace-border": `color-mix(in srgb, ${accent} 26%, #27272a)`,
        "--workspace-border-soft": `color-mix(in srgb, ${accent} 14%, #27272a)`,
        "--workspace-glow": `color-mix(in srgb, ${accent} 10%, transparent)`,
        "--workspace-surface": `color-mix(in srgb, ${accent} 7%, #18181b)`,
      }) as CSSProperties,
    [accent],
  );

  useEffect(() => {
    const label = routeLabel(location.pathname);
    document.title = activeWorkspace?.name
      ? `${label} · ${activeWorkspace.name} · OpenDesk`
      : `${label} · OpenDesk`;
  }, [activeWorkspace?.name, location.pathname]);

  return (
    <div
      className="workspace-scope flex h-dvh w-screen overflow-hidden bg-[#09090b] text-zinc-100"
      style={workspaceStyle}
    >
      <Sidebar />
      <div className="flex min-w-0 flex-1 flex-col">
        <Topbar />
        <AppBanners
          loading={loading}
          error={error}
          engineOffline={engineOffline}
          daemonMessage={daemon?.message}
          budgetAlert={budgetAlert}
          dismissedBudget={dismissedBudget}
          retryingEngine={retryingEngine}
          onRetryBoot={() => {
            setRetryingEngine(true);
            void boot().finally(() => setRetryingEngine(false));
          }}
          onConfigureEngine={() => navigate("/settings?section=engine")}
          onAdjustBudget={() => navigate("/router")}
          onRestartDaemon={() => {
            setRetryingEngine(true);
            void restartDaemon()
              .finally(() => void refresh())
              .finally(() => setRetryingEngine(false));
          }}
          onDismissBudget={setDismissedBudget}
        />
        <main
          ref={mainRef}
          className="app-main min-h-0 flex-1 pb-[calc(4rem+env(safe-area-inset-bottom))] sm:pb-0"
        >
          <Suspense
            fallback={
              <div className="space-y-3 p-5 lg:p-7">
                <div className="h-6 w-48 animate-pulse rounded bg-zinc-800/70" />
                <div className="h-24 animate-pulse rounded-xl border border-zinc-800/60 bg-zinc-900/30" />
                <div className="grid gap-3 md:grid-cols-2">
                  <div className="h-40 animate-pulse rounded-xl border border-zinc-800/60 bg-zinc-900/30" />
                  <div className="h-40 animate-pulse rounded-xl border border-zinc-800/60 bg-zinc-900/30" />
                </div>
              </div>
            }
          >
            <ErrorBoundary resetKey={location.pathname}>
              <Outlet />
            </ErrorBoundary>
          </Suspense>
        </main>
      </div>
      <MobileNav />
      <CommandPalette />
      <WorkspaceDialog />
      <ConfirmDialog />
      <GlobalHitlModal />
      <ShortcutsModal
        open={shortcutsOpen}
        onClose={() => setShortcutsOpen(false)}
      />
      <Toaster />
    </div>
  );
}

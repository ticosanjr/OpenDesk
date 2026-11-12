import { create } from "zustand";

interface UiStore {
  commandOpen: boolean;
  workspaceDialogOpen: boolean;
  taskDialogOpen: boolean;
  sidebarCollapsed: boolean;
  setCommandOpen: (open: boolean) => void;
  setWorkspaceDialogOpen: (open: boolean) => void;
  setTaskDialogOpen: (open: boolean) => void;
  toggleSidebar: () => void;
  setSidebarCollapsed: (collapsed: boolean, persist?: boolean) => void;
}

const SIDEBAR_KEY = "opendesk.sidebarCollapsed";

function readSidebarCollapsed(): boolean {
  try {
    return window.localStorage.getItem(SIDEBAR_KEY) === "1";
  } catch {
    return false;
  }
}

export const useUiStore = create<UiStore>((set) => ({
  commandOpen: false,
  workspaceDialogOpen: false,
  taskDialogOpen: false,
  sidebarCollapsed: readSidebarCollapsed(),
  setCommandOpen: (open) => set({ commandOpen: open }),
  setWorkspaceDialogOpen: (open) => set({ workspaceDialogOpen: open }),
  setTaskDialogOpen: (open) => set({ taskDialogOpen: open }),
  toggleSidebar: () =>
    set((state) => {
      const sidebarCollapsed = !state.sidebarCollapsed;
      try {
        window.localStorage.setItem(SIDEBAR_KEY, sidebarCollapsed ? "1" : "0");
      } catch {
        // A blocked localStorage must not break navigation.
      }
      return { sidebarCollapsed };
    }),
  setSidebarCollapsed: (sidebarCollapsed, persist = false) => {
    if (persist) {
      try {
        window.localStorage.setItem(SIDEBAR_KEY, sidebarCollapsed ? "1" : "0");
      } catch {
        // A blocked localStorage must not break navigation.
      }
    }
    set({ sidebarCollapsed });
  },
}));

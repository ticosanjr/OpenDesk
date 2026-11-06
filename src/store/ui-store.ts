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
}

export const useUiStore = create<UiStore>((set) => ({
  commandOpen: false,
  workspaceDialogOpen: false,
  taskDialogOpen: false,
  sidebarCollapsed: false,
  setCommandOpen: (open) => set({ commandOpen: open }),
  setWorkspaceDialogOpen: (open) => set({ workspaceDialogOpen: open }),
  setTaskDialogOpen: (open) => set({ taskDialogOpen: open }),
  toggleSidebar: () => set((state) => ({ sidebarCollapsed: !state.sidebarCollapsed })),
}));

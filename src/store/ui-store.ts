import { create } from "zustand";

interface UiStore {
  commandOpen: boolean;
  workspaceDialogOpen: boolean;
  sidebarCollapsed: boolean;
  setCommandOpen: (open: boolean) => void;
  setWorkspaceDialogOpen: (open: boolean) => void;
  toggleSidebar: () => void;
}

export const useUiStore = create<UiStore>((set) => ({
  commandOpen: false,
  workspaceDialogOpen: false,
  sidebarCollapsed: false,
  setCommandOpen: (open) => set({ commandOpen: open }),
  setWorkspaceDialogOpen: (open) => set({ workspaceDialogOpen: open }),
  toggleSidebar: () => set((state) => ({ sidebarCollapsed: !state.sidebarCollapsed })),
}));

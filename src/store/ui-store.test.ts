import { beforeEach, describe, expect, it } from "vitest";
import { useUiStore } from "./ui-store";

describe("ui store", () => {
  beforeEach(() => {
    useUiStore.setState({
      commandOpen: false,
      workspaceDialogOpen: false,
      taskDialogOpen: false,
      sidebarCollapsed: false,
    });
  });

  it("toggles sidebar and dialogs", () => {
    useUiStore.getState().toggleSidebar();
    expect(useUiStore.getState().sidebarCollapsed).toBe(true);
    useUiStore.getState().toggleSidebar();
    expect(useUiStore.getState().sidebarCollapsed).toBe(false);

    useUiStore.getState().setCommandOpen(true);
    useUiStore.getState().setWorkspaceDialogOpen(true);
    useUiStore.getState().setTaskDialogOpen(true);
    expect(useUiStore.getState()).toMatchObject({
      commandOpen: true,
      workspaceDialogOpen: true,
      taskDialogOpen: true,
    });
  });
});

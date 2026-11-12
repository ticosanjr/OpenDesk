import { beforeEach, describe, expect, it, vi } from "vitest";
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

  it("persists explicit sidebar changes when requested", () => {
    useUiStore.getState().setSidebarCollapsed(true, true);
    expect(useUiStore.getState().sidebarCollapsed).toBe(true);
    expect(localStorage.getItem("opendesk.sidebarCollapsed")).toBe("1");
    useUiStore.getState().setSidebarCollapsed(false, true);
    expect(localStorage.getItem("opendesk.sidebarCollapsed")).toBe("0");
    useUiStore.getState().setSidebarCollapsed(true);
    expect(useUiStore.getState().sidebarCollapsed).toBe(true);
    expect(localStorage.getItem("opendesk.sidebarCollapsed")).toBe("0");
  });

  it("survives blocked storage", () => {
    vi.stubGlobal("localStorage", undefined);
    expect(() => useUiStore.getState().toggleSidebar()).not.toThrow();
    expect(() => {
      useUiStore.getState().setSidebarCollapsed(false, true);
    }).not.toThrow();
    vi.unstubAllGlobals();
  });

  it("reads a persisted collapsed value at module load", async () => {
    localStorage.setItem("opendesk.sidebarCollapsed", "1");
    vi.resetModules();
    const { useUiStore: fresh } = await import("./ui-store");
    expect(fresh.getState().sidebarCollapsed).toBe(true);
  });

  it("falls back to expanded when storage read fails", async () => {
    vi.resetModules();
    vi.stubGlobal("localStorage", {
      getItem: () => {
        throw new Error("blocked");
      },
    });
    const { useUiStore: fresh } = await import("./ui-store");
    expect(fresh.getState().sidebarCollapsed).toBe(false);
    vi.unstubAllGlobals();
  });
});

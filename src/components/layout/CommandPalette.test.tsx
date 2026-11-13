import { fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";

const apiGet = vi.hoisted(() => vi.fn());
vi.mock("@/lib/api", () => ({ apiGet }));

import { CommandPalette } from "./CommandPalette";
import { useUiStore } from "@/store/ui-store";
import { useAppStore } from "@/store/app-store";
import { RECENT_SEARCH_KEY } from "@/lib/recent-searches";

describe("CommandPalette", () => {
  beforeEach(() => {
    apiGet.mockReset();
    localStorage.clear();
    useAppStore.setState({ activeWorkspaceId: "ws-1" });
    useUiStore.setState({ commandOpen: true });
  });

  it("uses the same workspace search API as the search page", async () => {
    apiGet.mockResolvedValue([
      {
        id: "task-1",
        kind: "task",
        title: "Diagnóstico Aurora",
        snippet: "briefing",
        updated_at: "2026-11-13T08:00:00Z",
        link: "/kanban?task=task-1",
      },
    ]);

    render(
      <MemoryRouter>
        <CommandPalette />
      </MemoryRouter>,
    );

    fireEvent.change(screen.getByPlaceholderText(/Buscar em todo o workspace/), {
      target: { value: "diagnóstico" },
    });

    expect(await screen.findByText("Diagnóstico Aurora")).toBeInTheDocument();
    expect(apiGet).toHaveBeenCalledWith(
      "/api/search?workspace_id=ws-1&q=diagn%C3%B3stico",
    );
  });

  it("renders hits the server matched outside the title", async () => {
    apiGet.mockResolvedValue([
      {
        id: "agent-1",
        kind: "agent",
        title: "Global Agent",
        snippet: "agente workspace · global-agent",
        updated_at: null,
        link: "/studio",
      },
    ]);

    render(
      <MemoryRouter>
        <CommandPalette />
      </MemoryRouter>,
    );

    fireEvent.change(screen.getByPlaceholderText(/Buscar em todo o workspace/), {
      target: { value: "fixture" },
    });

    expect(await screen.findByText("Global Agent")).toBeInTheDocument();
  });

  it("shows recent searches saved from an earlier query", async () => {
    localStorage.setItem(RECENT_SEARCH_KEY, JSON.stringify(["relatório"]));
    render(
      <MemoryRouter>
        <CommandPalette />
      </MemoryRouter>,
    );

    expect(await screen.findByText("Buscas recentes")).toBeInTheDocument();
    fireEvent.click(screen.getByText("relatório"));
    expect(
      screen.getByPlaceholderText(/Buscar em todo o workspace/),
    ).toHaveValue("relatório");
  });
});

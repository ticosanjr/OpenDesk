import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@/lib/api", () => ({
  apiGet: vi.fn(),
  apiPost: vi.fn(),
  apiPut: vi.fn(),
}));
vi.mock("@/store/toast-store", () => ({
  toast: vi.fn(),
}));

import { apiGet, apiPost } from "@/lib/api";
import { RecipesPage } from "./RecipesPage";
import { useAppStore } from "@/store/app-store";

const yaml = `id: receita-e2e
name: Receita E2E
enabled: true
trigger:
  type: MANUAL
  config: {}
steps:
  - id: passo-1
    name: Passo 1
    mode: build
    prompt: oi
hitl_checkpoints: []
sinks: []
`;

describe("RecipesPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useAppStore.setState({ activeWorkspaceId: "ws-1" });
    (apiGet as unknown as ReturnType<typeof vi.fn>).mockImplementation(
      async (path: string) => {
        if (path.startsWith("/api/recipes/runs")) return { runs: [] };
        if (path.startsWith("/api/recipes?")) {
          return {
            recipes: [
              {
                id: "receita-e2e",
                name: "Receita E2E",
                trigger_type: "MANUAL",
                is_enabled: true,
                total_runs: 0,
              },
            ],
          };
        }
        return { yaml };
      },
    );
    (apiPost as unknown as ReturnType<typeof vi.fn>).mockResolvedValue({
      id: "run-1",
      status: "RUNNING_STEP",
    });
  });

  it("loads a recipe, renders the stepper and starts a dry-run", async () => {
    render(
      <MemoryRouter>
        <RecipesPage />
      </MemoryRouter>,
    );
    await waitFor(() => expect(screen.getByText("Receita E2E")).toBeInTheDocument());
    expect(screen.getByText("Fluxo visual")).toBeInTheDocument();
    await waitFor(() =>
      expect(screen.getByDisplayValue("Passo 1")).toBeInTheDocument(),
    );
    fireEvent.click(screen.getByRole("button", { name: /Dry-run/ }));
    await waitFor(() =>
      expect(apiPost).toHaveBeenCalledWith("/api/recipes/receita-e2e/run", {
        dry_run: true,
        trigger_context: { manual: true },
      }),
    );
  });
});

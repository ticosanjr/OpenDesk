import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@/lib/api", () => ({
  apiDelete: vi.fn(),
  apiGet: vi.fn(),
  apiPost: vi.fn(),
  apiPut: vi.fn(),
}));
vi.mock("@/store/toast-store", () => ({
  toast: vi.fn(),
}));

import { apiDelete, apiGet, apiPost, apiPut } from "@/lib/api";
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
    (apiDelete as unknown as ReturnType<typeof vi.fn>).mockResolvedValue({
      ok: true,
    });
  });

  it("loads a recipe, renders the stepper and starts a dry-run", async () => {
    render(
      <MemoryRouter>
        <RecipesPage />
      </MemoryRouter>,
    );
    await waitFor(() =>
      expect(screen.getByText("Receita E2E")).toBeInTheDocument(),
    );
    expect(screen.getByText("Crie ou selecione uma receita")).toBeInTheDocument();
    fireEvent.click(screen.getByText("Receita E2E"));
    expect(await screen.findByText("Fluxo visual")).toBeInTheDocument();
    await waitFor(() =>
      expect(screen.getByDisplayValue("Passo 1")).toBeInTheDocument(),
    );
    fireEvent.click(screen.getByRole("button", { name: /Dry-run/ }));
    fireEvent.change(screen.getByPlaceholderText(/analise o contrato/), {
      target: { value: "analise o briefing" },
    });
    fireEvent.click(screen.getByRole("button", { name: /Iniciar dry-run/ }));
    await waitFor(() =>
      expect(apiPost).toHaveBeenCalledWith(
        "/api/recipes/receita-e2e/run?workspace_id=ws-1",
        {
          dry_run: true,
          trigger_context: {
            manual: true,
            context: "analise o briefing",
            prompt: "analise o briefing",
            question: "analise o briefing",
            attachments: [],
          },
        },
      ),
    );
    // The coverage run executes this page alongside 70 other jsdom files;
    // 5s is too tight for the stepper plus the dry-run round-trip.
  }, 15_000);

  it("deletes only on the second click of the same button", async () => {
    render(
      <MemoryRouter>
        <RecipesPage />
      </MemoryRouter>,
    );
    await waitFor(() =>
      expect(screen.getByText("Receita E2E")).toBeInTheDocument(),
    );
    fireEvent.click(screen.getByText("Receita E2E"));
    const remove = await screen.findByRole("button", { name: "Excluir" });
    fireEvent.click(remove);
    expect(apiDelete).not.toHaveBeenCalled();
    const confirm = screen.getByRole("button", { name: "Tem certeza?" });
    fireEvent.click(confirm);
    await waitFor(() =>
      expect(apiDelete).toHaveBeenCalledWith(
        "/api/recipes/receita-e2e?workspace_id=ws-1",
      ),
    );
  });

  it("creates new recipes disabled so triggers cannot fire before review", async () => {
    render(
      <MemoryRouter>
        <RecipesPage />
      </MemoryRouter>,
    );
    await waitFor(() =>
      expect(screen.getByText("Receita E2E")).toBeInTheDocument(),
    );
    fireEvent.click(screen.getByRole("button", { name: /Nova receita/ }));
    await waitFor(() => expect(apiPut).toHaveBeenCalled());
    const call = vi.mocked(apiPut).mock.calls.at(-1);
    expect(call?.[0]).toMatch(/^\/api\/recipes\/receita-/);
    expect((call?.[1] as { yaml: string }).yaml).toContain("enabled: false");
  });
});

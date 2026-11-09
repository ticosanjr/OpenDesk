import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { AttentionCard } from "./AttentionCard";
import { useAppStore } from "@/store/app-store";

const hitl = {
  token: "T1",
  recipe_run_id: "run-1",
  workspace_id: "ws",
  checkpoint_id: "checkpoint-1",
  interaction_type: "BINARY_GATE",
  title: "Receita de atenção",
  question_markdown: "Aprovar o envio do relatório?",
  context_json: JSON.stringify({ next_step: "Publicar" }),
  options_json: "[]",
  on_timeout: "PAUSE_AND_ALERT",
  expires_at: "2999-01-01T00:00:00Z",
  status: "PENDING",
  created_at: "2026-01-01T00:00:00Z",
};

vi.mock("@/lib/api", () => ({
  apiGet: vi.fn(async () => ({
    requests: [],
    hitl: [hitl],
    proposals: [],
    awaiting_approval: [],
    blocked: [],
    failed_runs: [],
  })),
  apiPost: vi.fn(),
  apiDelete: vi.fn(),
}));

describe("AttentionCard HITL", () => {
  beforeEach(() => {
    useAppStore.setState({ activeWorkspaceId: "ws" });
  });

  it("shows the question and the consequence of approving", async () => {
    render(
      <MemoryRouter>
        <AttentionCard />
      </MemoryRouter>,
    );
    expect(
      await screen.findByText("Aprovar o envio do relatório?"),
    ).toBeInTheDocument();
    expect(screen.getByText(/Retoma o fluxo e segue para Publicar/)).toBeInTheDocument();
    expect(screen.getByText(/Encerra este run/)).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Aprovar e continuar" }),
    ).toBeInTheDocument();
  });
});

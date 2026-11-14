import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { InteractionCard } from "./InteractionCard";
import type { TaskInteraction } from "@/lib/types";

const post = vi.fn(async (..._args: unknown[]) => ({}));
vi.mock("@/lib/api", () => ({
  apiPost: (...args: unknown[]) => post(...args),
}));

function interaction(overrides: Partial<TaskInteraction>): TaskInteraction {
  return {
    id: "interaction-1",
    workspace_id: "ws",
    task_id: "task-1",
    run_id: null,
    question: "Posso publicar?",
    context_markdown: null,
    options_json: "[]",
    kind: "APPROVAL",
    multiple: false,
    allow_custom: true,
    status: "PENDING",
    answer_json: null,
    expires_at: "2999-01-01T00:00:00Z",
    created_at: "2026-01-01T00:00:00Z",
    resolved_at: null,
    ...overrides,
  };
}

describe("InteractionCard HITL 2.0", () => {
  it("sends an approval decision with an optional payload", async () => {
    render(<InteractionCard interaction={interaction({})} />);
    expect(screen.getByText("aprovação")).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText("Payload opcional"), {
      target: { value: "pode publicar, mas cite a fonte" },
    });
    fireEvent.click(screen.getByRole("button", { name: /Aprovar/ }));
    await waitFor(() =>
      expect(post).toHaveBeenCalledWith("/api/interactions/interaction-1/resolve", {
        decision: "approve",
        selected: [],
        text: "pode publicar, mas cite a fonte",
      }),
    );
  });

  it("requires text before answering an open question", async () => {
    render(
      <InteractionCard
        interaction={interaction({ kind: "TEXT", question: "Qual o prazo?" })}
      />,
    );
    const button = screen.getByRole("button", { name: /Enviar resposta/ });
    expect(button).toBeDisabled();
    fireEvent.change(screen.getByLabelText("Resposta livre"), {
      target: { value: "sexta-feira" },
    });
    expect(button).toBeEnabled();
    fireEvent.click(button);
    await waitFor(() =>
      expect(post).toHaveBeenCalledWith("/api/interactions/interaction-1/resolve", {
        decision: "answer",
        selected: [],
        text: "sexta-feira",
      }),
    );
  });
});

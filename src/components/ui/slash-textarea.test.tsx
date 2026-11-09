import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { SlashTextarea } from "./slash-textarea";

vi.mock("@/lib/studio-cache", () => ({
  loadRuntime: vi.fn(async () => ({
    commands: [{ name: "resumir", description: "Resume texto" }],
    skills: [{ id: "pesquisa", description: "Pesquisa fontes" }],
  })),
  loadAgents: vi.fn(async () => [
    { id: "build", name: "Builder", description: "Executa" },
  ]),
}));

describe("SlashTextarea", () => {
  it("suggests agents, commands and skills with distinct triggers", async () => {
    const onChange = vi.fn();
    render(
      <SlashTextarea value="" onChange={onChange} workspaceId="ws" rows={3} />,
    );
    const textarea = screen.getByLabelText("Comando ou prompt");

    for (const [typed, expected] of [
      ["@b", "@build"],
      ["/res", "/resumir"],
      ["#pes", "#pesquisa"],
    ] as const) {
      const value = typed;
      fireEvent.change(textarea, { target: { value } });
      expect(await screen.findByRole("option", { name: new RegExp(expected) })).toBeTruthy();
    }
  });
});

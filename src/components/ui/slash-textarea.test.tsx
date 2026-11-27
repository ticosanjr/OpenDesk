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

  it("opens the portal above the field when it is close to the viewport bottom", async () => {
    const onChange = vi.fn();
    const { container } = render(
      <SlashTextarea value="" onChange={onChange} workspaceId="ws" rows={3} />,
    );
    const textarea = screen.getByLabelText("Comando ou prompt");
    const wrapper = container.firstElementChild as HTMLElement;
    wrapper.getBoundingClientRect = () =>
      ({
        top: 600,
        bottom: 700,
        left: 40,
        width: 420,
        height: 100,
        right: 460,
        x: 40,
        y: 600,
        toJSON: () => ({}),
      }) as DOMRect;

    fireEvent.change(textarea, { target: { value: "@b" } });
    const listbox = await screen.findByRole("listbox");
    expect(listbox).toHaveAttribute("data-placement", "top");
    expect(listbox).toHaveClass("max-h-60");
    expect(listbox.parentElement).toBe(document.body);
  });
});

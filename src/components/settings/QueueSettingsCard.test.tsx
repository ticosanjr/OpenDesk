import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { QueueSettingsCard } from "./QueueSettingsCard";

vi.mock("@/lib/api", () => ({
  apiGet: vi.fn(async () => ({ enabled: true, cooldown_minutes: 5 })),
  apiPut: vi.fn(async (_path: string, value: unknown) => value),
}));

describe("QueueSettingsCard", () => {
  it("enables the cooldown field when dedupe is turned off", async () => {
    render(<QueueSettingsCard />);
    const toggle = await screen.findByRole("switch", {
      name: "Evitar tarefas duplicadas",
    });
    expect(toggle).toHaveAttribute("aria-checked", "true");
    const input = screen.getByLabelText("Carência após conclusão em minutos");
    expect(input).toBeEnabled();

    fireEvent.click(toggle);
    expect(toggle).toHaveAttribute("aria-checked", "false");
    expect(input).toBeDisabled();

    fireEvent.click(toggle);
    expect(toggle).toHaveAttribute("aria-checked", "true");
    expect(input).toBeEnabled();
  });
});

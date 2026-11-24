import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { QueueSettingsCard } from "./QueueSettingsCard";

vi.mock("@/lib/api", () => ({
  apiGet: vi.fn(async (path: string) => {
    if (path === "/api/settings/dedupe") {
      return { enabled: true, cooldown_minutes: 5 };
    }
    if (path === "/api/settings/tasks_autostart") return true;
    if (path === "/api/settings/missed_run_policy") return "ASK";
    if (path === "/api/queue/config") {
      return {
        max_concurrency: 3,
        max_queue_size: 200,
        default_agent_concurrency: 2,
        group_limits: { default: 3 },
      };
    }
    return null;
  }),
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
    expect(screen.getByLabelText("Concorrência máxima")).toHaveValue(3);
    expect(screen.getByLabelText("Tamanho máximo da fila")).toHaveValue(200);
    expect(screen.getByLabelText("Quota padrão por agente")).toHaveValue(2);
    expect(screen.getByLabelText("Limite do grupo default")).toHaveValue(3);

    fireEvent.click(toggle);
    expect(toggle).toHaveAttribute("aria-checked", "false");
    expect(input).toBeDisabled();

    fireEvent.click(toggle);
    expect(toggle).toHaveAttribute("aria-checked", "true");
    expect(input).toBeEnabled();
  });
});

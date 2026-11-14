import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { ExecutionModal } from "./execution-modal";
import type { Task } from "@/lib/types";
import { useAppStore } from "@/store/app-store";

const get = vi.fn();
const post = vi.fn();

vi.mock("@/lib/api", () => ({
  apiGet: (...args: unknown[]) => get(...args),
  apiPost: (...args: unknown[]) => post(...args),
}));

vi.mock("@/lib/studio-cache", () => ({
  loadAgents: async () => [],
  loadModels: async () => [],
  loadModelHealth: async () => [],
  loadRuntime: async () => ({ agents: [], commands: [], skills: [] }),
}));

function task(overrides: Partial<Task> = {}): Task {
  return {
    id: "task-1",
    workspace_id: "ws-1",
    title: "Preparar relatório",
    description: "Resuma o trimestre.",
    status: "TODO",
    phase: "build",
    phase_status: null,
    priority: 2,
    execution_mode: "AUTO_DSH",
    assigned_agent: null,
    preferred_model: null,
    agent_override: false,
    model_override: false,
    effort_level: "medium",
    care_level: "standard",
    plan_markdown: null,
    output_dir_path: "/tmp/out",
    notify_email: null,
    export_to_gdocs: false,
    tags_json: "[]",
    position_index: 0,
    last_run_id: null,
    last_run_status: null,
    last_run_at: null,
    blocked_reason: null,
    cancelled_at: null,
    forked_from_task_id: null,
    origin: "MANUAL",
    held: false,
    created_at: "2026-01-01T10:00:00Z",
    updated_at: "2026-01-01T10:00:00Z",
    ...overrides,
  } as Task;
}

describe("ExecutionModal", () => {
  beforeEach(() => {
    get.mockReset();
    post.mockReset();
    get.mockResolvedValue(task());
    post.mockResolvedValue({
      decision: {
        phase: "build",
        selected_agent: "build",
        selected_model: "openai/gpt",
      },
    });
    useAppStore.setState({
      activeWorkspaceId: "ws-1",
      daemon: { online: true } as never,
    });
  });

  it("starts a run with per-execution agent, model and instruction", async () => {
    render(
      <MemoryRouter>
        <ExecutionModal open task={task()} onClose={() => {}} />
      </MemoryRouter>,
    );
    await screen.findByText("Preparar relatório");
    fireEvent.change(
      screen.getByPlaceholderText(/Digite \//),
      { target: { value: "/resumir Foque no mercado interno." } },
    );
    fireEvent.click(screen.getByRole("button", { name: /Executar agora/ }));
    await waitFor(() => expect(post).toHaveBeenCalled());
    expect(post).toHaveBeenCalledWith("/api/tasks/task-1/dispatch", {
      manual_agent: null,
      manual_model: null,
      manual_effort: "medium",
      execution_mode: "AUTO_DSH",
      prompt_append: "/resumir Foque no mercado interno.",
    });
  });

  it("stops a running execution from the same screen", async () => {
    get.mockResolvedValue(
      task({ status: "IN_PROGRESS", phase_status: "running", last_run_id: "run-1" }),
    );
    render(
      <MemoryRouter>
        <ExecutionModal open task={task({ status: "IN_PROGRESS" })} onClose={() => {}} />
      </MemoryRouter>,
    );
    const stop = await screen.findByRole("button", { name: /Parar execução/ });
    fireEvent.click(stop);
    await waitFor(() =>
      expect(post).toHaveBeenCalledWith("/api/tasks/task-1/transition", {
        status: "CANCELLED",
      }),
    );
  });
});

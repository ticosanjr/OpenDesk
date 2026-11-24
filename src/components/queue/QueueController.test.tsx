import { act, fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, describe, expect, it, vi } from "vitest";
import { QueueController } from "./QueueController";
import type { QueueJob, QueueStatus } from "@/lib/types";

const status: QueueStatus = {
  paused: false,
  active_jobs: 1,
  concurrency: 2,
  max_concurrency: 2,
  max_queue_size: 200,
  default_agent_concurrency: 2,
  group_usage: [],
  running: 1,
  waiting: 1,
};

const jobs: QueueJob[] = [
  {
    id: "job-1",
    task_id: "task-1",
    title: "Rodando",
    status: "RUNNING",
    priority: 2,
    resolved_mode: "build",
    resolved_agent: "build",
    resolved_model: "model",
    resolved_effort: "low",
    concurrency_group: "local_llm_heavy",
    enqueued_at: new Date().toISOString(),
    lease_expires_at: new Date(Date.now() + 60_000).toISOString(),
  },
  {
    id: "job-2",
    task_id: "task-2",
    title: "Aguardando",
    status: "QUEUED",
    priority: 2,
    resolved_mode: "build",
    resolved_agent: "build",
    resolved_model: "model",
    resolved_effort: "low",
    enqueued_at: new Date().toISOString(),
    wait_reason: "Grupo default cheio (1/1)",
  },
];

describe("QueueController", () => {
  it("shows counts, lease, and dispatches cancel/retry/priority actions", () => {
    const onJobAction = vi.fn();
    const onPriority = vi.fn();
    render(
      <MemoryRouter>
        <QueueController
          queue={jobs}
          status={status}
          loading={false}
          onRefresh={vi.fn()}
          onTogglePause={vi.fn()}
          onConcurrency={vi.fn()}
          onJobAction={onJobAction}
          onPriority={onPriority}
        />
      </MemoryRouter>,
    );
    expect(screen.getByText("Rodando")).toBeInTheDocument();
    expect(screen.getByText("Aguardando")).toBeInTheDocument();
    expect(screen.getAllByText(/lease/).length).toBeGreaterThan(0);
    expect(screen.getByText(/local_llm_heavy/)).toBeInTheDocument();
    expect(screen.getByText("Grupo default cheio (1/1)")).toBeInTheDocument();
    expect(screen.getByText(/slots 1\/2/)).toBeInTheDocument();
    expect(screen.getByText(/fila 1\/200/)).toBeInTheDocument();

    fireEvent.click(screen.getAllByRole("button", { name: "Cancelar" })[0]);
    expect(onJobAction).toHaveBeenCalledWith("job-1", "cancel");

    fireEvent.click(screen.getByTitle("Aumentar prioridade"));
    expect(onPriority).toHaveBeenCalledWith("job-2", 3);
    expect(
      document.querySelector('[data-focus-id="job-2"]'),
    ).toBeInTheDocument();
  });

  it("scrolls to and highlights the job from ?job=<id>", () => {
    vi.useFakeTimers();
    const scrollIntoView = vi.fn();
    Element.prototype.scrollIntoView = scrollIntoView;
    render(
      <MemoryRouter initialEntries={["/kanban?view=queue&job=job-2"]}>
        <QueueController
          queue={jobs}
          status={status}
          loading={false}
          onRefresh={vi.fn()}
          onTogglePause={vi.fn()}
          onConcurrency={vi.fn()}
          onJobAction={vi.fn()}
          onPriority={vi.fn()}
        />
      </MemoryRouter>,
    );
    act(() => {
      vi.advanceTimersByTime(200);
    });
    expect(scrollIntoView).toHaveBeenCalled();
    expect(
      document
        .querySelector('[data-focus-id="job-2"]')
        ?.classList.contains("focus-pulse"),
    ).toBe(true);
  });
});

afterEach(() => {
  vi.useRealTimers();
});

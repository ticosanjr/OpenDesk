import { fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it, vi } from "vitest";
import { ActivityFeed, type Activity } from "./ActivityFeed";
import type { Artifact, TaskRun } from "@/lib/types";

const now = new Date().toISOString();

const run = {
  id: "run-1",
  task_title: "Diagnóstico Aurora",
  agent_name: "build",
  model_id: "glm",
  status: "SUCCESS",
  phase: "build",
  execution_mode: "AUTO_DSH",
  started_at: now,
  completed_at: now,
  duration_ms: 1000,
  tokens_input: 10,
  tokens_output: 5,
  estimated_cost_usd: 0.01,
  error_message: null,
} as unknown as TaskRun;

const artifact = {
  id: "artifact-1",
  file_path: "output/relatorio.md",
  created_by_agent: "build",
  creation_reason: "entrega",
  created_at: now,
  pinned: false,
} as unknown as Artifact;

const activities: Activity[] = [
  { kind: "run", at: now, run },
  { kind: "artifact", at: now, artifact },
];

describe("ActivityFeed", () => {
  it("groups by day and filters runs from artifacts", () => {
    render(
      <MemoryRouter>
        <ActivityFeed activities={activities} onCreateTask={vi.fn()} />
      </MemoryRouter>,
    );

    expect(screen.getByText("Hoje")).toBeInTheDocument();
    expect(screen.getByText(/Diagnóstico Aurora/)).toBeInTheDocument();
    expect(screen.getByText("output/relatorio.md")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Artefatos" }));
    expect(screen.queryByText(/Diagnóstico Aurora/)).not.toBeInTheDocument();
    expect(screen.getByText("output/relatorio.md")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Runs" }));
    expect(screen.getByText(/Diagnóstico Aurora/)).toBeInTheDocument();
    expect(screen.queryByText("output/relatorio.md")).not.toBeInTheDocument();
  });
});

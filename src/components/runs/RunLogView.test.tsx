import { fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";

const apiGet = vi.hoisted(() => vi.fn());
vi.mock("@/lib/api", () => ({
  apiGet,
  apiText: vi.fn(),
}));

import { RunLogView } from "./RunLogView";
import type { RunTimeline } from "@/lib/types";

const timeline: RunTimeline = {
  status: "SUCCESS",
  started_at: "2026-11-06T21:32:04Z",
  completed_at: "2026-11-06T21:33:16Z",
  duration_ms: 72_000,
  events_count: 20,
  log_missing: false,
  workspace_root: "/tmp/workspace",
  output_directory: "/tmp/workspace/output/2026-11/test",
  output_directory_relative: "output/2026-11/test",
  artifacts: [],
  events: [
    {
      kind: "thinking",
      role: "assistant",
      text: "Vou analisar o workspace.",
      started_at: "2026-11-06T21:32:04Z",
      ended_at: "2026-11-06T21:32:08Z",
      duration_ms: 4000,
    },
    {
      kind: "tool",
      role: "tool",
      text: "read",
      tool: {
        id: "call-1",
        name: "read",
        summary: "read · input/brief.md",
        input: '{"path":"/tmp/workspace/input/brief.md"}',
        output: "conteúdo",
        status: "success",
        started_at: "2026-11-06T21:32:08Z",
        ended_at: "2026-11-06T21:32:09Z",
        duration_ms: 1000,
        output_truncated: false,
      },
      started_at: "2026-11-06T21:32:08Z",
      ended_at: "2026-11-06T21:32:09Z",
      duration_ms: 1000,
    },
  ],
  run: {
    id: "run-1",
    queue_job_id: null,
    workspace_id: "ws-1",
    task_id: "task-1",
    schedule_id: null,
    attempt_number: 1,
    session_id: null,
    parent_run_id: null,
    phase: "build",
    agent_name: "build",
    execution_mode: "AUTO_DSH",
    provider_id: "opencode-go",
    model_id: "deepseek",
    effort_level: "high",
    status: "SUCCESS",
    tokens_input: 2000,
    tokens_output: 300,
    tokens_reasoning: 0,
    tokens_cache_read: 0,
    tokens_cache_write: 0,
    estimated_cost_usd: 0.002,
    pricing_snapshot_json: null,
    duration_ms: 72_000,
    log_path: "/tmp/run.jsonl",
    raw_stdout_log: null,
    summary_response: "Resumo da execução",
    started_at: "2026-11-06T21:32:04Z",
    completed_at: "2026-11-06T21:33:16Z",
    error_code: null,
    error_message: null,
    error_hint: null,
    task_title: "Tarefa teste",
  },
  steps: [
    {
      index: 1,
      agent: "build",
      model: "opencode-go/deepseek#high",
      started_at: "2026-11-06T21:32:04Z",
      ended_at: "2026-11-06T21:32:10Z",
      duration_ms: 6000,
      tokens_input: 2000,
      tokens_output: 300,
      tokens_reasoning: 0,
      cost_usd: 0.002,
      finish: "stop",
    },
  ],
  thinking: {
    started_at: "2026-11-06T21:32:04Z",
    ended_at: "2026-11-06T21:32:08Z",
    duration_ms: 4000,
    text: "Vou analisar o workspace.",
    tools: [],
    truncated: false,
  },
  executed: {
    started_at: "2026-11-06T21:32:08Z",
    ended_at: "2026-11-06T21:32:12Z",
    duration_ms: 4000,
    text: "",
    tools: [
      {
        id: "call-1",
        name: "read",
        summary: "read · input/brief.md",
        input: '{"path":"/tmp/workspace/input/brief.md"}',
        output: "conteúdo",
        status: "success",
        started_at: "2026-11-06T21:32:08Z",
        ended_at: "2026-11-06T21:32:09Z",
        duration_ms: 1000,
        output_truncated: false,
      },
    ],
    truncated: false,
  },
  result: {
    started_at: "2026-11-06T21:33:05Z",
    ended_at: "2026-11-06T21:33:15Z",
    duration_ms: 10_000,
    text: "Resultado final da execução.",
    tools: [],
    truncated: false,
  },
};

describe("RunLogView", () => {
  beforeEach(() => {
    apiGet.mockReset();
    apiGet.mockImplementation((path: string) =>
      path.includes("/messages") ? Promise.resolve([]) : Promise.resolve(timeline),
    );
  });

  it("renders collapsed thinking/executed bands and the expanded result", async () => {
    render(
      <MemoryRouter>
        <RunLogView runId="run-1" />
      </MemoryRouter>,
    );

    expect(await screen.findByText("Raciocínio")).toBeInTheDocument();
    expect(screen.getByText("Resultado")).toBeInTheDocument();
    expect(screen.getByText("Resultado final da execução.")).toBeInTheDocument();
    const thinking = screen.getByRole("button", { name: /Raciocínio/ });
    expect(thinking).toHaveAttribute("aria-expanded", "false");
    fireEvent.click(thinking);
    expect(thinking).toHaveAttribute("aria-expanded", "true");
    expect(
      screen.getByText("Vou analisar o workspace.", { selector: "pre" }),
    ).toBeInTheDocument();

    expect(screen.getByText("read · input/brief.md")).toBeInTheDocument();
  });

  it("expands and collapses every section from the header", async () => {
    render(
      <MemoryRouter>
        <RunLogView runId="run-1" />
      </MemoryRouter>,
    );

    const thinking = await screen.findByRole("button", { name: /Raciocínio/ });
    expect(thinking).toHaveAttribute("aria-expanded", "false");

    fireEvent.click(screen.getByRole("button", { name: "expandir" }));
    expect(thinking).toHaveAttribute("aria-expanded", "true");

    fireEvent.click(screen.getByRole("button", { name: "recolher" }));
    expect(thinking).toHaveAttribute("aria-expanded", "false");
  });

  it("keeps actionable permission requests under any filter", async () => {
    apiGet.mockImplementation((path: string) => {
      if (path.includes("/messages")) return Promise.resolve([]);
      if (path.includes("/interactions")) return Promise.resolve([]);
      if (path.includes("/api/permissions")) {
        return Promise.resolve({
          requests: [
            {
              request_id: "perm-1",
              permission_action: "bash",
              resources: "rm -rf build",
              task_id: "task-1",
              run_id: "run-1",
            },
          ],
        });
      }
      return Promise.resolve(timeline);
    });

    render(
      <MemoryRouter>
        <RunLogView runId="run-1" />
      </MemoryRouter>,
    );

    expect(
      await screen.findByText("O agente pediu permissão"),
    ).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText("Filtrar eventos da execução"), {
      target: { value: "errors" },
    });
    expect(screen.getByText("O agente pediu permissão")).toBeInTheDocument();
  });

  it("filters the stream by tools and errors", async () => {
    render(
      <MemoryRouter>
        <RunLogView runId="run-1" />
      </MemoryRouter>,
    );

    await screen.findByText("Raciocínio");
    const filter = screen.getByLabelText("Filtrar eventos da execução");
    fireEvent.change(filter, { target: { value: "tools" } });
    expect(screen.queryByText("Raciocínio")).not.toBeInTheDocument();
    expect(screen.getByText("read · input/brief.md")).toBeInTheDocument();

    fireEvent.change(filter, { target: { value: "errors" } });
    expect(screen.queryByText("read · input/brief.md")).not.toBeInTheDocument();
    expect(
      screen.getByText("Nenhum evento corresponde a este filtro."),
    ).toBeInTheDocument();
  });
});

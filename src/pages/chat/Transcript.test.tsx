import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it, vi } from "vitest";
import type { ChatMessage, ChatThread } from "@/lib/types";
import { Transcript } from "./Transcript";

const thread: ChatThread = {
  id: "thread-1",
  workspace_id: "ws-1",
  title: "Conversa",
  agent: "build",
  source: "desktop",
  created_at: "2026-11-25T00:00:00Z",
  updated_at: "2026-11-25T00:00:00Z",
};

function renderTranscript(message: ChatMessage) {
  render(
    <MemoryRouter>
      <Transcript
        thread={thread}
        messages={[message]}
        thinking={false}
        turnRunning={false}
        latestTurn={null}
        stopping={false}
        transcriptRef={{ current: null }}
        onTranscriptScroll={vi.fn()}
        transcriptAtBottom
        onScrollToBottom={vi.fn()}
        onSpeak={vi.fn()}
        onStop={vi.fn()}
      />
    </MemoryRouter>,
  );
}

describe("Transcript attachments", () => {
  it("renders clickable sent and received chips from metadata", () => {
    renderTranscript({
      id: "m1",
      thread_id: "thread-1",
      role: "assistant",
      content: "Segue o relatório.",
      created_at: "2026-11-25T00:00:01Z",
      metadata_json: JSON.stringify({
        attachments: [
          {
            name: "relatorio.md",
            path: "output/relatorio.md",
            size: 2048,
            direction: "received",
            source: "agent",
          },
          {
            name: "brief.txt",
            path: "input/brief.txt",
            direction: "sent",
          },
        ],
      }),
    });
    expect(screen.getByTitle("Abrir output/relatorio.md")).toBeInTheDocument();
    expect(screen.getByText("relatorio.md")).toBeInTheDocument();
    expect(screen.getByText("2.0 KB")).toBeInTheDocument();
    expect(screen.getByTitle("Abrir input/brief.txt")).toBeInTheDocument();
  });

  it("ignores malformed metadata instead of crashing", () => {
    renderTranscript({
      id: "m2",
      thread_id: "thread-1",
      role: "user",
      content: "sem metadados válidos",
      created_at: "2026-11-25T00:00:02Z",
      metadata_json: "{not-json",
    });
    expect(screen.getByText("sem metadados válidos")).toBeInTheDocument();
  });
});

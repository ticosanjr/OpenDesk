import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { ChannelBotsPanel } from "./ChannelBotsPanel";
import { useAppStore } from "@/store/app-store";
import { apiGet, apiPost, apiPut } from "@/lib/api";
import type { ChannelBot, ChannelChatContext, Workspace } from "@/lib/types";

const BOT: ChannelBot = {
  id: "bot-1",
  workspace_id: "ws-1",
  channel: "telegram",
  name: "Bot Trabalho",
  enabled: true,
  token_configured: true,
  allowlist: ["-1001"],
  default_agent: "build",
  status: "running",
  last_error: null,
  last_checked_at: null,
  running: true,
  config_json: "{}",
  created_at: "2026-01-01",
  updated_at: "2026-01-01",
};

const OTHER: ChannelBot = {
  ...BOT,
  id: "bot-2",
  workspace_id: "ws-2",
  name: "Bot Hobbies",
  status: "error",
  running: false,
};

const CONTEXT: ChannelChatContext = {
  id: "ctx-1",
  bot_id: "bot-1",
  workspace_id: "ws-1",
  channel: "telegram",
  external_chat_id: "-1001",
  chat_type: "group",
  title: "Trabalho",
  system_context: "Contexto do trabalho",
  agent_override: null,
  memory_scope: "chat",
  opencode_session_id: "ses-1",
  enabled: true,
  created_at: "2026-01-01",
  updated_at: "2026-01-01",
};

const workspace = (id: string, name: string): Workspace => ({
  id,
  name,
  root_path: `/tmp/${id}`,
  color_hex: "#3b82f6",
  default_router_policy: "balanced",
  timezone: "UTC",
  is_active: id === "ws-1",
  created_at: "2026-01-01",
  last_opened_at: "2026-01-01",
});

vi.mock("@/lib/api", () => ({
  apiGet: vi.fn(),
  apiPost: vi.fn(),
  apiPut: vi.fn(),
  apiDelete: vi.fn(async () => ({ deleted: true })),
}));

vi.mock("@/store/toast-store", () => ({ toast: vi.fn() }));

vi.mock("@/store/confirm-store", () => ({
  confirmAction: vi.fn(async () => true),
}));

describe("ChannelBotsPanel", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useAppStore.setState({
      workspaces: [workspace("ws-1", "Workspace Um"), workspace("ws-2", "Outra")],
      activeWorkspaceId: "ws-1",
    });
    vi.mocked(apiGet).mockImplementation(async (path: string) => {
      if (path.startsWith("/api/channels/bots?all=true")) {
        return [BOT, OTHER] as never;
      }
      if (path.endsWith("/contexts")) {
        return [CONTEXT] as never;
      }
      return null as never;
    });
    vi.mocked(apiPost).mockResolvedValue(BOT);
    vi.mocked(apiPut).mockResolvedValue(BOT);
  });

  it("shows only the active workspace bots and summarizes the others", async () => {
    render(<ChannelBotsPanel channel="telegram" workspaceId="ws-1" />);

    expect(await screen.findByText("Bot Trabalho")).toBeInTheDocument();
    expect(screen.getByText("Bots de Telegram")).toBeInTheDocument();
    expect(screen.getByText("Workspace Um")).toBeInTheDocument();
    expect(screen.queryByText("Bot Hobbies")).not.toBeInTheDocument();
    expect(screen.getByText("Outra: 1")).toBeInTheDocument();
    expect(await screen.findByText("Contextos (1)")).toBeInTheDocument();
  });

  it("reloads when the workspace changes without leaking old bots", async () => {
    const view = render(<ChannelBotsPanel channel="telegram" workspaceId="ws-1" />);
    expect(await screen.findByText("Bot Trabalho")).toBeInTheDocument();

    view.rerender(<ChannelBotsPanel channel="telegram" workspaceId="ws-2" />);
    expect(await screen.findByText("Bot Hobbies")).toBeInTheDocument();
    expect(screen.queryByText("Bot Trabalho")).not.toBeInTheDocument();
    expect(screen.getByText("Workspace Um: 1")).toBeInTheDocument();
  });

  it("creates a bot with the active workspace and channel", async () => {
    render(<ChannelBotsPanel channel="telegram" workspaceId="ws-1" />);
    await screen.findByText("Bot Trabalho");

    fireEvent.click(screen.getByRole("button", { name: "Novo bot" }));
    fireEvent.change(screen.getByLabelText("Nome do bot"), {
      target: { value: "Bot Novo" },
    });
    fireEvent.change(
      screen.getByLabelText("Allowlist (IDs ou @username, separados por vírgula)"),
      { target: { value: "-1002, @teste" } },
    );
    fireEvent.click(screen.getByRole("button", { name: "Salvar bot" }));

    await waitFor(() => {
      expect(apiPost).toHaveBeenCalledWith(
        "/api/channels/bots",
        expect.objectContaining({
          workspace_id: "ws-1",
          channel: "telegram",
          name: "Bot Novo",
          allowlist: ["-1002", "@teste"],
        }),
      );
    });
  });

  it("opens the context editor and resets only the selected group session", async () => {
    render(<ChannelBotsPanel channel="telegram" workspaceId="ws-1" />);
    fireEvent.click(await screen.findByRole("button", { name: "Contextos (1)" }));

    expect(await screen.findByText("Contextos de Bot Trabalho")).toBeInTheDocument();
    expect(screen.getByDisplayValue("Trabalho")).toBeInTheDocument();
    fireEvent.click(
      screen.getByRole("button", { name: "Reiniciar sessão do grupo" }),
    );

    await waitFor(() => {
      expect(apiPut).toHaveBeenCalledWith(
        "/api/channels/bots/bot-1/contexts/ctx-1",
        { opencode_session_id: null },
      );
    });
  });
});

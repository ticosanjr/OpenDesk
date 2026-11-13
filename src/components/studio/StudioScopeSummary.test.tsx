import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

const apiGet = vi.hoisted(() => vi.fn());
vi.mock("@/lib/api", () => ({ apiGet }));

import { StudioScopeSummary } from "./StudioScopeSummary";

const item = (kind: string, source_scope: string, disabled = false) => ({
  kind,
  id: `${kind}-${source_scope}-${disabled}`,
  source_scope,
  disabled,
  overridden: false,
  value: {},
});

describe("StudioScopeSummary", () => {
  it("shows global and workspace counts separately", async () => {
    apiGet.mockResolvedValue({
      items: [
        item("agent", "global"),
        item("skill", "workspace"),
        item("skill", "workspace", true),
        item("command", "global"),
      ],
    });
    const { rerender } = render(
      <StudioScopeSummary scope="global" workspaceId="ws-1" />,
    );
    expect(await screen.findByText("Agentes")).toBeInTheDocument();
    expect(screen.getByText(/2 item\(ns\)/)).toBeInTheDocument();

    rerender(<StudioScopeSummary scope="workspace" workspaceId="ws-1" />);
    expect(await screen.findByText(/1 desabilitado\(s\)/)).toBeInTheDocument();
  });
});

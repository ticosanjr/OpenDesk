import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@/lib/api", () => ({
  apiDelete: vi.fn(),
  apiGet: vi.fn(),
  apiPatch: vi.fn(),
  apiPost: vi.fn(),
  apiPut: vi.fn(),
}));
vi.mock("@/store/toast-store", () => ({ toast: vi.fn() }));

import { apiGet } from "@/lib/api";
import { VaultCard } from "./VaultCard";

describe("VaultCard", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    (apiGet as unknown as ReturnType<typeof vi.fn>).mockResolvedValue({
      entries: [
        {
          id: "stripe",
          kind: "api_key",
          label: "Stripe",
          username: "ops",
          allowed_domains_json: '["api.example.com"]',
          requires_hitl: true,
          created_at: "2026-11-23T00:00:00Z",
          updated_at: "2026-11-23T00:00:00Z",
        },
      ],
    });
  });

  it("lists metadata without exposing a secret", async () => {
    render(<VaultCard workspaceId="ws-1" />);
    expect(await screen.findByText("Stripe")).toBeTruthy();
    expect(screen.getByText("HITL")).toBeTruthy();
    expect(screen.getByText(/api\.example\.com/)).toBeTruthy();
    expect(screen.queryByText(/super-secret/)).toBeNull();
  });
});

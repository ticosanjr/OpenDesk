import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";

const apiGet = vi.hoisted(() => vi.fn());
vi.mock("@/lib/api", () => ({ apiGet }));

import { SearchPage } from "./SearchPage";
import { useAppStore } from "@/store/app-store";

describe("SearchPage input", () => {
  beforeEach(() => {
    apiGet.mockReset();
    apiGet.mockResolvedValue([]);
    useAppStore.setState({ activeWorkspaceId: "ws-1" });
    window.location.hash = "#/search";
  });

  it("keeps every character typed while the debounced URL updates", async () => {
    render(
      <MemoryRouter initialEntries={["/search"]}>
        <SearchPage />
      </MemoryRouter>,
    );
    const input = screen.getByPlaceholderText(/Digite pelo menos 2 caracteres/);

    fireEvent.change(input, { target: { value: "dia" } });
    await waitFor(() => expect(input).toHaveValue("dia"));
    fireEvent.change(input, { target: { value: "diagn" } });
    fireEvent.change(input, { target: { value: "diagnó" } });
    fireEvent.change(input, { target: { value: "diagnóstico" } });

    // The debounce writes an older value to the URL; it must not overwrite the
    // text that is already in the field.
    await waitFor(() =>
      expect(apiGet).toHaveBeenCalledWith(
        expect.stringContaining("q=diagn%C3%B3stico"),
      ),
    );
    expect(input).toHaveValue("diagnóstico");
  });
});

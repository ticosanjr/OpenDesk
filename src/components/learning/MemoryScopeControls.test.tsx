import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import {
  MemoryScopeFilterBar,
  MemoryScopeSelect,
  matchesScopeFilter,
} from "./MemoryScopeControls";

describe("MemoryScopeControls", () => {
  it("marks the active filter and reports changes", () => {
    const onChange = vi.fn();
    render(<MemoryScopeFilterBar value="all" onChange={onChange} />);
    expect(screen.getByRole("button", { name: "Todos" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    fireEvent.click(screen.getByRole("button", { name: "Chat" }));
    expect(onChange).toHaveBeenCalledWith("chat");
  });

  it("changes the inline scope and filters legacy records as both", () => {
    const onChange = vi.fn();
    render(
      <MemoryScopeSelect value="normal" label="Nota" onChange={onChange} />,
    );
    fireEvent.change(screen.getByRole("combobox"), { target: { value: "chat" } });
    expect(onChange).toHaveBeenCalledWith("chat");
    expect(matchesScopeFilter(undefined, "both")).toBe(true);
    expect(matchesScopeFilter("chat", "normal")).toBe(false);
  });
});

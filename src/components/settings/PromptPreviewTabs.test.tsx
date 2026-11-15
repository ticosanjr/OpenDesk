import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { PromptPreviewTabs } from "./PromptPreviewTabs";

describe("PromptPreviewTabs", () => {
  it("keeps the required order and notifies changes", () => {
    const onChange = vi.fn();
    render(<PromptPreviewTabs view="sections" onChange={onChange} />);
    const labels = screen
      .getAllByRole("button")
      .map((button) => button.textContent);
    expect(labels).toEqual([
      "Seções",
      "OpenDesk",
      "OpenCode",
      "Final reconstruído",
    ]);
    fireEvent.click(screen.getByRole("button", { name: "OpenCode" }));
    expect(onChange).toHaveBeenCalledWith("opencode");
  });
});

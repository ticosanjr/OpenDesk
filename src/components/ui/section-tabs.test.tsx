import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { SectionTabs } from "./section-tabs";

describe("SectionTabs", () => {
  it("renders only the selected state and emits changes", () => {
    const onChange = vi.fn();
    render(
      <SectionTabs
        items={[
          { id: "general", label: "Geral" },
          { id: "engine", label: "Engine", badge: 2 },
        ]}
        value="general"
        onChange={onChange}
      />,
    );
    expect(screen.getByRole("tab", { name: "Geral" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    expect(screen.getByRole("tab", { name: /Engine/ })).toHaveAttribute(
      "aria-selected",
      "false",
    );
    fireEvent.click(screen.getByRole("tab", { name: /Engine/ }));
    expect(onChange).toHaveBeenCalledWith("engine");
  });
});

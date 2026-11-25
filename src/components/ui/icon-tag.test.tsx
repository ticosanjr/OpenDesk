import { render, screen } from "@testing-library/react";
import { Scale } from "lucide-react";
import { fireEvent } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { IconTag, originTag, routerPolicyTag, triggerTag } from "./icon-tag";

describe("IconTag", () => {
  it("exposes an accessible label and tooltip", () => {
    render(<IconTag icon={Scale} label="melhor custo-benefício" />);
    const icon = screen.getByRole("img", { name: "melhor custo-benefício" });
    expect(icon).toHaveAttribute("tabindex", "0");
    fireEvent.focus(icon);
    expect(screen.getByRole("tooltip")).toHaveTextContent(
      "melhor custo-benefício",
    );
  });

  it("maps router, origin and trigger metadata", () => {
    expect(routerPolicyTag("COST_BENEFIT").label).toBe(
      "melhor custo-benefício",
    );
    expect(routerPolicyTag("CUSTOM").label).toBe("CUSTOM");
    expect(originTag("RECIPE").label).toContain("receita");
    expect(triggerTag("CRON").label).toContain("cron");
  });
});

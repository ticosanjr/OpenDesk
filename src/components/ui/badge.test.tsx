import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { Badge } from "./badge";

describe("Badge", () => {
  it("renders children and variant classes", () => {
    render(<Badge variant="green">online</Badge>);
    const badge = screen.getByText("online");
    expect(badge).toBeInTheDocument();
    expect(badge.className).toContain("emerald");
  });
});

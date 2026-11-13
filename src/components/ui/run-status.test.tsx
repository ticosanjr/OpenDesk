import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { RunStatusBadge, RunStatusIcon, runStatusMeta } from "./run-status";

describe("runStatusMeta", () => {
  it("keeps icon, tone and label in sync for every terminal state", () => {
    expect(runStatusMeta("RUNNING")).toMatchObject({ tone: "blue", spin: true });
    expect(runStatusMeta("SUCCESS")).toMatchObject({ tone: "green", label: "sucesso" });
    expect(runStatusMeta("FAILED")).toMatchObject({ tone: "red", label: "falhou" });
    expect(runStatusMeta("CANCELLED")).toMatchObject({
      tone: "default",
      label: "cancelada",
      spin: false,
    });
    expect(runStatusMeta("BLOCKED")).toMatchObject({ tone: "yellow", label: "bloqueada" });
    expect(runStatusMeta("QUEUED")).toMatchObject({ tone: "yellow", label: "na fila" });
  });

  it("renders a spinning icon only while running", () => {
    const { rerender } = render(<RunStatusIcon status="RUNNING" />);
    expect(document.querySelector("svg")).toHaveClass("animate-spin");
    rerender(<RunStatusIcon status="CANCELLED" />);
    expect(document.querySelector("svg")).not.toHaveClass("animate-spin");
  });

  it("renders the same tone in badges", () => {
    render(<RunStatusBadge status="CANCELLED" />);
    expect(screen.getByText("cancelada")).toHaveClass("text-zinc-300");
  });
});

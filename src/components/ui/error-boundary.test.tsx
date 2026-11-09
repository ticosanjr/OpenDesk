import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { ErrorBoundary } from "./error-boundary";

function Boom(): never {
  throw new Error("boom");
}

describe("ErrorBoundary", () => {
  it("resets when the route key changes", () => {
    const consoleError = vi.spyOn(console, "error").mockImplementation(() => {});
    const { rerender } = render(
      <ErrorBoundary resetKey="/broken">
        <Boom />
      </ErrorBoundary>,
    );
    expect(screen.getByText("Algo quebrou nesta tela")).toBeInTheDocument();
    expect(screen.getByText("boom")).toBeInTheDocument();

    rerender(
      <ErrorBoundary resetKey="/healthy">
        <p>conteúdo saudável</p>
      </ErrorBoundary>,
    );
    expect(screen.getByText("conteúdo saudável")).toBeInTheDocument();
    consoleError.mockRestore();
  });
});

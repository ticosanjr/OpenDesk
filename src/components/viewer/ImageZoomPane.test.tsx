import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ImageZoomPane } from "./ImageZoomPane";

describe("ImageZoomPane", () => {
  it("zooms with the controls and resets to fit", () => {
    render(<ImageZoomPane src="blob:preview" alt="foto.png" />);

    expect(screen.getByText("100%", { selector: "span" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Aumentar zoom" }));
    expect(screen.getByText("125%", { selector: "span" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Diminuir zoom" }));
    expect(screen.getByText("100%", { selector: "span" })).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "200%" }));
    expect(screen.getByText("200%", { selector: "span" })).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /Ajustar/ }));
    expect(screen.getByText("100%", { selector: "span" })).toBeInTheDocument();
  });

  it("zooms with the wheel and toggles fit on double click", () => {
    render(<ImageZoomPane src="blob:preview" alt="foto.png" />);
    const pane = screen.getByRole("group", { name: "Visualizador de foto.png" });

    fireEvent.doubleClick(pane);
    expect(screen.getByText("200%", { selector: "span" })).toBeInTheDocument();
    fireEvent.doubleClick(pane);
    expect(screen.getByText("100%", { selector: "span" })).toBeInTheDocument();

    fireEvent.wheel(pane, { deltaY: -100 });
    expect(screen.getByText("125%", { selector: "span" })).toBeInTheDocument();
  });

  it("resets the zoom when the source changes", () => {
    const { rerender } = render(
      <ImageZoomPane src="blob:one" alt="foto.png" />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Aumentar zoom" }));
    expect(screen.getByText("125%", { selector: "span" })).toBeInTheDocument();
    rerender(<ImageZoomPane src="blob:two" alt="foto.png" />);
    expect(screen.getByText("100%", { selector: "span" })).toBeInTheDocument();
  });
});

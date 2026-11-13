import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

vi.mock("@/lib/file-preferences", () => ({
  loadFilePreferences: vi.fn(async () => ({
    downloadDir: "",
    downloadAsk: true,
    htmlAllowScripts: false,
  })),
}));

import { FileViewer } from "./FileViewer";

describe("FileViewer HTML", () => {
  it("blocks scripts by default and allows them for one view", async () => {
    render(
      <FileViewer
        name="relatorio.html"
        path="output/relatorio.html"
        content="<html><body>ok</body></html>"
        mime="text/html"
      />,
    );

    const frame = screen.getByTitle("relatorio.html");
    expect(frame).toHaveAttribute("sandbox", "");
    fireEvent.click(
      await screen.findByRole("button", { name: /scripts bloqueados/ }),
    );
    expect(frame).toHaveAttribute("sandbox", "allow-scripts");
    expect(screen.getByText("scripts liberados")).toBeInTheDocument();
  });
});

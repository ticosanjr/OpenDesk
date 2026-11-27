import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

vi.mock("@/lib/file-preferences", () => ({
  loadFilePreferences: vi.fn(async () => ({
    downloadDir: "",
    downloadAsk: true,
    importExportAsk: true,
    htmlAllowScripts: false,
  })),
  saveFilePreferences: vi.fn(async () => undefined),
}));

import { FileViewer } from "./FileViewer";

describe("FileViewer HTML", () => {
  it("blocks scripts by default and reloads when allowed", async () => {
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
    // The iframe is remounted on purpose so the browser re-runs the document.
    const reloaded = screen.getByTitle("relatorio.html");
    expect(reloaded).not.toBe(frame);
    expect(reloaded).toHaveAttribute("sandbox", "allow-scripts");
    expect(screen.getByText("scripts liberados")).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /recarregar/i }),
    ).toBeInTheDocument();
  });
});

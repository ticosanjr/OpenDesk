import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { useState } from "react";
import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { Modal } from "./modal";

// jsdom has no layout, so offsetParent is always null. The modal uses it to
// skip hidden controls; make visible controls look visible for this test.
const offsetParent = Object.getOwnPropertyDescriptor(
  HTMLElement.prototype,
  "offsetParent",
);

beforeAll(() => {
  Object.defineProperty(HTMLElement.prototype, "offsetParent", {
    configurable: true,
    get() {
      return document.body;
    },
  });
});

afterAll(() => {
  if (offsetParent) {
    Object.defineProperty(HTMLElement.prototype, "offsetParent", offsetParent);
  } else {
    delete (HTMLElement.prototype as { offsetParent?: unknown }).offsetParent;
  }
});

function Harness() {
  const [open, setOpen] = useState(true);
  const [value, setValue] = useState("");
  return (
    <Modal open={open} onClose={() => setOpen(false)} title="Teste">
      <label htmlFor="campo">Campo</label>
      <input
        id="campo"
        value={value}
        onChange={(event) => setValue(event.target.value)}
      />
    </Modal>
  );
}

describe("Modal focus management", () => {
  it("keeps focus on the field while typing with an unstable onClose", async () => {
    render(<Harness />);
    const input = screen.getByLabelText("Campo");
    await waitFor(() => expect(input).toHaveFocus());
    const close = within(screen.getByRole("dialog")).getByRole("button", {
      name: "Fechar",
    });
    expect(close).not.toHaveFocus();

    fireEvent.change(input, { target: { value: "a" } });
    fireEvent.change(input, { target: { value: "a b" } });

    await waitFor(() => expect(input).toHaveFocus());
    expect(input).toHaveValue("a b");
    expect(screen.getByRole("dialog")).toBeInTheDocument();
  });

  it("respects a child autoFocus", async () => {
    function AutoFocusHarness() {
      return (
        <Modal open onClose={() => {}} title="Auto">
          <input aria-label="Auto" autoFocus />
        </Modal>
      );
    }
    render(<AutoFocusHarness />);
    await waitFor(() =>
      expect(screen.getByRole("textbox", { name: "Auto" })).toHaveFocus(),
    );
  });

  it("closes on Escape with the latest onClose callback", async () => {
    render(<Harness />);
    await waitFor(() => expect(screen.getByLabelText("Campo")).toHaveFocus());

    fireEvent.keyDown(window, { key: "Escape" });

    await waitFor(() =>
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument(),
    );
  });
});

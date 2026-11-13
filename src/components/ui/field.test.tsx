import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import {
  Field,
  FormError,
  Input,
  Select,
  Textarea,
} from "./field";

describe("form primitives", () => {
  it("associates the label with the control and shows the hint", () => {
    render(
      <Field label="Nome" htmlFor="name" hint="Use o nome do workspace">
        <Input id="name" defaultValue="Ops" />
      </Field>,
    );

    expect(screen.getByLabelText("Nome")).toHaveValue("Ops");
    expect(screen.getByText("Use o nome do workspace")).toBeInTheDocument();
  });

  it("prefers the error over the hint", () => {
    render(
      <Field label="Nome" htmlFor="name" hint="hint" error="Informe o nome">
        <Input id="name" />
      </Field>,
    );

    expect(screen.getByText("Informe o nome")).toBeInTheDocument();
    expect(screen.queryByText("hint")).not.toBeInTheDocument();
  });

  it("renders select and textarea with the shared styling", () => {
    render(
      <Field label="Modo" htmlFor="mode">
        <Select id="mode" defaultValue="build">
          <option value="build">build</option>
        </Select>
        <Textarea aria-label="Notas" />
      </Field>,
    );

    expect(screen.getByLabelText("Modo")).toHaveValue("build");
    expect(screen.getByLabelText("Notas")).toHaveClass("min-h-24");
  });

  it("renders form errors", () => {
    render(<FormError>Falhou</FormError>);
    expect(screen.getByText("Falhou")).toHaveClass("text-red-300");
  });
});

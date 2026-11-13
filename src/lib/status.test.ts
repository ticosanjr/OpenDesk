import { describe, expect, it } from "vitest";
import {
  recipeRunStatusLabel,
  recipeRunStatusTone,
  runStatusLabel,
  runStatusTone,
} from "./status";

describe("shared status labels", () => {
  it("labels and tones every run status", () => {
    expect(runStatusLabel("SUCCESS")).toBe("sucesso");
    expect(runStatusLabel("FAILED")).toBe("falhou");
    expect(runStatusLabel("RUNNING")).toBe("executando");
    expect(runStatusLabel("CANCELLED")).toBe("cancelada");
    expect(runStatusLabel("PENDING")).toBe("pendente");
    expect(runStatusLabel("FUTURE")).toBe("future");
    expect(runStatusTone("SUCCESS")).toBe("green");
    expect(runStatusTone("FAILED")).toBe("red");
    expect(runStatusTone("RUNNING")).toBe("blue");
    expect(runStatusTone("CANCELLED")).toBe("yellow");
    expect(runStatusTone("PENDING")).toBe("yellow");
    expect(runStatusTone("FUTURE")).toBe("default");
  });

  it("labels and tones recipe run statuses", () => {
    expect(recipeRunStatusLabel("COMPLETED")).toBe("concluída");
    expect(recipeRunStatusLabel("FAILED")).toBe("falhou");
    expect(recipeRunStatusLabel("REJECTED")).toBe("rejeitada");
    expect(recipeRunStatusLabel("RUNNING")).toBe("executando");
    expect(recipeRunStatusLabel("WAITING_APPROVAL")).toBe("aguardando");
    expect(recipeRunStatusLabel("WAITING_ACTION")).toBe("aguardando");
    expect(recipeRunStatusLabel("CUSTOM")).toBe("custom");
    expect(recipeRunStatusTone("COMPLETED")).toBe("green");
    expect(recipeRunStatusTone("FAILED")).toBe("red");
    expect(recipeRunStatusTone("REJECTED")).toBe("red");
    expect(recipeRunStatusTone("WAITING_APPROVAL")).toBe("yellow");
    expect(recipeRunStatusTone("RUNNING")).toBe("blue");
    expect(recipeRunStatusTone("CUSTOM")).toBe("default");
  });
});

import { beforeEach, describe, expect, it } from "vitest";
import { confirmAction, useConfirmStore } from "./confirm-store";

describe("confirm store", () => {
  beforeEach(() => {
    useConfirmStore.setState({ options: null, resolve: null });
  });

  it("resolves true when the user confirms", async () => {
    const pending = confirmAction({ title: "Excluir?", tone: "destructive" });
    expect(useConfirmStore.getState().options?.title).toBe("Excluir?");
    useConfirmStore.getState().settle(true);
    await expect(pending).resolves.toBe(true);
    expect(useConfirmStore.getState().options).toBeNull();
  });

  it("resolves false when canceled", async () => {
    const pending = confirmAction({ title: "Continuar?" });
    useConfirmStore.getState().settle(false);
    await expect(pending).resolves.toBe(false);
  });
});

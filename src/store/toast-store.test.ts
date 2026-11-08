import { beforeEach, describe, expect, it } from "vitest";
import { toast, useToastStore } from "./toast-store";

describe("toast store", () => {
  beforeEach(() => {
    useToastStore.setState({ toasts: [] });
  });

  it("pushes and dismisses toasts", () => {
    const id = toast({ tone: "success", title: "Salvo", description: "ok" });
    expect(useToastStore.getState().toasts).toHaveLength(1);
    expect(useToastStore.getState().toasts[0]).toMatchObject({
      id,
      tone: "success",
      title: "Salvo",
      duration: 4200,
    });
    useToastStore.getState().dismiss(id);
    expect(useToastStore.getState().toasts).toHaveLength(0);
  });

  it("deduplicates by key and keeps hrefs", () => {
    const first = toast({
      tone: "info",
      title: "Run",
      href: "/runs/run-1",
      key: "run:run-1",
    });
    const second = toast({
      tone: "info",
      title: "Run",
      href: "/runs/run-1",
      key: "run:run-1",
    });
    expect(second).toBe(first);
    expect(useToastStore.getState().toasts).toHaveLength(1);
    expect(useToastStore.getState().toasts[0].href).toBe("/runs/run-1");
  });

  it("keeps errors until dismissed and caps the stack", () => {
    const error = toast({ tone: "error", title: "Falhou" });
    expect(
      useToastStore.getState().toasts.find((item) => item.id === error)?.duration,
    ).toBe(0);
    for (let index = 0; index < 8; index += 1) {
      toast({ tone: "info", title: `Item ${index}` });
    }
    expect(useToastStore.getState().toasts.length).toBeLessThanOrEqual(5);
    useToastStore.getState().clear();
    expect(useToastStore.getState().toasts).toHaveLength(0);
  });
});

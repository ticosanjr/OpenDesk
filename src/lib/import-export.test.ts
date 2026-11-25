import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@/lib/api", () => ({ apiPost: vi.fn() }));
vi.mock("@/lib/download", () => ({ downloadBlob: vi.fn() }));

import { apiPost } from "@/lib/api";
import { downloadBlob } from "@/lib/download";
import {
  exportJsonBundle,
  importJsonBundle,
  pickJsonBundle,
  readJsonBundle,
  validateBundleEnvelope,
  type BundleEnvelope,
} from "./import-export";

beforeEach(() => {
  vi.clearAllMocks();
});

describe("import/export bundles", () => {
  it("validates schema_version and kind fail-closed", () => {
    const bundle = validateBundleEnvelope<{ items: number[] }>(
      {
        schema_version: 1,
        kind: "personal_context",
        exported_at: "",
        items: { global: [], workspace: [] },
      },
      "personal_context",
    );
    expect(bundle.kind).toBe("personal_context");

    expect(() =>
      validateBundleEnvelope(
        {
          schema_version: 1,
          kind: "structured_memory",
          exported_at: "",
          items: {},
        },
        "prompt_templates",
      ),
    ).toThrow(/kind incompatível/);
    expect(() =>
      validateBundleEnvelope(
        { schema_version: 2, kind: "prompt_templates", items: {} },
        "prompt_templates",
      ),
    ).toThrow(/schema_version/);
    expect(() =>
      validateBundleEnvelope(
        { schema_version: 1, kind: "prompt_templates" },
        "prompt_templates",
      ),
    ).toThrow(/items/);
  });

  it("downloads a versioned envelope without a source workspace", () => {
    exportJsonBundle("script_tools", { tools: [] }, "tools.json");
    const [blob, filename] = (downloadBlob as ReturnType<typeof vi.fn>).mock
      .calls[0] as [Blob, string];
    expect(filename).toBe("tools.json");
    expect(blob).toBeInstanceOf(Blob);
  });

  it("downloads a versioned envelope", () => {
    const name = exportJsonBundle(
      "personal_context",
      { global: [], workspace: [] },
      "contexto.json",
      "ws-1",
    );
    expect(name).toBe("contexto.json");
    const [blob, filename] = (downloadBlob as ReturnType<typeof vi.fn>).mock
      .calls[0] as [Blob, string];
    expect(filename).toBe("contexto.json");
    expect(blob).toBeInstanceOf(Blob);
  });

  it("reads a JSON file and rejects a wrong kind", async () => {
    const file = new File(
      [
        JSON.stringify({
          schema_version: 1,
          kind: "structured_memory",
          exported_at: "",
          items: { namespaces: [], notes: [], acls: {} },
        }),
      ],
      "memory.json",
      { type: "application/json" },
    );
    const bundle = await readJsonBundle<{ notes: unknown[] }>(
      file,
      "structured_memory",
    );
    expect(bundle.items.notes).toEqual([]);

    const wrong = new File([JSON.stringify({ ...bundle, kind: "starter_pack" })], "x.json");
    await expect(readJsonBundle(wrong, "structured_memory")).rejects.toThrow(
      /kind incompatível/,
    );
  });

  it("posts the envelope with workspace and mode", async () => {
    const bundle: BundleEnvelope = {
      schema_version: 1,
      kind: "structured_memory",
      exported_at: "",
      items: {},
    };
    await importJsonBundle("/api/memory/import", bundle, "ws-2", "replace");
    expect(apiPost).toHaveBeenCalledWith("/api/memory/import", {
      workspace_id: "ws-2",
      mode: "replace",
      bundle,
    });
  });
});

describe("pickJsonBundle", () => {
  it("returns a validated file and removes the picker", async () => {
    const promise = pickJsonBundle<{ tools: unknown[] }>("script_tools");
    const input = document.querySelector(
      'input[type="file"]',
    ) as HTMLInputElement;
    expect(input).not.toBeNull();
    const file = new File(
      [
        JSON.stringify({
          schema_version: 1,
          kind: "script_tools",
          exported_at: "",
          items: { tools: [] },
        }),
      ],
      "tools.json",
    );
    Object.defineProperty(input, "files", {
      configurable: true,
      value: [file],
    });
    input.dispatchEvent(new Event("change"));
    await expect(promise).resolves.toMatchObject({
      fileName: "tools.json",
      bundle: { kind: "script_tools" },
    });
    expect(document.querySelector('input[type="file"]')).toBeNull();
  });

  it("rejects a wrong kind and resolves null on cancel/empty", async () => {
    const rejected = pickJsonBundle("structured_memory");
    const input = document.querySelector(
      'input[type="file"]',
    ) as HTMLInputElement;
    const file = new File(
      [JSON.stringify({ schema_version: 1, kind: "script_tools", items: {} })],
      "wrong.json",
    );
    Object.defineProperty(input, "files", { configurable: true, value: [file] });
    input.dispatchEvent(new Event("change"));
    await expect(rejected).rejects.toThrow(/kind incompatível/);

    const cancelled = pickJsonBundle("script_tools");
    document
      .querySelector('input[type="file"]')
      ?.dispatchEvent(new Event("cancel"));
    await expect(cancelled).resolves.toBeNull();

    const empty = pickJsonBundle("script_tools");
    const emptyInput = document.querySelector(
      'input[type="file"]',
    ) as HTMLInputElement;
    Object.defineProperty(emptyInput, "files", {
      configurable: true,
      value: [],
    });
    emptyInput.dispatchEvent(new Event("change"));
    await expect(empty).resolves.toBeNull();
  });

  it("ignores late events and missing file lists", async () => {
    const promise = pickJsonBundle("script_tools");
    const input = document.querySelector('input[type="file"]') as HTMLInputElement;
    Object.defineProperty(input, "files", { configurable: true, value: undefined });
    input.dispatchEvent(new Event("change"));
    await expect(promise).resolves.toBeNull();

    const cancelled = pickJsonBundle("script_tools");
    const cancelInput = document.querySelector(
      'input[type="file"]',
    ) as HTMLInputElement;
    cancelInput.dispatchEvent(new Event("cancel"));
    cancelInput.dispatchEvent(new Event("cancel"));
    cancelInput.dispatchEvent(new Event("change"));
    await expect(cancelled).resolves.toBeNull();
  });

  it("rejects invalid JSON and non-object envelopes", async () => {
    await expect(
      readJsonBundle(new File(["{bad"], "bad.json"), "script_tools"),
    ).rejects.toThrow(/JSON inválido/);
    expect(() => validateBundleEnvelope("x", "script_tools")).toThrow(
      /objeto JSON/,
    );
    expect(() =>
      validateBundleEnvelope(
        { schema_version: 1, kind: "script_tools", items: undefined },
        "script_tools",
      ),
    ).toThrow(/items/);
  });
});

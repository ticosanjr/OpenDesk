import { describe, expect, it } from "vitest";
import { parseUnifiedDiff } from "./diff";

const patch = `diff --git a/src/a.ts b/src/a.ts
index 111..222 100644
--- a/src/a.ts
+++ b/src/a.ts
@@ -1,3 +1,4 @@
 const a = 1;
-const b = 2;
+const b = 3;
+const c = 4;
 export { a };
`;

describe("parseUnifiedDiff", () => {
  it("parses hunks, additions, deletions and line numbers", () => {
    const parsed = parseUnifiedDiff(patch);
    expect(parsed.additions).toBe(2);
    expect(parsed.deletions).toBe(1);
    const hunk = parsed.lines.find((line) => line.kind === "hunk");
    expect(hunk?.text).toContain("@@ -1,3 +1,4 @@");
    const added = parsed.lines.filter((line) => line.kind === "add");
    expect(added.map((line) => line.text)).toEqual([
      "const b = 3;",
      "const c = 4;",
    ]);
    expect(added[0].newLine).toBe(2);
    expect(added[1].newLine).toBe(3);
    const removed = parsed.lines.find((line) => line.kind === "del");
    expect(removed?.text).toBe("const b = 2;");
    expect(removed?.oldLine).toBe(2);
  });

  it("keeps context line numbers after changes", () => {
    const parsed = parseUnifiedDiff(patch);
    const context = parsed.lines.filter((line) => line.kind === "context");
    expect(context.at(-1)).toMatchObject({
      text: "export { a };",
      oldLine: 3,
      newLine: 4,
    });
  });

  it("does not treat file headers as additions or deletions", () => {
    const parsed = parseUnifiedDiff(patch);
    expect(
      parsed.lines.filter((line) => line.kind === "meta").map((line) => line.text),
    ).toContain("--- a/src/a.ts");
    expect(parsed.lines.filter((line) => line.kind === "meta")).toHaveLength(4);
  });

  it("handles empty patches", () => {
    const parsed = parseUnifiedDiff("");
    expect(parsed.lines).toEqual([]);
    expect(parsed.additions).toBe(0);
    expect(parsed.deletions).toBe(0);
  });
});

describe("diff edge cases", () => {
  it("handles CRLF and no-newline markers", () => {
    const parsed = parseUnifiedDiff(
      "@@ -1 +1 @@\r\n-old\r\n+new\r\n\\ No newline at end of file\r\n",
    );
    expect(parsed.additions).toBe(1);
    expect(parsed.deletions).toBe(1);
    expect(parsed.lines.some((line) => line.text.includes("No newline"))).toBe(
      true,
    );
  });

  it("resets line counters across hunks", () => {
    const parsed = parseUnifiedDiff(
      "@@ -1 +1 @@\n-a\n+b\n@@ -10 +10 @@\n-c\n+d\n",
    );
    const hunks = parsed.lines.filter((line) => line.kind === "hunk");
    expect(hunks).toHaveLength(2);
    const added = parsed.lines.filter((line) => line.kind === "add");
    expect(added.map((line) => line.newLine)).toEqual([1, 10]);
  });

  it("keeps unmarked text as context", () => {
    const parsed = parseUnifiedDiff("texto sem marcador\n");
    expect(parsed.lines[0]).toMatchObject({
      kind: "context",
      text: "texto sem marcador",
    });
  });
});

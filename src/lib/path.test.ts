import { describe, expect, it } from "vitest";
import { basename } from "./path";

describe("basename", () => {
  it("handles unix, windows and trailing separators", () => {
    expect(basename("/a/b/c.txt")).toBe("c.txt");
    expect(basename("C:\\a\\b\\c.txt")).toBe("c.txt");
    expect(basename("/a/b/")).toBe("b");
    expect(basename("file.md")).toBe("file.md");
    expect(basename("")).toBe("");
  });
});

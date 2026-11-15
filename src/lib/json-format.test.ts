import { describe, expect, it } from "vitest";
import { formatJsonContent } from "./json-format";

describe("formatJsonContent", () => {
  it("pretty prints valid JSON with two spaces", () => {
    expect(formatJsonContent('{"b":2,"a":[1,2]}')).toEqual({
      text: '{\n  "b": 2,\n  "a": [\n    1,\n    2\n  ]\n}',
      formatted: true,
    });
  });

  it("formats JSON Lines line by line", () => {
    const result = formatJsonContent('{"a":1}\n{"b":2}\n');
    expect(result.formatted).toBe(true);
    expect(result.text).toBe('{"a":1}\n{"b":2}');
  });

  it("keeps invalid JSON untouched", () => {
    expect(formatJsonContent('{"a": // comment\n1}')).toEqual({
      text: '{"a": // comment\n1}',
      formatted: false,
    });
  });
});

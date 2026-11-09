import { describe, expect, it } from "vitest";
import { queueBadgeLabel, queueCounts } from "./queue";

describe("queue badge helpers", () => {
  it("counts only active jobs", () => {
    const counts = queueCounts([
      { status: "QUEUED" },
      { status: "QUEUED" },
      { status: "RUNNING" },
      { status: "COMPLETED" },
      { status: "FAILED" },
      { status: "CANCELLED" },
    ]);
    expect(counts).toEqual({ waiting: 2, running: 1, active: 3 });
    expect(queueBadgeLabel(counts)).toBe("2 aguardando · 1 executando");
  });

  it("hides the badge when nothing is active", () => {
    expect(queueBadgeLabel(queueCounts([{ status: "COMPLETED" }]))).toBe("");
    expect(queueBadgeLabel(queueCounts([]))).toBe("");
  });

  it("labels running-only and waiting-only queues", () => {
    expect(queueBadgeLabel(queueCounts([{ status: "RUNNING" }]))).toBe(
      "0 aguardando · 1 executando",
    );
    expect(queueBadgeLabel(queueCounts([{ status: "QUEUED" }]))).toBe(
      "1 aguardando",
    );
  });
});

import { describe, expect, it } from "vitest";
import { isWorkQueueJob, queueBadgeLabel, queueCounts, workQueue } from "./queue";

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

describe("workQueue", () => {
  it("keeps manual/capture jobs and hides automation jobs", () => {
    const jobs = [
      { id: "a", task_origin: "MANUAL" },
      { id: "b", task_origin: "CAPTURE" },
      { id: "c", task_origin: "RECIPE" },
      { id: "d", task_origin: "ROUTINE" },
      { id: "e", task_origin: null },
    ];
    expect(workQueue(jobs).map((job) => job.id)).toEqual(["a", "b"]);
    expect(isWorkQueueJob({ task_origin: "MANUAL" })).toBe(true);
    expect(isWorkQueueJob({ task_origin: "RECIPE" })).toBe(false);
  });
});

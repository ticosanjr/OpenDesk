export interface QueueCounts {
  waiting: number;
  running: number;
  active: number;
}

const WORK_ORIGINS = new Set(["MANUAL", "CAPTURE"]);

/** Kanban/Fila only shows tasks created directly by the user. */
export function isWorkQueueJob(job: {
  task_origin?: string | null;
}): boolean {
  return WORK_ORIGINS.has(job.task_origin ?? "");
}

export function workQueue<T extends { task_origin?: string | null }>(
  jobs: T[],
): T[] {
  return jobs.filter(isWorkQueueJob);
}

/** Real engine pressure, including routines, recipes and I/O automations. */
export function allQueueCounts<T extends { status: string }>(
  jobs: T[],
): QueueCounts {
  return queueCounts(jobs);
}

export function queueCounts(jobs: Array<{ status: string }>): QueueCounts {
  let waiting = 0;
  let running = 0;
  for (const job of jobs) {
    if (job.status === "QUEUED") waiting += 1;
    else if (job.status === "RUNNING") running += 1;
  }
  return { waiting, running, active: waiting + running };
}

export function queueBadgeLabel(counts: QueueCounts): string {
  if (counts.active === 0) return "";
  return `${counts.waiting} aguardando${
    counts.running > 0 ? ` · ${counts.running} executando` : ""
  }`;
}

export interface QueueCounts {
  waiting: number;
  running: number;
  active: number;
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

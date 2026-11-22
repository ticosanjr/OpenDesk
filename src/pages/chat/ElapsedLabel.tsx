import { useEffect, useState } from "react";

/** Isolated so the one-second timer does not re-render the whole chat page. */
export function ElapsedLabel({
  startedAt,
  running,
}: {
  startedAt: string | null;
  running: boolean;
}) {
  const [elapsed, setElapsed] = useState(0);
  useEffect(() => {
    if (!running || !startedAt) {
      setElapsed(0);
      return;
    }
    const started = Date.parse(startedAt);
    const update = () =>
      setElapsed(
        Number.isFinite(started)
          ? Math.max(0, Math.round((Date.now() - started) / 1000))
          : 0,
      );
    update();
    const interval = window.setInterval(update, 1_000);
    return () => window.clearInterval(interval);
  }, [running, startedAt]);
  return elapsed > 0 ? <> · {elapsed}s</> : null;
}

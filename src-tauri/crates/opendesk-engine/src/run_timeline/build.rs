use ::opendesk_storage::repos::runs::TaskRun;
use serde_json::Value;

use super::RunTimeline;
use super::accumulator::Accumulator;

pub fn timeline_from_events(
    run: Option<TaskRun>,
    events: impl IntoIterator<Item = Value>,
    log_missing: bool,
) -> RunTimeline {
    let mut timeline = RunTimeline {
        run,
        log_missing,
        ..Default::default()
    };
    let mut accumulator = Accumulator::default();
    for event in events {
        accumulator.observe(&mut timeline, event);
    }
    accumulator.finish(&mut timeline);
    timeline
}

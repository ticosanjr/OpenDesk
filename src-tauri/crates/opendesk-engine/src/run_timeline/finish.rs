use super::accumulator::Accumulator;
use super::helpers::{duration, iso, parse_iso_ms, tool_summary, truncate};
use super::{MAX_TEXT, MAX_TOOL_OUTPUT, RunTimeline, TimelineSection, TimelineTool};

impl Accumulator {
    pub(super) fn finish(self, timeline: &mut RunTimeline) {
        if timeline.status.is_empty()
            && let Some(run) = &timeline.run
        {
            timeline.status = run.status.clone();
        }
        timeline.started_at = self
            .first_ms
            .and_then(iso)
            .or_else(|| timeline.run.as_ref().map(|run| run.started_at.clone()));
        let terminal = matches!(timeline.status.as_str(), "SUCCESS" | "FAILED" | "CANCELLED");
        timeline.completed_at = if terminal {
            timeline.completed_at.clone().or_else(|| {
                timeline
                    .run
                    .as_ref()
                    .and_then(|run| run.completed_at.clone())
                    .or_else(|| self.last_ms.and_then(iso))
            })
        } else {
            None
        };
        timeline.duration_ms = timeline
            .run
            .as_ref()
            .map(|run| run.duration_ms)
            .filter(|duration| *duration > 0)
            .unwrap_or_else(|| match (self.first_ms, self.last_ms) {
                (Some(start), Some(end)) => (end - start).max(0),
                _ => 0,
            });

        let (thinking_text, thinking_truncated) = truncate(self.thinking_text, MAX_TEXT);
        timeline.thinking = TimelineSection {
            started_at: self.thinking_first.and_then(iso),
            ended_at: self.thinking_last.and_then(iso),
            duration_ms: duration(self.thinking_first, self.thinking_last),
            text: thinking_text,
            truncated: thinking_truncated,
            ..Default::default()
        };

        let mut timeline_tools: Vec<TimelineTool> = self
            .tools
            .into_iter()
            .map(|tool| {
                let (output, output_truncated) = truncate(tool.output, MAX_TOOL_OUTPUT);
                let started = tool.started_ms;
                let ended = tool.ended_ms;
                TimelineTool {
                    summary: tool_summary(&tool.name, &tool.input),
                    id: tool.id,
                    name: tool.name,
                    input: tool.input,
                    output,
                    status: tool.status,
                    started_at: started.and_then(iso),
                    ended_at: ended.and_then(iso),
                    duration_ms: duration(started, ended),
                    output_truncated,
                }
            })
            .collect();
        timeline_tools.sort_by(|left, right| {
            left.started_at
                .as_deref()
                .unwrap_or_default()
                .cmp(right.started_at.as_deref().unwrap_or_default())
        });
        let executed_first = timeline_tools
            .iter()
            .filter_map(|tool| tool.started_at.as_deref().and_then(parse_iso_ms))
            .min();
        let executed_last = timeline_tools
            .iter()
            .filter_map(|tool| tool.ended_at.as_deref().and_then(parse_iso_ms))
            .max();
        timeline.executed = TimelineSection {
            started_at: executed_first.and_then(iso),
            ended_at: executed_last.and_then(iso),
            duration_ms: duration(executed_first, executed_last),
            tools: timeline_tools.clone(),
            ..Default::default()
        };
        for event in &mut timeline.events {
            if event.event_type.is_empty() {
                event.event_type = event.kind.clone();
            }
            if let Some(tool) = &event.tool
                && let Some(final_tool) = timeline_tools.iter().find(|item| item.id == tool.id)
            {
                event.tool = Some(final_tool.clone());
            }
            if event.kind == "thinking" || event.kind == "comment" {
                let (text, truncated) = truncate(event.text.clone(), MAX_TEXT);
                event.text = text;
                if truncated {
                    event.text.push_str("\n\n[truncado]");
                }
            }
        }

        let (result_text, result_truncated) = truncate(self.result_text, MAX_TEXT);
        timeline.result = TimelineSection {
            started_at: self
                .result_first
                .and_then(iso)
                .or_else(|| timeline.started_at.clone()),
            ended_at: self
                .result_last
                .and_then(iso)
                .or_else(|| timeline.completed_at.clone()),
            duration_ms: duration(self.result_first, self.result_last),
            text: result_text,
            truncated: result_truncated,
            ..Default::default()
        };
    }
}

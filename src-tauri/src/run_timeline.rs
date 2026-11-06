use crate::db::runs::TaskRun;
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Default)]
pub struct RunTimeline {
    pub run: Option<TaskRun>,
    pub status: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub duration_ms: i64,
    pub events_count: usize,
    pub steps: Vec<TimelineStep>,
    pub thinking: TimelineSection,
    pub executed: TimelineSection,
    pub result: TimelineSection,
    pub log_missing: bool,
    pub workspace_root: Option<String>,
    pub events: Vec<TimelineEvent>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct TimelineEvent {
    pub kind: String,
    pub role: String,
    pub text: String,
    pub tool: Option<TimelineTool>,
    pub step_index: Option<usize>,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    pub duration_ms: i64,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct TimelineSection {
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    pub duration_ms: i64,
    pub text: String,
    pub tools: Vec<TimelineTool>,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct TimelineStep {
    pub index: usize,
    pub agent: Option<String>,
    pub model: Option<String>,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    pub duration_ms: i64,
    pub tokens_input: i64,
    pub tokens_output: i64,
    pub tokens_reasoning: i64,
    pub cost_usd: f64,
    pub finish: Option<String>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct TimelineTool {
    pub id: String,
    pub name: String,
    pub summary: String,
    pub input: String,
    pub output: String,
    pub status: String,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    pub duration_ms: i64,
    pub output_truncated: bool,
}

const MAX_TEXT: usize = 64 * 1024;
const MAX_TOOL_OUTPUT: usize = 6 * 1024;

#[derive(Default)]
struct ToolAccumulator {
    id: String,
    name: String,
    input: String,
    output: String,
    status: String,
    started_ms: Option<i64>,
    ended_ms: Option<i64>,
}

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
    let mut first_ms: Option<i64> = None;
    let mut last_ms: Option<i64> = None;
    let mut thinking_first: Option<i64> = None;
    let mut thinking_last: Option<i64> = None;
    let mut thinking_text = String::new();
    let mut reasoning_started: Option<i64> = None;
    let mut tools: Vec<ToolAccumulator> = Vec::new();
    let mut tool_index: HashMap<String, usize> = HashMap::new();
    let mut result_text = String::new();
    let mut result_first: Option<i64> = None;
    let mut result_last: Option<i64> = None;
    let mut text_started: Option<i64> = None;
    let mut step_stack: Vec<usize> = Vec::new();
    let mut current_thinking: Option<usize> = None;
    let mut current_comment: Option<usize> = None;
    let mut tool_events: HashMap<String, usize> = HashMap::new();
    let mut step_events: HashMap<usize, usize> = HashMap::new();

    for event in events {
        timeline.events_count += 1;
        let Some(created) = event.get("created").and_then(Value::as_i64) else {
            continue;
        };
        first_ms = Some(first_ms.map_or(created, |value| value.min(created)));
        last_ms = Some(last_ms.map_or(created, |value| value.max(created)));
        if timeline.workspace_root.is_none()
            && let Some(directory) = event.pointer("/location/directory").and_then(Value::as_str)
        {
            timeline.workspace_root = Some(directory.to_string());
        }
        let kind = event
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let data = event.get("data").cloned().unwrap_or(Value::Null);

        match kind {
            "session.step.started" => {
                let index = timeline.steps.len() + 1;
                let started = data
                    .get("started")
                    .and_then(Value::as_i64)
                    .unwrap_or(created);
                timeline.steps.push(TimelineStep {
                    index,
                    agent: data
                        .get("agent")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    model: data.get("model").and_then(|model| {
                        let provider = model.get("providerID").and_then(Value::as_str)?;
                        let id = model.get("id").and_then(Value::as_str)?;
                        let variant = model.get("variant").and_then(Value::as_str);
                        Some(match variant {
                            Some(variant) => format!("{provider}/{id}#{variant}"),
                            None => format!("{provider}/{id}"),
                        })
                    }),
                    started_at: iso(started),
                    duration_ms: (created - started).max(0),
                    ..Default::default()
                });
                step_stack.push(index - 1);
                timeline.events.push(TimelineEvent {
                    kind: "step".into(),
                    role: "system".into(),
                    text: format!("Passo {}", index),
                    step_index: Some(index),
                    started_at: iso(created),
                    ..Default::default()
                });
                step_events.insert(index, timeline.events.len() - 1);
            }
            "session.step.ended" => {
                if let Some(index) = step_stack.pop()
                    && let Some(step) = timeline.steps.get_mut(index)
                {
                    let started = step
                        .started_at
                        .as_deref()
                        .and_then(parse_iso_ms)
                        .unwrap_or(created);
                    step.ended_at = iso(created);
                    step.duration_ms = (created - started).max(0);
                    step.finish = data
                        .get("finish")
                        .and_then(Value::as_str)
                        .map(str::to_string);
                    step.cost_usd = data.get("cost").and_then(Value::as_f64).unwrap_or(0.0);
                    if let Some(tokens) = data.get("tokens") {
                        step.tokens_input =
                            tokens.get("input").and_then(Value::as_i64).unwrap_or(0);
                        step.tokens_output =
                            tokens.get("output").and_then(Value::as_i64).unwrap_or(0);
                        step.tokens_reasoning =
                            tokens.get("reasoning").and_then(Value::as_i64).unwrap_or(0);
                    }
                }
            }
            "session.reasoning.started" => {
                reasoning_started = Some(created);
                thinking_first = Some(thinking_first.map_or(created, |value| value.min(created)));
                thinking_last = Some(thinking_last.map_or(created, |value| value.max(created)));
                timeline.events.push(TimelineEvent {
                    kind: "thinking".into(),
                    role: "assistant".into(),
                    started_at: iso(created),
                    ..Default::default()
                });
                current_thinking = Some(timeline.events.len() - 1);
            }
            "session.reasoning.delta" => {
                let delta = data.get("delta").and_then(Value::as_str).unwrap_or("");
                thinking_text.push_str(delta);
                if current_thinking.is_none() {
                    timeline.events.push(TimelineEvent {
                        kind: "thinking".into(),
                        role: "assistant".into(),
                        started_at: iso(created),
                        ..Default::default()
                    });
                    current_thinking = Some(timeline.events.len() - 1);
                }
                if let Some(index) = current_thinking
                    && let Some(event) = timeline.events.get_mut(index)
                {
                    event.text.push_str(delta);
                    event.ended_at = iso(created);
                    let start = event.started_at.as_deref().and_then(parse_iso_ms);
                    event.duration_ms = duration(start, Some(created));
                }
                thinking_first = Some(thinking_first.map_or(created, |value| value.min(created)));
                thinking_last = Some(thinking_last.map_or(created, |value| value.max(created)));
            }
            "session.reasoning.ended" => {
                if thinking_text.trim().is_empty()
                    && let Some(text) = data.get("text").and_then(Value::as_str)
                {
                    thinking_text.push_str(text);
                }
                if let Some(index) = current_thinking
                    && let Some(event) = timeline.events.get_mut(index)
                {
                    if event.text.trim().is_empty()
                        && let Some(text) = data.get("text").and_then(Value::as_str)
                    {
                        event.text.push_str(text);
                    }
                    event.ended_at = iso(created);
                    let start = event.started_at.as_deref().and_then(parse_iso_ms);
                    event.duration_ms = duration(start, Some(created));
                }
                current_thinking = None;
                thinking_last = Some(thinking_last.map_or(created, |value| value.max(created)));
                reasoning_started = None;
            }
            "session.tool.input.started" => {
                let id = data.get("id").and_then(Value::as_str).unwrap_or_default();
                if !id.is_empty() {
                    let index = *tool_index.entry(id.to_string()).or_insert_with(|| {
                        tools.push(ToolAccumulator {
                            id: id.to_string(),
                            status: "running".into(),
                            ..Default::default()
                        });
                        tools.len() - 1
                    });
                    tools[index].name = data
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or("tool")
                        .to_string();
                    tools[index].started_ms = Some(created);
                    let name = tools[index].name.clone();
                    let id = tools[index].id.clone();
                    timeline.events.push(TimelineEvent {
                        kind: "tool".into(),
                        role: "tool".into(),
                        text: name.clone(),
                        tool: Some(TimelineTool {
                            id: id.clone(),
                            name,
                            status: "running".into(),
                            started_at: iso(created),
                            ..Default::default()
                        }),
                        started_at: iso(created),
                        ..Default::default()
                    });
                    tool_events.insert(id, timeline.events.len() - 1);
                }
            }
            "session.tool.called" => {
                let id = data.get("id").and_then(Value::as_str).unwrap_or_default();
                if !id.is_empty() {
                    let index = *tool_index.entry(id.to_string()).or_insert_with(|| {
                        tools.push(ToolAccumulator {
                            id: id.to_string(),
                            status: "running".into(),
                            ..Default::default()
                        });
                        tools.len() - 1
                    });
                    if tools[index].name.is_empty() {
                        tools[index].name = data
                            .get("name")
                            .and_then(Value::as_str)
                            .unwrap_or("tool")
                            .to_string();
                    }
                    if let Some(input) = data.get("input") {
                        tools[index].input = input.to_string();
                    }
                    tools[index].started_ms.get_or_insert(created);
                    if !tool_events.contains_key(id) {
                        let name = tools[index].name.clone();
                        let id_value = tools[index].id.clone();
                        timeline.events.push(TimelineEvent {
                            kind: "tool".into(),
                            role: "tool".into(),
                            text: name.clone(),
                            tool: Some(TimelineTool {
                                id: id_value.clone(),
                                name,
                                status: "running".into(),
                                started_at: iso(created),
                                ..Default::default()
                            }),
                            started_at: iso(created),
                            ..Default::default()
                        });
                        tool_events.insert(id_value, timeline.events.len() - 1);
                    }
                }
            }
            "session.tool.input.ended" => {
                let id = data.get("id").and_then(Value::as_str).unwrap_or_default();
                if let Some(index) = tool_index.get(id) {
                    if let Some(text) = data.get("text").and_then(Value::as_str) {
                        tools[*index].input = text.to_string();
                    }
                    if let Some(event_index) = tool_events.get(id).copied()
                        && let Some(event) = timeline.events.get_mut(event_index)
                        && let Some(tool) = event.tool.as_mut()
                    {
                        tool.input = tools[*index].input.clone();
                        tool.summary = tool_summary(&tool.name, &tool.input);
                    }
                }
            }
            "session.tool.success" | "session.tool.failed" => {
                let id = data.get("id").and_then(Value::as_str).unwrap_or_default();
                if let Some(index) = tool_index.get(id) {
                    tools[*index].status = if kind == "session.tool.success" {
                        "success".into()
                    } else {
                        "failed".into()
                    };
                    tools[*index].ended_ms = Some(created);
                    tools[*index].output = tool_output(&data);
                    if let Some(event_index) = tool_events.get(id).copied()
                        && let Some(event) = timeline.events.get_mut(event_index)
                        && let Some(tool) = event.tool.as_mut()
                    {
                        tool.status = tools[*index].status.clone();
                        tool.output = tools[*index].output.clone();
                        tool.ended_at = iso(created);
                        let start = tool.started_at.as_deref().and_then(parse_iso_ms);
                        tool.duration_ms = duration(start, Some(created));
                        event.ended_at = iso(created);
                        event.duration_ms = duration(start, Some(created));
                    }
                }
            }
            "session.text.started" => {
                text_started = Some(created);
                timeline.events.push(TimelineEvent {
                    kind: "comment".into(),
                    role: "assistant".into(),
                    started_at: iso(created),
                    ..Default::default()
                });
                current_comment = Some(timeline.events.len() - 1);
            }
            "session.text.delta" => {
                let delta = data.get("delta").and_then(Value::as_str).unwrap_or("");
                if result_text.is_empty() {
                    result_text.push_str(delta);
                }
                if current_comment.is_none() {
                    timeline.events.push(TimelineEvent {
                        kind: "comment".into(),
                        role: "assistant".into(),
                        started_at: iso(created),
                        ..Default::default()
                    });
                    current_comment = Some(timeline.events.len() - 1);
                }
                if let Some(index) = current_comment
                    && let Some(event) = timeline.events.get_mut(index)
                {
                    event.text.push_str(delta);
                    event.ended_at = iso(created);
                    let start = event.started_at.as_deref().and_then(parse_iso_ms);
                    event.duration_ms = duration(start, Some(created));
                }
                result_first = Some(result_first.map_or(created, |value| value.min(created)));
                result_last = Some(result_last.map_or(created, |value| value.max(created)));
            }
            "session.text.ended" => {
                if let Some(text) = data.get("text").and_then(Value::as_str)
                    && !text.trim().is_empty()
                {
                    result_text = text.to_string();
                }
                result_first = Some(
                    result_first
                        .or(text_started)
                        .map_or(created, |value| value.min(created)),
                );
                if let Some(index) = current_comment
                    && let Some(event) = timeline.events.get_mut(index)
                {
                    if event.text.trim().is_empty()
                        && let Some(text) = data.get("text").and_then(Value::as_str)
                    {
                        event.text.push_str(text);
                    }
                    event.ended_at = iso(created);
                    let start = event.started_at.as_deref().and_then(parse_iso_ms);
                    event.duration_ms = duration(start, Some(created));
                }
                current_comment = None;
                result_last = Some(result_last.map_or(created, |value| value.max(created)));
            }
            "session.execution.succeeded" | "session.execution.failed" => {
                timeline.status = if kind == "session.execution.succeeded" {
                    "SUCCESS".into()
                } else {
                    "FAILED".into()
                };
                timeline.completed_at = iso(created);
            }
            _ => {}
        }

        let _ = reasoning_started;
    }

    if timeline.status.is_empty()
        && let Some(run) = &timeline.run
    {
        timeline.status = run.status.clone();
    }
    timeline.started_at = first_ms
        .and_then(iso)
        .or_else(|| timeline.run.as_ref().map(|run| run.started_at.clone()));
    timeline.completed_at = timeline.completed_at.or_else(|| {
        timeline
            .run
            .as_ref()
            .and_then(|run| run.completed_at.clone())
            .or_else(|| last_ms.and_then(iso))
    });
    timeline.duration_ms = timeline
        .run
        .as_ref()
        .map(|run| run.duration_ms)
        .filter(|duration| *duration > 0)
        .unwrap_or_else(|| match (first_ms, last_ms) {
            (Some(start), Some(end)) => (end - start).max(0),
            _ => 0,
        });

    let (thinking_text, thinking_truncated) = truncate(thinking_text, MAX_TEXT);
    timeline.thinking = TimelineSection {
        started_at: thinking_first.and_then(iso),
        ended_at: thinking_last.and_then(iso),
        duration_ms: duration(thinking_first, thinking_last),
        text: thinking_text,
        truncated: thinking_truncated,
        ..Default::default()
    };

    let mut timeline_tools: Vec<TimelineTool> = tools
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

    let (result_text, result_truncated) = truncate(result_text, MAX_TEXT);
    timeline.result = TimelineSection {
        started_at: result_first
            .and_then(iso)
            .or_else(|| timeline.started_at.clone()),
        ended_at: result_last
            .and_then(iso)
            .or_else(|| timeline.completed_at.clone()),
        duration_ms: duration(result_first, result_last),
        text: result_text,
        truncated: result_truncated,
        ..Default::default()
    };

    timeline
}

fn tool_output(data: &Value) -> String {
    if let Some(content) = data.get("content").and_then(Value::as_array) {
        let text = content
            .iter()
            .filter_map(|part| part.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n");
        if !text.is_empty() {
            return text;
        }
    }
    data.get("error")
        .map(|error| error.to_string())
        .unwrap_or_default()
}

fn tool_summary(name: &str, input: &str) -> String {
    let parsed = serde_json::from_str::<Value>(input).ok();
    let field = |key: &str| {
        parsed
            .as_ref()
            .and_then(|value| value.get(key))
            .and_then(Value::as_str)
    };
    let target = field("command")
        .or_else(|| field("path"))
        .or_else(|| field("filePath"))
        .or_else(|| field("file_path"))
        .or_else(|| field("pattern"))
        .or_else(|| field("query"));
    match target {
        Some(value) => format!("{} · {}", name, shorten(value, 120)),
        None => format!("{} · {}", name, shorten(input, 120)),
    }
}

fn shorten(value: &str, max: usize) -> String {
    let single = value.replace('\n', " ");
    if single.chars().count() <= max {
        single
    } else {
        format!("{}…", single.chars().take(max).collect::<String>())
    }
}

fn truncate(value: String, max: usize) -> (String, bool) {
    if value.len() <= max {
        return (value, false);
    }
    let mut end = max;
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    (format!("{}…", &value[..end]), true)
}

fn duration(start: Option<i64>, end: Option<i64>) -> i64 {
    match (start, end) {
        (Some(start), Some(end)) => (end - start).max(0),
        _ => 0,
    }
}

fn iso(millis: i64) -> Option<String> {
    DateTime::<Utc>::from_timestamp_millis(millis).map(|value| value.to_rfc3339())
}

fn parse_iso_ms(value: &str) -> Option<i64> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|value| value.timestamp_millis())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn events() -> Vec<Value> {
        vec![
            json!({"created": 1000, "type": "session.step.started", "location": {"directory": "/tmp/workspace"}, "data": {"agent": "build", "model": {"providerID": "opencode-go", "id": "deepseek", "variant": "low"}, "started": 900}}),
            json!({"created": 1100, "type": "session.reasoning.started", "data": {}}),
            json!({"created": 1200, "type": "session.reasoning.delta", "data": {"delta": "vou olhar "}}),
            json!({"created": 1500, "type": "session.reasoning.delta", "data": {"delta": "o workspace"}}),
            json!({"created": 1600, "type": "session.reasoning.ended", "data": {"text": "vou olhar o workspace"}}),
            json!({"created": 1700, "type": "session.tool.input.started", "data": {"id": "call_1", "name": "read"}}),
            json!({"created": 1750, "type": "session.tool.input.ended", "data": {"id": "call_1", "text": "{\"path\":\"/tmp/a.md\"}"}}),
            json!({"created": 1900, "type": "session.tool.success", "data": {"id": "call_1", "content": [{"type": "text", "text": "conteúdo"}]}}),
            json!({"created": 2000, "type": "session.text.started", "data": {}}),
            json!({"created": 2100, "type": "session.text.delta", "data": {"delta": "resultado "}}),
            json!({"created": 2300, "type": "session.text.ended", "data": {"text": "resultado final"}}),
            json!({"created": 2500, "type": "session.step.ended", "data": {"cost": 0.01, "finish": "stop", "tokens": {"input": 10, "output": 5, "reasoning": 2}}}),
            json!({"created": 2600, "type": "session.execution.succeeded", "data": {}}),
        ]
    }

    #[test]
    fn builds_thinking_executed_result_bands() {
        let timeline = timeline_from_events(None, events(), false);
        assert_eq!(timeline.status, "SUCCESS");
        assert_eq!(timeline.events_count, 13);
        assert_eq!(timeline.workspace_root.as_deref(), Some("/tmp/workspace"));
        assert_eq!(timeline.duration_ms, 1600);
        assert_eq!(timeline.thinking.text, "vou olhar o workspace");
        assert_eq!(timeline.thinking.duration_ms, 500);
        assert_eq!(timeline.executed.tools.len(), 1);
        assert_eq!(timeline.executed.tools[0].name, "read");
        assert_eq!(timeline.executed.tools[0].status, "success");
        assert_eq!(timeline.executed.tools[0].duration_ms, 200);
        assert_eq!(timeline.executed.tools[0].summary, "read · /tmp/a.md");
        assert_eq!(timeline.result.text, "resultado final");
        assert_eq!(timeline.steps.len(), 1);
        assert_eq!(timeline.steps[0].tokens_input, 10);
        assert_eq!(timeline.steps[0].duration_ms, 1600);
        assert!(timeline.events.iter().any(|event| event.kind == "step"));
        assert!(timeline.events.iter().any(|event| event.kind == "thinking"));
        assert!(timeline.events.iter().any(|event| event.kind == "tool"));
        assert!(timeline.events.iter().any(|event| event.kind == "comment"));
        let kinds = timeline
            .events
            .iter()
            .map(|event| event.kind.as_str())
            .collect::<Vec<_>>();
        let thinking = kinds.iter().position(|kind| *kind == "thinking").unwrap();
        let tool = kinds.iter().position(|kind| *kind == "tool").unwrap();
        let comment = kinds.iter().position(|kind| *kind == "comment").unwrap();
        assert!(thinking < tool && tool < comment);
    }

    #[test]
    fn tolerates_events_without_timestamps() {
        let timeline = timeline_from_events(
            None,
            vec![json!({"type": "session.text.delta", "data": {"delta": "x"}})],
            false,
        );
        assert_eq!(timeline.events_count, 1);
        assert!(timeline.started_at.is_none());
        assert!(timeline.result.text.is_empty());
    }

    #[test]
    fn marks_missing_logs() {
        let timeline = timeline_from_events(None, Vec::new(), true);
        assert!(timeline.log_missing);
        assert!(timeline.thinking.text.is_empty());
    }
}

use serde_json::Value;
use std::collections::HashMap;

use super::helpers::{duration, iso, parse_iso_ms, tool_output, tool_summary, truncate};
use super::{MAX_TOOL_OUTPUT, RunTimeline, TimelineEvent, TimelineStep, TimelineTool};

#[derive(Default)]
pub(super) struct ToolAccumulator {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) input: String,
    pub(super) output: String,
    pub(super) status: String,
    pub(super) started_ms: Option<i64>,
    pub(super) ended_ms: Option<i64>,
}

#[derive(Default)]
pub(super) struct Accumulator {
    pub(super) first_ms: Option<i64>,
    pub(super) last_ms: Option<i64>,
    pub(super) thinking_first: Option<i64>,
    pub(super) thinking_last: Option<i64>,
    pub(super) thinking_text: String,
    pub(super) tools: Vec<ToolAccumulator>,
    pub(super) tool_index: HashMap<String, usize>,
    pub(super) result_text: String,
    pub(super) result_first: Option<i64>,
    pub(super) result_last: Option<i64>,
    pub(super) text_started: Option<i64>,
    pub(super) step_stack: Vec<usize>,
    pub(super) current_thinking: Option<usize>,
    pub(super) current_comment: Option<usize>,
    pub(super) tool_events: HashMap<String, usize>,
}

impl Accumulator {
    pub(super) fn observe(&mut self, timeline: &mut RunTimeline, event: Value) {
        timeline.events_count += 1;
        let Some(created) = event.get("created").and_then(Value::as_i64) else {
            return;
        };
        self.first_ms = Some(self.first_ms.map_or(created, |value| value.min(created)));
        self.last_ms = Some(self.last_ms.map_or(created, |value| value.max(created)));
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

        let mut reasoning_started: Option<i64> = None;
        let mut step_events: HashMap<usize, usize> = HashMap::new();
        let mut tools = std::mem::take(&mut self.tools);
        let mut tool_index = std::mem::take(&mut self.tool_index);

        match kind {
            "session.execution.started" => {
                if timeline.status.is_empty() {
                    timeline.status = "RUNNING".into();
                }
            }
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
                self.step_stack.push(index - 1);
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
                if let Some(index) = self.step_stack.pop()
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
                self.thinking_first = Some(
                    self.thinking_first
                        .map_or(created, |value| value.min(created)),
                );
                self.thinking_last = Some(
                    self.thinking_last
                        .map_or(created, |value| value.max(created)),
                );
                timeline.events.push(TimelineEvent {
                    kind: "thinking".into(),
                    role: "assistant".into(),
                    started_at: iso(created),
                    ..Default::default()
                });
                self.current_thinking = Some(timeline.events.len() - 1);
            }
            "session.reasoning.delta" => {
                let delta = data.get("delta").and_then(Value::as_str).unwrap_or("");
                self.thinking_text.push_str(delta);
                if self.current_thinking.is_none() {
                    timeline.events.push(TimelineEvent {
                        kind: "thinking".into(),
                        role: "assistant".into(),
                        started_at: iso(created),
                        ..Default::default()
                    });
                    self.current_thinking = Some(timeline.events.len() - 1);
                }
                if let Some(index) = self.current_thinking
                    && let Some(event) = timeline.events.get_mut(index)
                {
                    event.text.push_str(delta);
                    event.ended_at = iso(created);
                    let start = event.started_at.as_deref().and_then(parse_iso_ms);
                    event.duration_ms = duration(start, Some(created));
                }
                self.thinking_first = Some(
                    self.thinking_first
                        .map_or(created, |value| value.min(created)),
                );
                self.thinking_last = Some(
                    self.thinking_last
                        .map_or(created, |value| value.max(created)),
                );
            }
            "session.reasoning.ended" => {
                if self.thinking_text.trim().is_empty()
                    && let Some(text) = data.get("text").and_then(Value::as_str)
                {
                    self.thinking_text.push_str(text);
                }
                if let Some(index) = self.current_thinking
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
                self.current_thinking = None;
                self.thinking_last = Some(
                    self.thinking_last
                        .map_or(created, |value| value.max(created)),
                );
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
                    self.tool_events.insert(id, timeline.events.len() - 1);
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
                    if !self.tool_events.contains_key(id) {
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
                        self.tool_events.insert(id_value, timeline.events.len() - 1);
                    }
                }
            }
            "session.tool.input.ended" => {
                let id = data.get("id").and_then(Value::as_str).unwrap_or_default();
                if let Some(index) = tool_index.get(id) {
                    if let Some(text) = data.get("text").and_then(Value::as_str) {
                        tools[*index].input = text.to_string();
                    }
                    if let Some(event_index) = self.tool_events.get(id).copied()
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
                    if let Some(event_index) = self.tool_events.get(id).copied()
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
                self.text_started = Some(created);
                timeline.events.push(TimelineEvent {
                    kind: "comment".into(),
                    role: "assistant".into(),
                    started_at: iso(created),
                    ..Default::default()
                });
                self.current_comment = Some(timeline.events.len() - 1);
            }
            "session.text.delta" => {
                let delta = data.get("delta").and_then(Value::as_str).unwrap_or("");
                if self.result_text.is_empty() {
                    self.result_text.push_str(delta);
                }
                if self.current_comment.is_none() {
                    timeline.events.push(TimelineEvent {
                        kind: "comment".into(),
                        role: "assistant".into(),
                        started_at: iso(created),
                        ..Default::default()
                    });
                    self.current_comment = Some(timeline.events.len() - 1);
                }
                if let Some(index) = self.current_comment
                    && let Some(event) = timeline.events.get_mut(index)
                {
                    event.text.push_str(delta);
                    event.ended_at = iso(created);
                    let start = event.started_at.as_deref().and_then(parse_iso_ms);
                    event.duration_ms = duration(start, Some(created));
                }
                self.result_first = Some(
                    self.result_first
                        .map_or(created, |value| value.min(created)),
                );
                self.result_last =
                    Some(self.result_last.map_or(created, |value| value.max(created)));
            }
            "session.text.ended" => {
                if let Some(text) = data.get("text").and_then(Value::as_str)
                    && !text.trim().is_empty()
                {
                    self.result_text = text.to_string();
                }
                self.result_first = Some(
                    self.result_first
                        .or(self.text_started)
                        .map_or(created, |value| value.min(created)),
                );
                if let Some(index) = self.current_comment
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
                self.current_comment = None;
                self.result_last =
                    Some(self.result_last.map_or(created, |value| value.max(created)));
            }
            "session.execution.succeeded" | "session.execution.failed" => {
                timeline.status = if kind == "session.execution.succeeded" {
                    "SUCCESS".into()
                } else {
                    "FAILED".into()
                };
                timeline.completed_at = iso(created);
            }
            _ => {
                // Keep every OpenCode event visible even when the UI does not
                // know its schema. Future engine events then show up without a
                // frontend release.
                let (data, _) = truncate(data.to_string(), MAX_TOOL_OUTPUT);
                timeline.events.push(TimelineEvent {
                    kind: "event".into(),
                    role: "system".into(),
                    text: kind.to_string(),
                    event_type: kind.to_string(),
                    data,
                    started_at: iso(created),
                    ended_at: iso(created),
                    ..Default::default()
                });
            }
        }

        let _ = reasoning_started;

        self.tools = tools;
        self.tool_index = tool_index;
    }
}

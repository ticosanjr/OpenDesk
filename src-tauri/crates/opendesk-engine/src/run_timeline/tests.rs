use super::*;
use serde_json::{Value, json};

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
        json!({"created": 2700, "type": "session.permission.requested", "data": {"action": "bash", "resource": "npm test"}}),
        json!({"created": 2800, "type": "session.execution.succeeded", "data": {}}),
    ]
}

#[test]
fn builds_thinking_executed_result_bands() {
    let timeline = timeline_from_events(None, events(), false);
    assert_eq!(timeline.status, "SUCCESS");
    assert_eq!(timeline.events_count, 14);
    assert_eq!(timeline.workspace_root.as_deref(), Some("/tmp/workspace"));
    assert_eq!(timeline.duration_ms, 1800);
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
    let generic = timeline
        .events
        .iter()
        .find(|event| event.kind == "event")
        .expect("generic event");
    assert_eq!(generic.event_type, "session.permission.requested");
    assert!(generic.data.contains("npm test"));
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
fn running_timeline_stays_open_until_terminal_event() {
    let timeline = timeline_from_events(
        None,
        vec![
            json!({"created": 1000, "type": "session.execution.started", "data": {}}),
            json!({"created": 1200, "type": "session.reasoning.delta", "data": {"delta": "pensando"}}),
        ],
        false,
    );
    assert_eq!(timeline.status, "RUNNING");
    assert!(timeline.completed_at.is_none());
    assert_eq!(timeline.thinking.text, "pensando");
    assert!(timeline.artifacts.is_empty());
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

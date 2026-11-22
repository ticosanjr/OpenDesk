use super::process::render_args;
use super::*;
use serde_json::json;

#[test]
fn renders_prompt_placeholder_or_appends_it() {
    let args = render_args(
        &["-p".into(), "{prompt}".into(), "{model}".into()],
        "hello",
        "build",
        Some("m1"),
        "/tmp/ws",
    );
    assert_eq!(args, vec!["-p", "hello", "m1"]);

    let args = render_args(
        &["exec".into(), "--json".into()],
        "hi",
        "build",
        None,
        "/ws",
    );
    assert_eq!(args, vec!["exec", "--json", "hi"]);
}

#[test]
fn normalizes_claude_stream_json() {
    let line = json!({
        "type": "assistant",
        "message": { "content": [
            { "type": "text", "text": "Vou editar." },
            { "type": "tool_use", "id": "t1", "name": "Edit", "input": { "file": "a.rs" } }
        ]}
    })
    .to_string();
    let events = normalize_line(&line);
    assert!(matches!(events[0], RuntimeEvent::TextDelta { .. }));
    assert!(matches!(events[1], RuntimeEvent::ToolCall { .. }));

    let result = json!({
        "type": "result",
        "is_error": false,
        "result": "ok",
        "total_cost_usd": 0.01,
        "usage": { "input_tokens": 10, "output_tokens": 5 }
    })
    .to_string();
    let events = normalize_line(&result);
    assert!(matches!(
        events[0],
        RuntimeEvent::Usage {
            input_tokens: 10,
            ..
        }
    ));
    assert!(matches!(
        events[1],
        RuntimeEvent::RunFinished { ref status, .. } if status == "success"
    ));
}

#[test]
fn normalizes_codex_and_plain_text() {
    let line = json!({
        "type": "item.completed",
        "item": { "id": "item_1", "type": "agent_message", "text": "feito" }
    })
    .to_string();
    assert!(matches!(
        normalize_line(&line)[0],
        RuntimeEvent::TextDelta { .. }
    ));

    let events = normalize_line("compilando...");
    assert!(matches!(events[0], RuntimeEvent::TextDelta { .. }));
}

fn spec(command: &str, args: &[&str]) -> HarnessSpec {
    HarnessSpec {
        id: "test-cli".into(),
        name: "Test CLI".into(),
        kind: "cli".into(),
        command: Some(command.into()),
        capabilities_json: "{}".into(),
        status: "READY".into(),
        args_json: serde_json::to_string(args).expect("args"),
        health_args_json: "[]".into(),
        last_health_at: None,
    }
}

#[tokio::test]
async fn streams_plain_stdout_and_finishes() {
    use futures_util::StreamExt;
    let dir = tempfile::tempdir().expect("tempdir");
    let adapter = CliAdapter::new(spec("/bin/echo", &["{prompt}"])).expect("adapter");
    let session = adapter
        .start(RuntimeRequest {
            workspace_root: dir.path().to_string_lossy().to_string(),
            agent: "build".into(),
            model: None,
            prompt: String::new(),
            env: Default::default(),
            sandbox: Default::default(),
        })
        .await
        .expect("start");
    adapter
        .prompt(&session, "olá harness")
        .await
        .expect("prompt");
    let mut stream = adapter.events(&session).await.expect("events");
    let mut text = String::new();
    let mut finished = false;
    while let Some(event) = stream.next().await {
        match event.expect("event") {
            RuntimeEvent::TextDelta { delta } => text.push_str(&delta),
            RuntimeEvent::RunFinished { status, .. } => {
                assert_eq!(status, "success");
                finished = true;
            }
            _ => {}
        }
    }
    assert!(text.contains("olá harness"), "text: {text}");
    assert!(finished);
}

#[test]
fn parses_porcelain_paths() {
    let rd = |line: &str| {
        let code = line[..2].trim();
        (code.to_string(), line[3..].trim().to_string())
    };
    assert_eq!(rd(" M src/a.rs"), ("M".into(), "src/a.rs".into()));
    assert_eq!(rd("?? novo.md"), ("??".into(), "novo.md".into()));
}

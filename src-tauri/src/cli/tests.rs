use super::args::{parse, shorten};
use super::node::node_harness_command;
use super::opencode_job::{extract_sse_events, opencode_session_matches};

#[test]
fn parses_flags_positionals_and_json() {
    let args = vec![
        "tasks".to_string(),
        "create".to_string(),
        "--title".to_string(),
        "Relatório".to_string(),
        "--description=Resumo".to_string(),
        "--json".to_string(),
    ];
    let parsed = parse(&args);
    assert_eq!(parsed.positionals, vec!["tasks", "create"]);
    assert_eq!(
        parsed.flags.get("title").map(String::as_str),
        Some("Relatório")
    );
    assert_eq!(
        parsed.flags.get("description").map(String::as_str),
        Some("Resumo")
    );
    assert!(parsed.json);
}

#[test]
fn parses_sse_and_matches_sessions() {
    let mut buffer = "data: {\"type\":\"session.text.delta\",\"data\":{\"sessionID\":\"s1\",\"delta\":\"oi\"}}\n\n".to_string();
    let events = extract_sse_events(&mut buffer);
    assert_eq!(events.len(), 1);
    assert!(opencode_session_matches(&events[0], "s1"));
    assert!(!opencode_session_matches(&events[0], "s2"));
    assert!(buffer.is_empty());
}

#[test]
fn maps_remote_harnesses_to_commands() {
    let (program, args) = node_harness_command("claude-code", "olá").expect("claude");
    assert_eq!(program, "claude");
    assert!(args.contains(&"olá".to_string()));
    let (program, args) = node_harness_command("codex", "olá").expect("codex");
    assert_eq!(program, "codex");
    assert_eq!(args[0], "exec");
    assert!(node_harness_command("opencode", "olá").is_none());
    assert!(node_harness_command("desconhecido", "olá").is_none());
}

#[test]
fn shortens_ids_safely() {
    assert_eq!(shorten("123456789"), "12345678");
    assert_eq!(shorten("abc"), "abc");
}

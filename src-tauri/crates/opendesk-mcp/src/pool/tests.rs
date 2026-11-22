#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fake_server() -> Option<(tempfile::TempDir, PathBuf)> {
        let dir = tempfile::TempDir::new().ok()?;
        let script = dir.path().join("fake_mcp.py");
        std::fs::write(
            &script,
            r#"import json, sys
for line in sys.stdin:
    req = json.loads(line)
    method = req.get("method")
    if method == "initialize":
        result = {"protocolVersion": "2024-11-05", "capabilities": {"tools": {}}, "serverInfo": {"name": "fake", "version": "1"}}
    elif method == "tools/list":
        result = {"tools": [{"name": "echo", "description": "echo", "inputSchema": {"type": "object"}}]}
    elif method == "tools/call":
        args = req.get("params", {}).get("arguments", {})
        result = {"content": [{"type": "text", "text": json.dumps(args)}], "isError": False}
    else:
        result = {}
    print(json.dumps({"jsonrpc": "2.0", "id": req.get("id"), "result": result}), flush=True)
"#,
        )
        .ok()?;
        Some((dir, script))
    }

    #[test]
    fn namespaces_and_splits_tools() {
        assert_eq!(namespaced("fs", "read"), "fs__read");
        assert_eq!(split_namespaced("fs__read"), Some(("fs", "read")));
        assert_eq!(split_namespaced("plain"), None);
        assert_eq!(split_namespaced("__bad"), None);
    }

    #[tokio::test]
    async fn connects_lists_and_calls_a_stdio_server() {
        let Some((dir, script)) = fake_server() else {
            return;
        };
        if std::process::Command::new("python3")
            .arg("--version")
            .output()
            .is_err()
        {
            return;
        }
        let config = ServerConfig {
            name: "fake".into(),
            transport: "local".into(),
            command: vec!["python3".into(), script.to_string_lossy().to_string()],
            url: None,
            headers: HashMap::new(),
            env: HashMap::new(),
            disabled: false,
        };
        let handle = ServerHandle::new(config);
        handle.ensure(dir.path()).await.expect("connect");
        let tools = handle.tools.lock().await.clone();
        assert_eq!(tools[0]["name"], "echo");
        let result = handle
            .call("echo", json!({ "text": "oi" }), dir.path())
            .await
            .expect("call");
        assert!(
            result["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("oi")
        );
    }

    #[tokio::test]
    async fn aggregates_two_servers_with_distinct_namespaces() {
        let Some((dir, script)) = fake_server() else {
            return;
        };
        if std::process::Command::new("python3")
            .arg("--version")
            .output()
            .is_err()
        {
            return;
        }
        std::fs::write(
            dir.path().join("opencode.jsonc"),
            format!(
                r#"{{"mcp":{{"servers":{{"one":{{"type":"local","command":["python3","{}"]}},"two":{{"type":"local","command":["python3","{}"]}}}}}}}}"#,
                script.display(),
                script.display()
            ),
        )
        .expect("config");
        let pool = McpPool::new();
        let tools = pool.tools("ws", dir.path()).await.expect("tools");
        let names: Vec<_> = tools
            .iter()
            .filter_map(|tool| tool["name"].as_str())
            .collect();
        assert!(names.contains(&"one__echo"), "{names:?}");
        assert!(names.contains(&"two__echo"), "{names:?}");
        let one = pool
            .call("ws", dir.path(), "one__echo", json!({ "side": "one" }))
            .await
            .expect("one");
        let two = pool
            .call("ws", dir.path(), "two__echo", json!({ "side": "two" }))
            .await
            .expect("two");
        assert!(one["content"][0]["text"].as_str().unwrap().contains("one"));
        assert!(two["content"][0]["text"].as_str().unwrap().contains("two"));
    }

    #[tokio::test]
    async fn loads_workspace_config_and_skips_disabled() {
        let dir = tempfile::TempDir::new().expect("temp");
        std::fs::write(
            dir.path().join("opencode.jsonc"),
            r#"{"mcp":{"servers":{"fake":{"type":"local","command":["python3","x.py"]},"off":{"type":"local","command":["python3","y.py"],"disabled":true}}}}"#,
        )
        .expect("config");
        let configs = load_configs(dir.path()).expect("configs");
        assert!(configs.iter().any(|config| config.name == "fake"));
        assert!(!configs.iter().any(|config| config.name == "off"));
    }
}

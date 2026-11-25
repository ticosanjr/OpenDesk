use super::*;

fn tool_body(slug: &str) -> Value {
    json!({
        "scope": "workspace",
        "slug": slug,
        "name": "Echo JSON",
        "description": "Devolve a entrada",
        "agent_prompt": "Use para testar a entrada.",
        "runtime": "bash",
        "script_body": "cat",
        "input_schema_json": "{\"type\":\"object\",\"properties\":{\"name\":{\"type\":\"string\"}},\"required\":[\"name\"]}",
        "env_secret_refs_json": "{}",
        "timeout_secs": 10,
        "require_approval": false,
        "enabled": true
    })
}

#[tokio::test]
async fn script_tools_crud_test_and_export_import() {
    let ctx = test_support::context().await;
    let workspace = format!("workspace_id={}", ctx.workspace_id);
    let (status, created) = call(
        &ctx,
        Method::POST,
        &format!("/api/script-tools?{workspace}"),
        Some(tool_body("echo_json")),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{created}");
    let id = created["id"].as_str().expect("id");
    assert_eq!(created["slug"], "echo_json");
    assert_eq!(created["workspace_id"], ctx.workspace_id);

    let (status, list) = call(
        &ctx,
        Method::GET,
        &format!("/api/script-tools?{workspace}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{list}");
    assert_eq!(list["tools"].as_array().map(Vec::len), Some(1));
    assert!(list["runtimes"].as_array().is_some());

    let (status, result) = call(
        &ctx,
        Method::POST,
        &format!("/api/script-tools/{id}/test"),
        Some(json!({ "input": { "name": "Ana" } })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert_eq!(result["ok"], true);
    assert_eq!(result["exit_code"], 0);
    assert!(result["stdout"].as_str().unwrap_or("").contains("Ana"));

    let (status, invalid) = call(
        &ctx,
        Method::POST,
        &format!("/api/script-tools?{workspace}"),
        Some(json!({
            "scope": "workspace",
            "slug": "Bad Slug",
            "name": "x",
            "runtime": "bash",
            "script_body": "cat"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{invalid}");

    let (status, invalid_runtime) = call(
        &ctx,
        Method::POST,
        &format!("/api/script-tools?{workspace}"),
        Some(json!({
            "scope": "workspace",
            "slug": "bad-runtime",
            "name": "x",
            "runtime": "ruby",
            "script_body": "puts 1"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{invalid_runtime}");

    let (status, exported) = call(
        &ctx,
        Method::GET,
        &format!("/api/script-tools/export?{workspace}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{exported}");
    assert_eq!(exported["kind"], "script_tools");
    assert_eq!(exported["items"]["tools"].as_array().map(Vec::len), Some(1));

    let (status, imported) = call(
        &ctx,
        Method::POST,
        "/api/script-tools/import",
        Some(json!({
            "workspace_id": ctx.workspace_id,
            "mode": "merge",
            "bundle": exported,
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{imported}");
    assert_eq!(imported["updated"], 1);

    let (status, rejected) = call(
        &ctx,
        Method::POST,
        "/api/script-tools/import",
        Some(json!({
            "workspace_id": ctx.workspace_id,
            "bundle": { "schema_version": 1, "kind": "packs_bundle", "items": { "tools": [] } },
        })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{rejected}");

    let (status, _) = call(
        &ctx,
        Method::DELETE,
        &format!("/api/script-tools/{id}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn script_tools_validate_schema_and_timeout() {
    let ctx = test_support::context().await;
    let workspace = format!("workspace_id={}", ctx.workspace_id);
    let mut body = tool_body("slow_echo");
    body["script_body"] = json!("sleep 2; cat");
    body["timeout_secs"] = json!(1);
    let (status, created) = call(
        &ctx,
        Method::POST,
        &format!("/api/script-tools?{workspace}"),
        Some(body),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{created}");
    let id = created["id"].as_str().expect("id");

    let (status, missing) = call(
        &ctx,
        Method::POST,
        &format!("/api/script-tools/{id}/test"),
        Some(json!({ "input": {} })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{missing}");

    let (status, timed_out) = call(
        &ctx,
        Method::POST,
        &format!("/api/script-tools/{id}/test"),
        Some(json!({ "input": { "name": "x" } })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{timed_out}");
    assert_eq!(timed_out["timed_out"], true);
    assert_eq!(timed_out["ok"], false);
}

#[tokio::test]
async fn mcp_scripts_lists_and_calls_enabled_tools() {
    let ctx = test_support::context().await;
    let tool = crate::db::script_tools::create(
        &ctx.state.db,
        crate::db::script_tools::CreateScriptToolInput {
            workspace_id: Some(ctx.workspace_id.clone()),
            slug: "mcp_echo".into(),
            name: "MCP echo".into(),
            description: "Echo".into(),
            agent_prompt: "Use quando precisar ecoar.".into(),
            runtime: "bash".into(),
            custom_command: None,
            script_body: "cat".into(),
            input_schema_json: r#"{"type":"object","properties":{},"required":[]}"#.into(),
            env_secret_refs_json: "{}".into(),
            timeout_secs: 10,
            require_approval: false,
            enabled: true,
        },
    )
    .await
    .expect("tool");
    let tools = crate::db::script_tools::list_enabled(&ctx.state.db, Some(&ctx.workspace_id))
        .await
        .expect("list");
    let (definitions, by_name) = crate::mcp_scripts::definitions(tools).expect("definitions");
    assert_eq!(definitions.len(), 1);
    assert_eq!(definitions[0]["name"], "script_mcp_echo");
    assert!(
        definitions[0]["description"]
            .as_str()
            .unwrap_or("")
            .contains("Echo")
    );
    let definition_tool = by_name.get("script_mcp_echo").expect("tool by name");
    let output = crate::mcp_scripts::call_tool(
        &ctx.state.db,
        &ctx.state.paths,
        &ctx.workspace_id,
        definition_tool,
        &json!({ "hello": true }),
    )
    .await
    .expect("call");
    let parsed: Value = serde_json::from_str(&output).expect("json");
    assert_eq!(parsed["ok"], true);
    assert!(parsed["stdout"].as_str().unwrap_or("").contains("hello"));
    let _ = tool;
}

#[tokio::test]
async fn script_tools_reject_plaintext_env_secrets() {
    let ctx = test_support::context().await;
    let workspace = format!("workspace_id={}", ctx.workspace_id);
    let mut body = tool_body("env_tool");
    body["env_secret_refs_json"] = json!(r#"{"API_KEY":"minha-chave"}"#);
    let (status, rejected) = call(
        &ctx,
        Method::POST,
        &format!("/api/script-tools?{workspace}"),
        Some(body),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{rejected}");
    assert!(
        rejected["message"]
            .as_str()
            .unwrap_or("")
            .contains("secret_ref")
    );
}

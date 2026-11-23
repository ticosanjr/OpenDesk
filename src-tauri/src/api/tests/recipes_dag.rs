use super::*;

const YAML: &str = r#"
id: dag-validacao
name: DAG validação
enabled: false
triggers: []
flow:
  - type: step
    id: coletar
    name: Coletar
    mode: build
    prompt: "Liste itens em JSON"
    output_schema:
      type: object
      required: [items]
      properties:
        items: { type: array }
    retry_max: 2
    timeout_secs: 60
    on_schema_error: hitl
  - type: branch
    id: decidir
    when: "{{steps.coletar.structured.items}} != []"
    then:
      - type: map
        id: processar
        items: "{{steps.coletar.structured.items}}"
        flow:
          - type: step
            id: resumir
            name: Resumir
            mode: build
            prompt: "Resuma o item"
    else: []
"#;

#[tokio::test]
async fn validates_dag_and_reports_missing_vault_refs() {
    let ctx = test_support::context().await;
    let query = format!("?workspace_id={}", ctx.workspace_id);
    let (status, body) = call(
        &ctx,
        Method::POST,
        &format!("/api/recipes/validate{query}"),
        Some(json!({ "yaml": YAML })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["valid"], true, "{body}");
    assert_eq!(body["recipe"]["flow"][1]["type"], "branch");

    let with_vault = YAML.replace(
        "    on_schema_error: hitl",
        "    on_schema_error: hitl\n    vault_refs: [credencial-inexistente]",
    );
    let (status, body) = call(
        &ctx,
        Method::POST,
        &format!("/api/recipes/validate{query}"),
        Some(json!({ "yaml": with_vault })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["valid"], false);
    assert!(
        body["issues"][0]["message"]
            .as_str()
            .unwrap_or_default()
            .contains("credencial-inexistente")
    );
}

#[tokio::test]
async fn resume_and_node_retry_reject_unknown_runs() {
    let ctx = test_support::context().await;
    let (status, _) = call(
        &ctx,
        Method::POST,
        "/api/recipes/runs/run-inexistente/resume",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, _) = call(
        &ctx,
        Method::POST,
        "/api/recipes/runs/run-inexistente/nodes/coletar",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn completed_dry_run_rejects_resume_and_node_retry() {
    let ctx = test_support::context().await;
    let query = format!("?workspace_id={}", ctx.workspace_id);
    let (status, saved) = call(
        &ctx,
        Method::PUT,
        &format!("/api/recipes/dag-validacao{query}"),
        Some(json!({ "yaml": YAML })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    let (status, _) = call(
        &ctx,
        Method::POST,
        &format!("/api/recipes/dag-validacao/enabled{query}"),
        Some(json!({ "enabled": true })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, run) = call(
        &ctx,
        Method::POST,
        &format!("/api/recipes/dag-validacao/run{query}"),
        Some(json!({ "dry_run": true })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{run}");
    let run_id = run["id"].as_str().expect("run id").to_string();
    let mut completed = false;
    for _ in 0..30 {
        let (status, detail) = call(
            &ctx,
            Method::GET,
            &format!("/api/recipes/runs/{run_id}"),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        if detail["run"]["status"] == "COMPLETED" {
            completed = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    assert!(completed, "dry run não concluiu");

    let (status, _) = call(
        &ctx,
        Method::POST,
        &format!("/api/recipes/runs/{run_id}/resume"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);

    let (status, _) = call(
        &ctx,
        Method::POST,
        &format!("/api/recipes/runs/{run_id}/nodes/coletar"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn dry_run_rejects_missing_vault_refs() {
    let ctx = test_support::context().await;
    let query = format!("?workspace_id={}", ctx.workspace_id);
    let yaml = YAML.replace(
        "    on_schema_error: hitl",
        "    on_schema_error: hitl\n    vault_refs: [credencial-inexistente]",
    );
    let (status, saved) = call(
        &ctx,
        Method::PUT,
        &format!("/api/recipes/dag-validacao{query}"),
        Some(json!({ "yaml": yaml })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    let (status, _) = call(
        &ctx,
        Method::POST,
        &format!("/api/recipes/dag-validacao/enabled{query}"),
        Some(json!({ "enabled": true })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, error) = call(
        &ctx,
        Method::POST,
        &format!("/api/recipes/dag-validacao/run{query}"),
        Some(json!({ "dry_run": true })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{error}");
    assert!(error.to_string().contains("credencial-inexistente"));
}

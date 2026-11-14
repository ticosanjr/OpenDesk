use super::*;

#[tokio::test]
async fn hitl_v2_supports_approval_choices_and_text() {
    let ctx = test_support::context().await;
    let (status, task) = call(
        &ctx,
        Method::POST,
        &format!("/api/tasks?workspace_id={}", ctx.workspace_id),
        Some(json!({
            "title": "HITL 2.0",
            "description": "Teste das estruturas unificadas."
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{task}");
    let task_id = task["id"].as_str().expect("task id");

    // APPROVAL with an optional payload.
    let (status, approval) = call(
        &ctx,
        Method::POST,
        &format!("/api/tasks/{task_id}/interactions"),
        Some(json!({
            "kind": "APPROVAL",
            "question": "Posso publicar?",
            "context": "A publicação é externa.",
            "allow_custom": true
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{approval}");
    assert_eq!(approval["kind"], "APPROVAL");
    assert_eq!(approval["multiple"], false);
    let approval_id = approval["id"].as_str().expect("approval id");

    // Approval without a decision is rejected by the API.
    let (status, error) = call(
        &ctx,
        Method::POST,
        &format!("/api/interactions/{approval_id}/resolve"),
        Some(json!({ "text": "sem decisão" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{error}");

    let (status, resolved) = call(
        &ctx,
        Method::POST,
        &format!("/api/interactions/{approval_id}/resolve"),
        Some(json!({ "decision": "deny", "text": "faltou citar a fonte" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{resolved}");
    let answer = resolved["answer_json"].as_str().expect("answer");
    assert!(answer.contains("\"decision\":\"deny\""));
    assert!(answer.contains("faltou citar a fonte"));

    // SINGLE_CHOICE requires options and resolves with one selection.
    let (status, error) = call(
        &ctx,
        Method::POST,
        &format!("/api/tasks/{task_id}/interactions"),
        Some(json!({ "kind": "SINGLE_CHOICE", "question": "Qual tom?" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{error}");

    let (status, choice) = call(
        &ctx,
        Method::POST,
        &format!("/api/tasks/{task_id}/interactions"),
        Some(json!({
            "kind": "SINGLE_CHOICE",
            "question": "Qual tom?",
            "options": ["formal", "direto"]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{choice}");
    let choice_id = choice["id"].as_str().expect("choice id");
    let (status, resolved) = call(
        &ctx,
        Method::POST,
        &format!("/api/interactions/{choice_id}/resolve"),
        Some(json!({ "selected": ["direto"] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{resolved}");
    assert!(
        resolved["answer_json"]
            .as_str()
            .unwrap_or_default()
            .contains("direto")
    );

    // TEXT needs a payload.
    let (status, text) = call(
        &ctx,
        Method::POST,
        &format!("/api/tasks/{task_id}/interactions"),
        Some(json!({ "kind": "TEXT", "question": "Qual o prazo?" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{text}");
    let text_id = text["id"].as_str().expect("text id");
    let (status, error) = call(
        &ctx,
        Method::POST,
        &format!("/api/interactions/{text_id}/resolve"),
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{error}");
    let (status, resolved) = call(
        &ctx,
        Method::POST,
        &format!("/api/interactions/{text_id}/resolve"),
        Some(json!({ "text": "sexta-feira" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{resolved}");
    assert!(
        resolved["answer_json"]
            .as_str()
            .unwrap_or_default()
            .contains("sexta-feira")
    );

    // Legacy callers still derive the kind from options.
    let (status, legacy) = call(
        &ctx,
        Method::POST,
        &format!("/api/tasks/{task_id}/interactions"),
        Some(json!({
            "question": "Escolha uma",
            "options": ["A", "B"]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{legacy}");
    assert_eq!(legacy["kind"], "SINGLE_CHOICE");
}

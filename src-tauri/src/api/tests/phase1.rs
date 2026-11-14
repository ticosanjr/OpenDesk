use super::*;

#[tokio::test]
async fn integrations_overview_reports_known_channels() {
    let ctx = test_support::context().await;
    let (status, value) = call(
        &ctx,
        Method::GET,
        &format!(
            "/api/integrations/overview?workspace_id={}",
            ctx.workspace_id
        ),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{value}");
    let items = value.as_array().expect("items");
    for id in ["smtp", "google", "drive", "calendar", "signal", "telegram"] {
        assert!(items.iter().any(|item| item["id"] == id), "missing {id}");
    }
}

#[tokio::test]
async fn recent_search_returns_latest_activity_without_a_query() {
    let ctx = test_support::context().await;
    let (status, task) = call(
        &ctx,
        Method::POST,
        &format!("/api/tasks?workspace_id={}", ctx.workspace_id),
        Some(json!({
            "title": "Atividade recente",
            "description": "Criada para o teste de recentes."
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{task}");

    let (status, hits) = call(
        &ctx,
        Method::GET,
        &format!(
            "/api/search/recent?workspace_id={}&limit=20",
            ctx.workspace_id
        ),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{hits}");
    assert!(hits.as_array().is_some_and(|items| {
        items
            .iter()
            .any(|item| item["kind"] == "task" && item["title"] == "Atividade recente")
    }));
}

#[tokio::test]
async fn deleting_a_recipe_removes_it_from_the_list() {
    let ctx = test_support::context().await;
    let yaml = "id: apagar\nname: Apagar\nenabled: false\ntriggers: []\nflow:\n  - type: step\n    id: s1\n    name: Passo\n    mode: build\n    agent: build\n    prompt: oi\n";
    let (status, _) = call(
        &ctx,
        Method::PUT,
        &format!("/api/recipes/apagar?workspace_id={}", ctx.workspace_id),
        Some(json!({ "yaml": yaml })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = call(
        &ctx,
        Method::DELETE,
        &format!("/api/recipes/apagar?workspace_id={}", ctx.workspace_id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, listed) = call(
        &ctx,
        Method::GET,
        &format!("/api/recipes?workspace_id={}", ctx.workspace_id),
        None,
    )
    .await;
    assert!(
        !listed["recipes"]
            .as_array()
            .is_some_and(|items| items.iter().any(|item| item["id"] == "apagar"))
    );
}

use super::*;

#[tokio::test]
async fn saves_a_workspace_file_to_a_chosen_destination() {
    let ctx = test_support::context().await;
    let source = ctx.workspace_root.join("relatorio.md");
    std::fs::write(&source, b"conteudo seguro").expect("source");
    let destination = ctx.temp.path().join("download-escolhido.md");

    let (status, body) = call(
        &ctx,
        Method::POST,
        "/api/files/save",
        Some(json!({
            "workspace_id": ctx.workspace_id,
            "path": "relatorio.md",
            "destination": destination.to_string_lossy(),
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        std::fs::read_to_string(&destination).expect("destination"),
        "conteudo seguro"
    );

    let (status, _) = call(
        &ctx,
        Method::POST,
        "/api/files/save",
        Some(json!({
            "workspace_id": ctx.workspace_id,
            "path": "../outside.md",
            "destination": destination.to_string_lossy(),
        })),
    )
    .await;
    assert!(status.is_client_error());

    let (status, _) = call(
        &ctx,
        Method::POST,
        "/api/files/save",
        Some(json!({
            "workspace_id": ctx.workspace_id,
            "path": "relatorio.md",
            "destination": "relative.md",
        })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn saves_exported_bytes_to_an_approved_destination() {
    use base64::{Engine as _, engine::general_purpose::STANDARD};

    let ctx = test_support::context().await;
    let destination = ctx.temp.path().join("exported.json");
    let encoded = STANDARD.encode(b"{\"ok\":true}");
    let (status, body) = call(
        &ctx,
        Method::POST,
        "/api/files/save-bytes",
        Some(json!({
            "destination": destination.to_string_lossy(),
            "content_base64": encoded,
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        std::fs::read_to_string(&destination).expect("exported"),
        "{\"ok\":true}"
    );

    let (status, _) = call(
        &ctx,
        Method::POST,
        "/api/files/save-bytes",
        Some(json!({
            "destination": "relative.json",
            "content_base64": STANDARD.encode(b"x"),
        })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

use super::*;
use crate::db::workspaces;

#[tokio::test]
async fn chat_can_send_a_workspace_file_by_path() {
    let ctx = test_support::context().await;
    let workspace = workspaces::get(&ctx.state.db, &ctx.workspace_id)
        .await
        .expect("workspace");
    let root = std::path::PathBuf::from(&workspace.root_path);
    std::fs::create_dir_all(root.join("output")).expect("output");
    std::fs::write(root.join("output/relatorio.txt"), "conteudo").expect("file");

    let (status, thread) = call(
        &ctx,
        Method::POST,
        "/api/chat/threads",
        Some(json!({
            "workspace_id": ctx.workspace_id,
            "title": "Teste de arquivo",
            "agent": "build",
            "source": "desktop"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{thread}");
    let thread_id = thread["id"].as_str().expect("thread id");

    let (status, sent) = call(
        &ctx,
        Method::POST,
        &format!("/api/chat/threads/{thread_id}/files"),
        Some(json!({
            "path": "output/relatorio.txt",
            "caption": "Segue o relatório"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{sent}");
    assert_eq!(sent["sent"], false);
    assert_eq!(sent["path"], "output/relatorio.txt");

    let (status, error) = call(
        &ctx,
        Method::POST,
        &format!("/api/chat/threads/{thread_id}/files"),
        Some(json!({ "path": "output/nao-existe.txt" })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{error}");

    let outside = tempfile::NamedTempFile::new().expect("outside");
    let (status, error) = call(
        &ctx,
        Method::POST,
        &format!("/api/chat/threads/{thread_id}/files"),
        Some(json!({ "path": outside.path().to_string_lossy() })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{error}");
}

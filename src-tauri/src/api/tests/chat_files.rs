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

#[tokio::test]
async fn email_reply_requires_a_thread_id_and_smtp() {
    let ctx = test_support::context().await;
    let thread = crate::db::chat::create_thread(
        &ctx.state.db,
        crate::db::chat::CreateThreadInput {
            workspace_id: ctx.workspace_id.clone(),
            title: Some("Email thread".into()),
            agent: "build".into(),
            model: None,
            source: Some("email".into()),
            external_id: Some("<root@example.com>".into()),
        },
    )
    .await
    .expect("thread");
    let (status, body) = call(
        &ctx,
        Method::POST,
        &format!("/api/chat/threads/{}/email-reply", thread.id),
        Some(json!({
            "to": "user@example.com",
            "subject": "Re: Email thread",
            "body_html": "<p>ok</p>"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(
        body["message"]
            .as_str()
            .unwrap_or_default()
            .contains("SMTP"),
        "{body}"
    );
}

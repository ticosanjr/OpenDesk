use super::export::sanitize;
use super::*;
use crate::db::memory;
use crate::test_support;
use serde_json::json;

#[test]
fn sanitize_removes_secret_fields_at_any_depth() {
    let mut value = json!({
        "token": "abc",
        "nested": { "api_key": "key", "password": "pw", "keep": "ok" },
        "array": [{ "secret": "s", "name": "visível" }]
    });
    sanitize(&mut value);
    assert!(value.get("token").is_none());
    assert_eq!(value["nested"]["keep"], "ok");
    assert!(value["nested"].get("api_key").is_none());
    assert_eq!(value["array"][0]["name"], "visível");
    assert!(value["array"][0].get("secret").is_none());
}

#[tokio::test]
async fn exports_previews_and_imports_memories_without_duplicates() {
    let ctx = test_support::context().await;
    memory::ensure_namespace(&ctx.state.db, &ctx.workspace_id, "core", None)
        .await
        .expect("namespace");
    memory::create_note(
        &ctx.state.db,
        &ctx.workspace_id,
        memory::CreateNoteInput {
            namespace: "core".into(),
            kind: "semantic".into(),
            title: "Preferência".into(),
            body: "usar pt-BR".into(),
            tags: vec!["idioma".into()],
            confidence: 0.9,
            trust_level: "observed".into(),
            expires_at: None,
            source: Some("test".into()),
        },
    )
    .await
    .expect("note");

    let bundle = export(&ctx.state.db, &ctx.workspace_id, &ExportRequest::default())
        .await
        .expect("export");
    assert_eq!(bundle.schema_version, SCHEMA_VERSION);
    let notes = bundle.sections.structured_memory.as_ref().expect("memory")["notes"]
        .as_array()
        .expect("notes");
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0]["title"], "Preferência");

    let preview = preview(&ctx.state.db, &ctx.workspace_id, &bundle, None)
        .await
        .expect("preview");
    assert_eq!(preview.sections["structured_memory"].incoming, 1);

    let report = import(
        &ctx.state.db,
        &ctx.workspace_id,
        &ImportRequest {
            workspace_id: None,
            bundle: bundle.clone(),
            sections: None,
            mode: ImportMode::MergeSkipExisting,
        },
    )
    .await
    .expect("skip import");
    assert_eq!(report.sections["structured_memory"].incoming, 1);
    let after = memory::list_notes(&ctx.state.db, &ctx.workspace_id, None, None, 100)
        .await
        .expect("notes");
    assert_eq!(after.len(), 1);

    let mut changed = bundle;
    if let Some(notes) = changed.sections.structured_memory.as_mut()
        && let Some(note) = notes["notes"].get_mut(0)
    {
        note["body"] = json!("usar en-US");
    }
    import(
        &ctx.state.db,
        &ctx.workspace_id,
        &ImportRequest {
            workspace_id: None,
            bundle: changed,
            sections: None,
            mode: ImportMode::MergeOverwrite,
        },
    )
    .await
    .expect("overwrite import");
    let after = memory::list_notes(&ctx.state.db, &ctx.workspace_id, None, None, 100)
        .await
        .expect("notes");
    assert_eq!(after.len(), 1);
    assert_eq!(after[0].body, "usar en-US");
}

#[tokio::test]
async fn rejects_unknown_schema_version() {
    let ctx = test_support::context().await;
    let mut bundle = export(&ctx.state.db, &ctx.workspace_id, &ExportRequest::default())
        .await
        .expect("export");
    bundle.schema_version = 99;
    let error = preview(&ctx.state.db, &ctx.workspace_id, &bundle, None)
        .await
        .expect_err("schema");
    assert!(error.to_string().contains("schema_version"));
}

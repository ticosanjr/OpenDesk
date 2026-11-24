use super::*;
use crate::test_db_support as test_support;

#[tokio::test]
async fn creates_lists_restores_and_deletes_a_workspace_backup() {
    let ctx = test_support::context().await;
    std::fs::create_dir_all(ctx.workspace_root.join(".opencode/agents")).expect("agents");
    std::fs::create_dir_all(ctx.workspace_root.join("input")).expect("input dir");
    std::fs::write(
        ctx.workspace_root.join(".opencode/agents/build.md"),
        "---\ndescription: build\n---\nbody\n",
    )
    .expect("agent");
    std::fs::write(ctx.workspace_root.join("input/brief.md"), "brief").expect("input");
    std::fs::create_dir_all(ctx.workspace_root.join("node_modules/pkg")).expect("nm");
    std::fs::write(ctx.workspace_root.join("node_modules/pkg/x.js"), "x").expect("nm file");

    sqlx::query(
        "INSERT INTO tasks (id, workspace_id, title, description)
         VALUES ('backup-task', ?, 'Tarefa do backup', 'descrição')",
    )
    .bind(&ctx.workspace_id)
    .execute(&ctx.db)
    .await
    .expect("task");

    let backup = create(&ctx.db, &ctx.paths, &ctx.workspace_id)
        .await
        .expect("backup");
    assert_eq!(backup.version, 2);
    assert!(backup.includes.sqlite_state);
    assert_eq!(backup.counts.get("tasks"), Some(&1));
    assert_eq!(backup.files, 2, "node_modules must be excluded");
    assert!(backup.size_bytes > 0);
    assert_eq!(list(&ctx.paths, &ctx.workspace_id).expect("list").len(), 1);

    let target = ctx.temp.path().join("restored-workspace");
    let restored = restore_as_new(
        &ctx.db,
        &ctx.paths,
        &ctx.workspace_id,
        &backup.id,
        RestoreInput {
            target_path: target.to_string_lossy().to_string(),
            name: Some("Restaurado".into()),
            ..Default::default()
        },
    )
    .await
    .expect("restore");
    assert_eq!(restored.name, "Restaurado");
    let restored_tasks: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM tasks WHERE workspace_id = ? AND title = 'Tarefa do backup'",
    )
    .bind(&restored.id)
    .fetch_one(&ctx.db)
    .await
    .expect("restored tasks");
    assert_eq!(restored_tasks, 1);
    assert!(target.join("input/brief.md").is_file());
    assert!(target.join(".opencode/agents/build.md").is_file());
    assert!(!target.join("node_modules").exists());

    delete(&ctx.paths, &ctx.workspace_id, &backup.id).expect("delete");
    assert!(
        list(&ctx.paths, &ctx.workspace_id)
            .expect("list")
            .is_empty()
    );
}

#[tokio::test]
async fn refuses_non_empty_target_and_corrupted_archive() {
    let ctx = test_support::context().await;
    std::fs::create_dir_all(ctx.workspace_root.join("input")).expect("input dir");
    std::fs::write(ctx.workspace_root.join("input/brief.md"), "brief").expect("input");
    let backup = create(&ctx.db, &ctx.paths, &ctx.workspace_id)
        .await
        .expect("backup");
    let target = ctx.temp.path().join("occupied");
    std::fs::create_dir_all(&target).expect("target");
    std::fs::write(target.join("keep.txt"), "keep").expect("keep");
    assert!(
        restore_as_new(
            &ctx.db,
            &ctx.paths,
            &ctx.workspace_id,
            &backup.id,
            RestoreInput {
                target_path: target.to_string_lossy().to_string(),
                name: None,
                ..Default::default()
            },
        )
        .await
        .is_err()
    );
    let (path, _) = get(&ctx.paths, &ctx.workspace_id, &backup.id).expect("path");
    std::fs::write(&path, "not a zip").expect("corrupt");
    assert!(
        restore_as_new(
            &ctx.db,
            &ctx.paths,
            &ctx.workspace_id,
            &backup.id,
            RestoreInput {
                target_path: ctx
                    .temp
                    .path()
                    .join("corrupt-target")
                    .to_string_lossy()
                    .to_string(),
                name: None,
                ..Default::default()
            },
        )
        .await
        .is_err()
    );
}

#[test]
fn portable_secrets_roundtrip_and_reject_wrong_password() {
    use std::collections::BTreeMap;
    let portable = super::portable::PortableSecrets {
        secret_store: BTreeMap::from([("smtp_ws_1".into(), "smtp-pass".into())]),
        vault: BTreeMap::from([("vault-1".into(), "vault-pass".into())]),
    };
    let encrypted = super::portable::encrypt(&portable, "correct horse").expect("encrypt");
    let decoded = super::portable::decrypt(&encrypted, "correct horse").expect("decrypt");
    assert_eq!(decoded.secret_store["smtp_ws_1"], "smtp-pass");
    assert_eq!(decoded.vault["vault-1"], "vault-pass");
    assert!(super::portable::decrypt(&encrypted, "wrong").is_err());
}

#[tokio::test]
async fn restores_in_place_and_keeps_a_pre_restore_backup() {
    let ctx = test_support::context().await;
    std::fs::create_dir_all(ctx.workspace_root.join("input")).expect("input");
    std::fs::write(ctx.workspace_root.join("input/brief.md"), "before").expect("before");
    sqlx::query(
        "INSERT INTO tasks (id, workspace_id, title, description)
         VALUES ('in-place-task', ?, 'Original', 'd')",
    )
    .bind(&ctx.workspace_id)
    .execute(&ctx.db)
    .await
    .expect("task");
    let backup = create(&ctx.db, &ctx.paths, &ctx.workspace_id)
        .await
        .expect("backup");
    sqlx::query("UPDATE tasks SET title = 'Mutated' WHERE id = 'in-place-task'")
        .execute(&ctx.db)
        .await
        .expect("mutate");
    std::fs::write(ctx.workspace_root.join("input/brief.md"), "after").expect("after");

    let (workspace, report) = restore(
        &ctx.db,
        &ctx.paths,
        &ctx.workspace_id,
        &backup.id,
        RestoreInput {
            mode: RestoreMode::InPlace,
            ..Default::default()
        },
    )
    .await
    .expect("in-place restore");
    assert_eq!(workspace.id, ctx.workspace_id);
    assert!(report.pre_restore_backup.is_some());
    let title: String = sqlx::query_scalar("SELECT title FROM tasks WHERE id = 'in-place-task'")
        .fetch_one(&ctx.db)
        .await
        .expect("title");
    assert_eq!(title, "Original");
    let content =
        std::fs::read_to_string(ctx.workspace_root.join("input/brief.md")).expect("content");
    assert_eq!(content, "before");
}

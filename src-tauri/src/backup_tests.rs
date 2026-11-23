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

    let backup = create(&ctx.db, &ctx.paths, &ctx.workspace_id)
        .await
        .expect("backup");
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
        },
    )
    .await
    .expect("restore");
    assert_eq!(restored.name, "Restaurado");
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
            },
        )
        .await
        .is_err()
    );
}

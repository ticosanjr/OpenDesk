use super::*;

#[test]
fn bundle_manifests_and_recipes_are_valid() {
    let issues = validate_bundle().expect("bundle must parse");
    assert!(issues.is_empty(), "issues: {issues:?}");
    let packs = catalog().expect("catalog").packs;
    assert!(
        packs.len() >= 20,
        "expected a broad catalog, got {}",
        packs.len()
    );
}

#[test]
fn every_bundled_recipe_is_disabled() {
    for id in catalog().expect("catalog").packs {
        let (manifest, _) = load_manifest(&id).expect("manifest");
        for name in manifest.contents.recipes {
            let text = BUNDLE
                .get_file(format!("packs/{id}/recipes/{name}"))
                .and_then(|file| std::str::from_utf8(file.contents()).ok())
                .unwrap_or_else(|| panic!("{id}: {name} ausente ou inválida"));
            assert!(
                text.contains("enabled: false"),
                "{id}: receita {name} deve nascer desligada"
            );
        }
    }
}

#[tokio::test]
async fn installs_the_operational_base_pack() {
    let (_temp, state, workspace_id) = test_state().await;
    let result = install_base_pack(&state, &workspace_id)
        .await
        .expect("base pack");
    assert_eq!(result.pack_id, BASE_PACK_ID);
    assert!(result.written > 0);
    assert!(
        std::path::Path::new(
            &crate::db::workspaces::get(&state.db, &workspace_id)
                .await
                .expect("workspace")
                .root_path
        )
        .join(".opencode/skills/operacao-opendesk/SKILL.md")
        .is_file()
    );
    for command in [
        "opendesk-status.md",
        "opendesk-tarefa.md",
        "opendesk-rotina.md",
        "opendesk-receita.md",
        "opendesk-executar.md",
    ] {
        assert!(
            std::path::Path::new(
                &crate::db::workspaces::get(&state.db, &workspace_id)
                    .await
                    .expect("workspace")
                    .root_path
            )
            .join(".opencode/commands")
            .join(command)
            .is_file(),
            "{command}"
        );
    }
    let routine_count = schedules::list(&state.db, &workspace_id)
        .await
        .expect("schedules")
        .len();
    assert_eq!(routine_count, 0, "base pack has no routines");
}

#[test]
fn every_skill_has_a_skill_md() {
    for id in catalog().expect("catalog").packs {
        let (manifest, dir) = load_manifest(&id).expect("manifest");
        for skill in manifest.contents.skills {
            let _ = dir;
            assert!(
                BUNDLE
                    .get_file(format!("packs/{id}/skills/{skill}/SKILL.md"))
                    .is_some(),
                "{id}: {skill} sem SKILL.md"
            );
        }
    }
}

async fn test_state() -> (tempfile::TempDir, Arc<AppState>, String) {
    use crate::paths::AppPaths;
    let temp = tempfile::TempDir::new().expect("temp");
    let root = temp.path();
    let paths = AppPaths {
        data_dir: root.join("data"),
        config_dir: root.join("config"),
        cache_dir: root.join("cache"),
        log_dir: root.join("logs"),
        db_path: root.join("data").join("opendesk.db"),
        backups_dir: root.join("backups"),
        artifacts_dir: root.join("artifacts"),
        run_logs_dir: root.join("runs"),
        daemon_lock_path: root.join("daemon.json"),
        api_lock_path: root.join("api.json"),
    };
    for dir in [
        &paths.data_dir,
        &paths.config_dir,
        &paths.cache_dir,
        &paths.log_dir,
        &paths.backups_dir,
        &paths.artifacts_dir,
        &paths.run_logs_dir,
    ] {
        std::fs::create_dir_all(dir).expect("dir");
    }
    let workspace_root = root.join("workspace");
    std::fs::create_dir_all(&workspace_root).expect("workspace root");
    let state = Arc::new(
        crate::app_state::AppState::initialize(paths)
            .await
            .expect("state"),
    );
    sqlx::query("INSERT INTO workspaces (id, name, root_path) VALUES ('ws-pack', 'Pack', ?)")
        .bind(workspace_root.to_string_lossy().to_string())
        .execute(&state.db)
        .await
        .expect("workspace");
    (temp, state, "ws-pack".to_string())
}

#[tokio::test]
async fn creates_the_default_factory_workspace_once() {
    let (temp, state, _) = test_state().await;
    crate::db::settings::set(
        &state.db,
        "workspaces_root",
        &serde_json::json!(temp.path().join("areas")),
    )
    .await
    .expect("root setting");
    let created = ensure_default_factory_workspace(&state)
        .await
        .expect("factory")
        .expect("created");
    assert_eq!(created.name, "Fábrica de Capacidades");
    let root = PathBuf::from(&created.root_path);
    assert!(
        root.join(".opencode/agents/arquiteto-capacidades.md")
            .is_file()
    );
    assert!(root.join("capabilities/evals").is_dir());
    assert!(
        ensure_default_factory_workspace(&state)
            .await
            .expect("second")
            .is_none()
    );
    assert!(
        workspaces::list(&state.db)
            .await
            .expect("workspaces")
            .iter()
            .filter(|workspace| workspace.name == "Fábrica de Capacidades")
            .count()
            == 1
    );
}

#[tokio::test]
async fn installs_validates_and_uninstalls_a_pack() {
    let (_temp, state, workspace_id) = test_state().await;
    let workspace = workspaces::get(&state.db, &workspace_id).await.expect("ws");
    let root = PathBuf::from(&workspace.root_path);

    let result = install(&state, "essenciais", &workspace_id, &[], false)
        .await
        .expect("install");
    assert!(result.written >= 8, "written: {result:?}");
    assert!(root.join(".opencode/agents/assistente.md").is_file());
    assert!(
        root.join(".opencode/skills/pesquisa-web/SKILL.md")
            .is_file()
    );
    assert!(
        root.join(".opencode/recipes/briefing-diario.yaml")
            .is_file()
    );
    crate::recipes::store::get(&state.db, &workspace_id, "briefing-diario")
        .await
        .expect("recipe indexed");

    let again = install(&state, "essenciais", &workspace_id, &[], false)
        .await
        .expect("reinstall");
    assert_eq!(again.written, 0);
    assert!(again.skipped >= 8);

    let agent_path = root.join(".opencode/agents/assistente.md");
    std::fs::write(&agent_path, "editado pelo usuário").expect("edit");

    let removed = uninstall(&state, "essenciais", &workspace_id)
        .await
        .expect("uninstall");
    assert!(
        removed
            .kept
            .iter()
            .any(|path| path.ends_with("assistente.md"))
    );
    assert!(agent_path.exists(), "modified file must be preserved");
    assert!(!root.join(".opencode/agents/pesquisador.md").exists());
}

#[tokio::test]
async fn reinstalling_a_pack_does_not_duplicate_routines() {
    let (_temp, state, workspace_id) = test_state().await;
    let first = install(&state, "wiki-segundo-cerebro", &workspace_id, &[], false)
        .await
        .expect("first install");
    let second = install(&state, "wiki-segundo-cerebro", &workspace_id, &[], false)
        .await
        .expect("second install");
    assert_eq!(first.routines, second.routines);
    let routines = schedules::list(&state.db, &workspace_id)
        .await
        .expect("list");
    assert_eq!(routines.len(), 1, "routines: {routines:?}");
}

#[tokio::test]
async fn recipe_io_triggers_create_and_remove_sources() {
    let (_temp, state, workspace_id) = test_state().await;
    let yaml = r#"
id: webhook-recipe
name: Webhook Recipe
triggers:
  - type: WEBHOOK
    config: { secret: teste }
flow:
  - type: step
    id: s1
    name: Um
    prompt: ok
"#;
    crate::recipes::store::save_yaml(&state.db, &workspace_id, "webhook-recipe", yaml)
        .await
        .expect("save recipe");
    crate::api::recipes::sync_recipe_trigger_sources(&state, &workspace_id, "webhook-recipe")
        .await
        .expect("sync source");
    let sources = crate::db::io_bus::list_inbound(&state.db, &workspace_id)
        .await
        .expect("sources");
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].source_type, "WEBHOOK_HTTP");
    let config: serde_json::Value = serde_json::from_str(&sources[0].config_json).expect("config");
    assert_eq!(config["recipe_id"], "webhook-recipe");

    let without = yaml.replace(
        "triggers:\n  - type: WEBHOOK\n    config: { secret: teste }",
        "triggers: []",
    );
    crate::recipes::store::save_yaml(&state.db, &workspace_id, "webhook-recipe", &without)
        .await
        .expect("save without trigger");
    crate::api::recipes::sync_recipe_trigger_sources(&state, &workspace_id, "webhook-recipe")
        .await
        .expect("sync removal");
    assert!(
        crate::db::io_bus::list_inbound(&state.db, &workspace_id)
            .await
            .expect("sources")
            .is_empty()
    );
}

#[tokio::test]
async fn skip_conflicts_preserves_custom_files_without_overwrite() {
    let (_temp, state, workspace_id) = test_state().await;
    let workspace = workspaces::get(&state.db, &workspace_id).await.expect("ws");
    let root = PathBuf::from(&workspace.root_path);
    let agent = root.join(".opencode/agents/assistente.md");
    std::fs::create_dir_all(agent.parent().unwrap()).expect("dir");
    std::fs::write(&agent, "custom").expect("custom");

    let result = install(&state, "essenciais", &workspace_id, &[], true)
        .await
        .expect("install");
    assert_eq!(result.skipped_conflicts, 1);
    assert_eq!(std::fs::read_to_string(&agent).unwrap(), "custom");
    assert!(root.join(".opencode/agents/pesquisador.md").is_file());

    install(
        &state,
        "essenciais",
        &workspace_id,
        &[".opencode/agents/assistente.md".into()],
        true,
    )
    .await
    .expect("overwrite");
    assert!(
        std::fs::read_to_string(&agent)
            .unwrap()
            .contains("Assistente")
    );
}

#[tokio::test]
async fn reports_an_available_pack_update() {
    let (_temp, state, workspace_id) = test_state().await;
    install(&state, "essenciais", &workspace_id, &[], false)
        .await
        .expect("install");
    sqlx::query(
        "UPDATE starter_pack_installs SET pack_version = '0.0.1'
             WHERE workspace_id = ? AND pack_id = 'essenciais'",
    )
    .bind(&workspace_id)
    .execute(&state.db)
    .await
    .expect("downgrade version");
    let (_, packs) = list(&state, Some(&workspace_id)).await.expect("list");
    let pack = packs
        .iter()
        .find(|pack| pack.id == "essenciais")
        .expect("pack");
    assert!(pack.update_available);
}

#[tokio::test]
async fn reports_partial_pack_and_repairs_missing_files() {
    let (_temp, state, workspace_id) = test_state().await;
    install(&state, "essenciais", &workspace_id, &[], false)
        .await
        .expect("install");
    let workspace = workspaces::get(&state.db, &workspace_id).await.expect("ws");
    let root = PathBuf::from(&workspace.root_path);
    let (_, packs) = list(&state, Some(&workspace_id)).await.expect("list");
    let pack = packs
        .iter()
        .find(|pack| pack.id == "essenciais")
        .expect("pack");
    assert_eq!(pack.health.status, "installed");

    std::fs::remove_file(root.join(".opencode/agents/assistente.md")).expect("remove");
    let (_, packs) = list(&state, Some(&workspace_id)).await.expect("list");
    let pack = packs
        .iter()
        .find(|pack| pack.id == "essenciais")
        .expect("pack");
    assert_eq!(pack.health.status, "partial");
    assert_eq!(pack.health.missing_files, 1);

    install(&state, "essenciais", &workspace_id, &[], false)
        .await
        .expect("repair");
    assert!(root.join(".opencode/agents/assistente.md").is_file());
    let (_, packs) = list(&state, Some(&workspace_id)).await.expect("list");
    let pack = packs
        .iter()
        .find(|pack| pack.id == "essenciais")
        .expect("pack");
    assert_eq!(pack.health.status, "installed");
}

#[tokio::test]
async fn reinstall_reuses_a_routine_kept_by_uninstall() {
    let (_temp, state, workspace_id) = test_state().await;
    let first = install(&state, "wiki-segundo-cerebro", &workspace_id, &[], false)
        .await
        .expect("install");
    let routine_id = first.routines[0].clone();
    schedules::mark_run(
        &state.db,
        &routine_id,
        "SUCCESS",
        &chrono::Utc::now().to_rfc3339(),
    )
    .await
    .expect("mark run");
    let removed = uninstall(&state, "wiki-segundo-cerebro", &workspace_id)
        .await
        .expect("uninstall");
    assert_eq!(removed.routines_kept, vec![routine_id.clone()]);

    let again = install(&state, "wiki-segundo-cerebro", &workspace_id, &[], false)
        .await
        .expect("reinstall");
    assert_eq!(again.routines, vec![routine_id]);
    assert_eq!(
        schedules::list(&state.db, &workspace_id)
            .await
            .expect("list")
            .len(),
        1
    );
}

#[tokio::test]
async fn installs_routines_inactive_and_removes_them() {
    let (_temp, state, workspace_id) = test_state().await;
    let result = install(&state, "wiki-segundo-cerebro", &workspace_id, &[], false)
        .await
        .expect("install");
    assert_eq!(result.routines.len(), 1);
    let routine = schedules::get(&state.db, &result.routines[0])
        .await
        .expect("routine");
    assert!(!routine.is_active);
    assert_eq!(routine.target_kind, "RECIPE");
    let removed = uninstall(&state, "wiki-segundo-cerebro", &workspace_id)
        .await
        .expect("uninstall");
    assert_eq!(removed.routines_removed, 1);
}

#[tokio::test]
async fn lists_and_installs_individual_pack_items() {
    let (_temp, state, workspace_id) = test_state().await;
    let items = items("essenciais").expect("items");
    assert!(
        items
            .iter()
            .any(|item| item.kind == "agent" && item.id == "assistente")
    );
    assert!(items.iter().any(|item| item.kind == "skill"));
    assert!(items.iter().any(|item| item.kind == "command"));
    assert!(items.iter().any(|item| item.kind == "recipe"));

    install_item(
        &state,
        "essenciais",
        &workspace_id,
        "agent",
        "assistente",
        false,
    )
    .await
    .expect("install agent");
    let workspace = workspaces::get(&state.db, &workspace_id)
        .await
        .expect("workspace");
    assert!(
        Path::new(&workspace.root_path)
            .join(".opencode/agents/assistente.md")
            .is_file()
    );
    install_item(
        &state,
        "essenciais",
        &workspace_id,
        "skill",
        "pesquisa-web",
        false,
    )
    .await
    .expect("install skill");
    assert!(
        Path::new(&workspace.root_path)
            .join(".opencode/skills/pesquisa-web/SKILL.md")
            .is_file()
    );
}

#[tokio::test]
async fn creates_a_workspace_from_a_template() {
    let (temp, state, _) = test_state().await;
    let root = temp.path().join("template-workspace");
    let result = create_from_template(
        &state,
        "conteudo-midia",
        Some("Conteúdo Teste"),
        Some(root.to_string_lossy().as_ref()),
        Some("#8b5cf6"),
    )
    .await
    .expect("template");
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    assert_eq!(
        result.installed.len(),
        7,
        "installed: {:?}",
        result.installed
    );
    assert!(
        result
            .installed
            .iter()
            .any(|pack| pack == "controle-opendesk")
    );
    assert!(root.join("README.md").is_file());
    assert!(
        root.join(".opencode/agents/roteirista-youtube.md")
            .is_file()
    );
    assert!(root.join("roteiros").is_dir());
}

#[test]
fn workspace_templates_do_not_collide() {
    for template in templates().expect("templates") {
        let mut seen: HashMap<String, String> = HashMap::new();
        for pack_id in &template.packs {
            let (manifest, dir) = load_manifest(pack_id).expect("manifest");
            for planned in planned_for(&manifest, dir).expect("planned") {
                if let Some(previous) = seen.insert(planned.target.clone(), pack_id.clone())
                    && previous != *pack_id
                {
                    panic!(
                        "template {}: {} definido por {} e {}",
                        template.id, planned.target, previous, pack_id
                    );
                }
            }
        }
    }
}

#[test]
fn rejects_path_traversal_in_folders() {
    assert!(safe_relative("../etc").is_err());
    assert!(safe_relative("/etc").is_err());
    assert!(safe_relative("input/../etc").is_err());
    assert!(safe_relative("input/dropzone").is_ok());
}

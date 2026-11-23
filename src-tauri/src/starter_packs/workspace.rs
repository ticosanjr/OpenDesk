//! Workspace lifecycle: base pack, uninstall, capability factory and templates.

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use serde::Serialize;

use crate::{
    app_state::AppState,
    config_sync::file_ops::read_text,
    db::{self, schedules, settings, workspaces},
    error::{AppError, AppResult},
};

use super::{
    BASE_PACK_ID, BUNDLE,
    bundle::{catalog, safe_relative},
    install::install,
    types::{
        InstallResult, InstalledFile, UninstallResult, WorkspaceTemplate, parse_stored_routines,
    },
};

/// Installs the operational baseline in a newly created workspace. Existing
/// files are never overwritten; conflicts are reported in the returned result
/// (or logged by callers that treat them as best effort).
pub async fn install_base_pack(
    state: &Arc<AppState>,
    workspace_id: &str,
) -> AppResult<InstallResult> {
    install(state, BASE_PACK_ID, workspace_id, &[], true).await
}

pub async fn uninstall(
    state: &Arc<AppState>,
    pack_id: &str,
    workspace_id: &str,
) -> AppResult<UninstallResult> {
    let workspace = workspaces::get(&state.db, workspace_id).await?;
    let root = Path::new(&workspace.root_path);
    let row = sqlx::query_as::<_, (String, String)>(
        "SELECT files_json, routines_json FROM starter_pack_installs
         WHERE workspace_id = ? AND pack_id = ?",
    )
    .bind(workspace_id)
    .bind(pack_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("pacote {pack_id} não instalado")))?;
    let files: Vec<InstalledFile> = serde_json::from_str(&row.0).unwrap_or_default();
    let routines = parse_stored_routines(&row.1);

    // Routines that already ran (or were activated) are kept, together with
    // their recipe file, so the user never ends up with a broken routine.
    let mut routines_removed = 0usize;
    let mut routines_kept = Vec::new();
    let mut kept_recipe_targets = std::collections::HashSet::new();
    for routine in routines {
        let routine_id = routine.id;
        if let Ok(schedule) = schedules::get(&state.db, &routine_id).await {
            if schedule.last_run_at.is_some() || schedule.is_active {
                if let Some(recipe_id) = schedule.recipe_id.clone() {
                    kept_recipe_targets.insert(format!(".opencode/recipes/{recipe_id}.yaml"));
                }
                routines_kept.push(routine_id);
            } else {
                let _ = crate::api::schedules::remove_trigger_sources(state, &routine_id).await;
                schedules::delete(&state.db, &routine_id).await?;
                routines_removed += 1;
            }
        }
    }

    let mut removed = 0usize;
    let mut kept = Vec::new();
    for file in files {
        if kept_recipe_targets.contains(&file.path) {
            kept.push(file.path);
            continue;
        }
        let path = root.join(&file.path);
        match read_text(&path)? {
            Some(snapshot) if snapshot.sha256 == file.sha256 => {
                std::fs::remove_file(&path)?;
                removed += 1;
            }
            Some(_) => kept.push(file.path),
            None => {}
        }
    }

    if removed > 0 {
        // Keep the recipe index in step with files removed from disk.
        let _ = crate::recipes::store::sync_workspace(&state.db, workspace_id, root).await;
    }
    sqlx::query("DELETE FROM starter_pack_installs WHERE workspace_id = ? AND pack_id = ?")
        .bind(workspace_id)
        .bind(pack_id)
        .execute(&state.db)
        .await?;
    let _ = crate::api::recipes::sync_all_recipe_trigger_sources(state).await;

    crate::api::files::invalidate_search_index();
    if let Err(error) = state.refresh_watcher().await {
        tracing::warn!(error = %error, "watcher refresh after pack uninstall failed");
    }

    Ok(UninstallResult {
        pack_id: pack_id.to_string(),
        removed,
        kept,
        routines_removed,
        routines_kept,
    })
}

/// Create the first-run capability factory workspace once. Existing users keep
/// their active workspace; the factory is additive and fully removable.
pub async fn ensure_default_factory_workspace(
    state: &Arc<AppState>,
) -> AppResult<Option<workspaces::Workspace>> {
    if settings::get(&state.db, "factory_workspace_created")
        .await?
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
    {
        return Ok(None);
    }
    let existing = workspaces::list(&state.db).await?;
    let had_workspaces = !existing.is_empty();
    let workspace = if let Some(found) = existing
        .iter()
        .find(|workspace| workspace.name == "Fábrica de Capacidades")
        .cloned()
    {
        found
    } else {
        let root_setting = settings::get(&state.db, "workspaces_root")
            .await?
            .and_then(|value| value.as_str().map(str::to_string))
            .unwrap_or_else(|| {
                crate::paths::default_workspaces_root()
                    .to_string_lossy()
                    .to_string()
            });
        let root = PathBuf::from(root_setting).join("fabrica-de-capacidades");
        workspaces::create_trusted(
            &state.db,
            &state.paths,
            db::models::CreateWorkspaceInput {
                name: "Fábrica de Capacidades".into(),
                root_path: root.to_string_lossy().to_string(),
                description: Some(
                    "Crie, avalie e melhore skills, agentes e comandos com metodologia eval-driven."
                        .into(),
                ),
                color_hex: Some("#8b5cf6".into()),
                default_router_policy: Some("COST_BENEFIT".into()),
                default_email_recipient: None,
                google_drive_folder_id: None,
                timezone: None,
            },
        )
        .await?
    };

    let template = templates()?
        .into_iter()
        .find(|template| template.id == "fabrica-capacidades");
    if let Some(template) = &template {
        for folder in &template.folders {
            safe_relative(folder)?;
            std::fs::create_dir_all(Path::new(&workspace.root_path).join(folder))?;
        }
        for (filename, content) in [
            ("README.md", template.readme.as_deref()),
            ("MEMORY.md", template.memory.as_deref()),
        ] {
            if let Some(content) = content {
                let path = Path::new(&workspace.root_path).join(filename);
                if !path.exists() {
                    std::fs::write(path, content)?;
                }
            }
        }
    }

    let packs = template.map(|template| template.packs).unwrap_or_else(|| {
        vec![
            BASE_PACK_ID.into(),
            "essenciais".into(),
            "meta-forge".into(),
        ]
    });
    for pack in packs {
        if let Err(error) = install(state, &pack, &workspace.id, &[], true).await {
            tracing::warn!(pack = %pack, error = %error, "default factory pack install failed");
            return Err(error);
        }
    }
    if !had_workspaces {
        settings::set_active_workspace(&state.db, &workspace.id).await?;
    }
    if let Err(error) = crate::proposals::ensure_workspace_bridge(state, &workspace.id).await {
        tracing::warn!(error = %error, "could not register config bridge for factory workspace");
    }
    settings::set(
        &state.db,
        "factory_workspace_created",
        &serde_json::Value::Bool(true),
    )
    .await?;
    if let Err(error) = state.refresh_watcher().await {
        tracing::warn!(error = %error, "watcher refresh after factory workspace failed");
    }
    Ok(Some(workspace))
}

pub fn templates() -> AppResult<Vec<WorkspaceTemplate>> {
    let catalog = catalog()?;
    let mut list = Vec::new();
    for id in &catalog.templates {
        let file = BUNDLE
            .get_file(format!("templates/{id}.json"))
            .ok_or_else(|| AppError::NotFound(format!("template {id}")))?;
        let template: WorkspaceTemplate = serde_json::from_slice(file.contents())
            .map_err(|error| AppError::BadRequest(format!("template inválido {id}: {error}")))?;
        list.push(template);
    }
    Ok(list)
}

#[derive(Debug, Clone, Serialize)]
pub struct TemplateResult {
    pub workspace: workspaces::Workspace,
    pub installed: Vec<String>,
    pub errors: Vec<String>,
}

pub async fn create_from_template(
    state: &Arc<AppState>,
    template_id: &str,
    name: Option<&str>,
    root_path: Option<&str>,
    color: Option<&str>,
) -> AppResult<TemplateResult> {
    let template = templates()?
        .into_iter()
        .find(|template| template.id == template_id)
        .ok_or_else(|| AppError::NotFound(format!("template {template_id}")))?;
    let name = name
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(&template.name)
        .to_string();
    let root = match root_path.map(str::trim).filter(|value| !value.is_empty()) {
        Some(path) => PathBuf::from(path),
        None => crate::paths::default_workspaces_root().join(crate::dsh_router::slugify(&name)),
    };
    std::fs::create_dir_all(&root)?;
    let workspace = workspaces::create(
        &state.db,
        &state.paths,
        db::models::CreateWorkspaceInput {
            name: name.clone(),
            root_path: root.to_string_lossy().to_string(),
            description: Some(template.description.clone()),
            color_hex: Some(color.unwrap_or(&template.color).to_string()),
            default_router_policy: Some(template.router_policy.clone()),
            default_email_recipient: None,
            google_drive_folder_id: None,
            timezone: None,
        },
    )
    .await?;
    crate::db::settings::set_active_workspace(&state.db, &workspace.id).await?;
    if let Err(error) = crate::proposals::ensure_workspace_bridge(state, &workspace.id).await {
        tracing::warn!(error = %error, "could not register config bridge for template workspace");
    }

    for folder in &template.folders {
        safe_relative(folder)?;
        std::fs::create_dir_all(Path::new(&workspace.root_path).join(folder))?;
    }
    for (filename, content) in [
        ("README.md", template.readme.as_deref()),
        ("MEMORY.md", template.memory.as_deref()),
    ] {
        if let Some(content) = content {
            let path = Path::new(&workspace.root_path).join(filename);
            if !path.exists() {
                std::fs::write(path, content)?;
            }
        }
    }

    let mut installed = Vec::new();
    let mut errors = Vec::new();
    match install_base_pack(state, &workspace.id).await {
        Ok(result) => installed.push(result.pack_id),
        Err(error) => errors.push(format!("{BASE_PACK_ID}: {error}")),
    }
    for pack_id in &template.packs {
        match install(state, pack_id, &workspace.id, &[], false).await {
            Ok(result) => installed.push(result.pack_id),
            Err(error) => errors.push(format!("{pack_id}: {error}")),
        }
    }
    if let Err(error) = state.refresh_watcher().await {
        tracing::warn!(error = %error, "watcher refresh after template create failed");
    }
    Ok(TemplateResult {
        workspace,
        installed,
        errors,
    })
}

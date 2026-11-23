//! Pack and item installation: preview plans, conflict handling and DB records.

use std::{path::Path, sync::Arc};

use crate::{
    app_state::AppState,
    config_sync::file_ops::{read_text, write_text_atomic},
    db::{schedules, workspaces},
    error::{AppError, AppResult},
    recipes::store as recipe_store,
};

use super::{
    bundle::{load_manifest, safe_relative},
    plan::{load_pack, planned_for},
    types::{
        InstallPlan, InstallResult, InstalledFile, InstalledRoutine, ItemInstallResult, PlanFile,
        PlanRoutine, parse_stored_routines,
    },
};

/// Installs one item from a pack. The pack itself is not marked as installed;
/// item installs are intentionally independent.
pub async fn install_item(
    state: &Arc<AppState>,
    pack_id: &str,
    workspace_id: &str,
    kind: &str,
    id: &str,
    overwrite: bool,
) -> AppResult<ItemInstallResult> {
    let workspace = workspaces::get(&state.db, workspace_id).await?;
    let root = Path::new(&workspace.root_path);
    let (manifest, dir) = load_manifest(pack_id)?;
    let planned = planned_for(&manifest, dir)?;
    let item_prefix = format!(".opencode/skills/{id}/");
    let item_suffix = format!("/{id}.md");
    let item_recipe_suffix = format!("/{id}.yaml");
    let matching = planned
        .iter()
        .filter(|file| match kind {
            "agent" | "command" => file.target.ends_with(&item_suffix),
            "skill" => file.target.starts_with(&item_prefix),
            "recipe" => file.target.ends_with(&item_recipe_suffix),
            _ => false,
        })
        .collect::<Vec<_>>();
    if matching.is_empty() {
        return Err(AppError::NotFound(format!(
            "item {kind} {id} no pacote {pack_id}"
        )));
    }

    let mut written = 0;
    let mut skipped_conflicts = 0;
    let mut recipes_changed = false;
    for file in matching {
        if let Some(snapshot) = read_text(&root.join(&file.target))?
            && snapshot.sha256 != file.sha256
            && !overwrite
        {
            skipped_conflicts += 1;
            continue;
        }
        let text = std::str::from_utf8(&file.bytes)
            .map_err(|_| AppError::Internal(format!("arquivo não é UTF-8: {}", file.target)))?;
        write_text_atomic(
            &root.join(&file.target),
            text,
            None,
            &state.paths.backups_dir,
        )?;
        if file.kind == "recipe" {
            recipes_changed = true;
        }
        written += 1;
    }
    if recipes_changed {
        recipe_store::sync_workspace(&state.db, workspace_id, root).await?;
        crate::api::recipes::sync_all_recipe_trigger_sources(state).await?;
    }
    if written == 0 && skipped_conflicts > 0 {
        return Err(AppError::Conflict(format!(
            "{kind} {id} já existe com conteúdo diferente; confirme a substituição"
        )));
    }
    Ok(ItemInstallResult {
        kind: kind.to_string(),
        id: id.to_string(),
        written,
        skipped_conflicts,
    })
}

pub async fn preview(
    state: &Arc<AppState>,
    pack_id: &str,
    workspace_id: &str,
) -> AppResult<InstallPlan> {
    let workspace = workspaces::get(&state.db, workspace_id).await?;
    let (manifest, dir) = load_manifest(pack_id)?;
    let loaded = load_pack(manifest, dir, workspace_id)?;
    let root = Path::new(&workspace.root_path);
    let mut files = Vec::new();
    let mut conflicts = 0;
    for planned in &loaded.planned {
        let status = match read_text(&root.join(&planned.target))? {
            Some(snapshot) if snapshot.sha256 == planned.sha256 => "identical",
            Some(_) => {
                conflicts += 1;
                "conflict"
            }
            None => "create",
        };
        files.push(PlanFile {
            path: planned.target.clone(),
            kind: planned.kind.clone(),
            status: status.to_string(),
            sha256: planned.sha256.clone(),
        });
    }
    let routines = loaded
        .routines
        .iter()
        .map(|(file, input)| PlanRoutine {
            file: file.clone(),
            name: input.name.clone(),
            trigger_type: input.trigger_type.clone(),
            recipe_id: input.recipe_id.clone(),
        })
        .collect();
    Ok(InstallPlan {
        pack_id: pack_id.to_string(),
        version: loaded.manifest.version,
        files,
        routines,
        folders: loaded.manifest.contents.folders.clone(),
        requires: loaded.manifest.requires.clone(),
        conflicts,
    })
}

pub async fn install(
    state: &Arc<AppState>,
    pack_id: &str,
    workspace_id: &str,
    overwrite: &[String],
    skip_conflicts: bool,
) -> AppResult<InstallResult> {
    let workspace = workspaces::get(&state.db, workspace_id).await?;
    let root = Path::new(&workspace.root_path);
    let (manifest, dir) = load_manifest(pack_id)?;
    let loaded = load_pack(manifest, dir, workspace_id)?;

    for folder in &loaded.manifest.contents.folders {
        safe_relative(folder)?;
        std::fs::create_dir_all(root.join(folder))?;
    }

    let overwrite: std::collections::HashSet<&str> = overwrite.iter().map(String::as_str).collect();
    let mut conflicts = Vec::new();
    for planned in &loaded.planned {
        if let Some(snapshot) = read_text(&root.join(&planned.target))?
            && snapshot.sha256 != planned.sha256
            && !overwrite.contains(planned.target.as_str())
        {
            conflicts.push(planned.target.clone());
        }
    }
    if !conflicts.is_empty() && !skip_conflicts {
        return Err(AppError::Conflict(format!(
            "estes arquivos já existem com conteúdo diferente: {}",
            conflicts.join(", ")
        )));
    }

    let mut written = 0usize;
    let mut skipped = 0usize;
    let mut skipped_conflicts = 0usize;
    let mut recipes_changed = false;
    for planned in &loaded.planned {
        if let Some(snapshot) = read_text(&root.join(&planned.target))?
            && snapshot.sha256 == planned.sha256
        {
            skipped += 1;
            continue;
        }
        if let Some(snapshot) = read_text(&root.join(&planned.target))?
            && snapshot.sha256 != planned.sha256
            && !overwrite.contains(planned.target.as_str())
        {
            skipped_conflicts += 1;
            continue;
        }
        let text = std::str::from_utf8(&planned.bytes)
            .map_err(|_| AppError::Internal(format!("arquivo não é UTF-8: {}", planned.target)))?;
        write_text_atomic(
            &root.join(&planned.target),
            text,
            None,
            &state.paths.backups_dir,
        )?;
        if planned.kind == "recipe" {
            recipes_changed = true;
        }
        written += 1;
    }
    if recipes_changed {
        recipe_store::sync_workspace(&state.db, workspace_id, root).await?;
        crate::api::recipes::sync_all_recipe_trigger_sources(state).await?;
    }

    let previous = sqlx::query_scalar::<_, String>(
        "SELECT routines_json FROM starter_pack_installs
         WHERE workspace_id = ? AND pack_id = ?",
    )
    .bind(workspace_id)
    .bind(&loaded.manifest.id)
    .fetch_optional(&state.db)
    .await?
    .map(|json| parse_stored_routines(&json))
    .unwrap_or_default();

    let mut installed_routines = Vec::new();
    for (index, (file, input)) in loaded.routines.iter().enumerate() {
        let mut existing_id = previous
            .iter()
            .find(|routine| !routine.file.is_empty() && routine.file == *file)
            .map(|routine| routine.id.clone());
        if existing_id.is_none()
            && let Some(routine) = previous.get(index)
            && routine.file.is_empty()
        {
            existing_id = Some(routine.id.clone());
        }
        if existing_id.is_none()
            && let Some(recipe_id) = input.recipe_id.as_deref()
        {
            if let Some(routine) =
                schedules::find_by_name_recipe(&state.db, workspace_id, &input.name, recipe_id)
                    .await?
            {
                existing_id = Some(routine.id);
            } else if let Some(routine) =
                schedules::find_by_recipe(&state.db, workspace_id, recipe_id).await?
            {
                existing_id = Some(routine.id);
            }
        }

        if let Some(id) = existing_id
            && schedules::get(&state.db, &id).await.is_ok()
        {
            let update = schedules::UpdateScheduleInput {
                name: Some(input.name.clone()),
                description: input.description.clone(),
                cron_expression: Some(input.cron_expression.clone()),
                timezone: Some(input.timezone.clone()),
                agent_name: input.agent_name.clone(),
                command_or_prompt: Some(input.command_or_prompt.clone()),
                trigger_type: Some(input.trigger_type.clone()),
                trigger_config_json: input.trigger_config_json.clone(),
                target_kind: Some(input.target_kind.clone()),
                recipe_id: input.recipe_id.clone(),
                router_policy: Some(input.router_policy.clone()),
                output_folder_template: Some(input.output_folder_template.clone()),
                notify_email_recipients: input.notify_email_recipients.clone(),
                notify_signal: Some(input.notify_signal),
                notify_signal_recipients: input.notify_signal_recipients.clone(),
                notify_telegram: Some(input.notify_telegram),
                notify_telegram_recipients: input.notify_telegram_recipients.clone(),
                notify_only_on_change: Some(input.notify_only_on_change),
                recurrence_json: input.recurrence_json.clone(),
                one_time: Some(input.one_time),
                misfire_policy: Some(input.misfire_policy.clone()),
                safe_readonly_mode: Some(input.safe_readonly_mode),
                sync_to_google_docs: Some(input.sync_to_google_docs),
                sync_to_google_calendar: Some(input.sync_to_google_calendar),
                is_active: None,
            };
            let schedule = schedules::update(&state.db, &id, update).await?;
            crate::api::schedules::sync_trigger_source(state, &schedule).await?;
            installed_routines.push(InstalledRoutine {
                id: schedule.id,
                file: file.clone(),
                name: schedule.name,
            });
            continue;
        }

        let schedule = schedules::create(&state.db, input.clone()).await?;
        crate::api::schedules::sync_trigger_source(state, &schedule).await?;
        installed_routines.push(InstalledRoutine {
            id: schedule.id,
            file: file.clone(),
            name: schedule.name.clone(),
        });
    }
    let routine_ids: Vec<String> = installed_routines
        .iter()
        .map(|routine| routine.id.clone())
        .collect();

    let files_json = serde_json::to_string(
        &loaded
            .planned
            .iter()
            .map(|planned| InstalledFile {
                path: planned.target.clone(),
                sha256: planned.sha256.clone(),
            })
            .collect::<Vec<_>>(),
    )?;
    if !installed_routines.is_empty() {
        state.io_bus.reload(state.clone()).await?;
    }
    let routines_json = serde_json::to_string(&installed_routines)?;
    let id = uuid::Uuid::now_v7().to_string();
    sqlx::query(
        "INSERT INTO starter_pack_installs
            (id, workspace_id, pack_id, pack_version, files_json, routines_json)
         VALUES (?, ?, ?, ?, ?, ?)
         ON CONFLICT(workspace_id, pack_id) DO UPDATE SET
            pack_version = excluded.pack_version,
            files_json = excluded.files_json,
            routines_json = excluded.routines_json,
            installed_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')",
    )
    .bind(id)
    .bind(workspace_id)
    .bind(&loaded.manifest.id)
    .bind(&loaded.manifest.version)
    .bind(files_json)
    .bind(routines_json)
    .execute(&state.db)
    .await?;

    crate::api::files::invalidate_search_index();
    if let Err(error) = state.refresh_watcher().await {
        tracing::warn!(error = %error, "watcher refresh after pack install failed");
    }

    Ok(InstallResult {
        pack_id: loaded.manifest.id,
        version: loaded.manifest.version,
        written,
        skipped,
        skipped_conflicts,
        routines: routine_ids,
        folders: loaded.manifest.contents.folders,
    })
}

use crate::{
    app_state::AppState,
    config_sync::{
        agents::{self, AgentWriteInput},
        commands::{self, CommandWriteInput},
        file_ops, jsonc_editor,
        paths::StudioPaths,
        skills::{self, SkillWriteInput},
    },
    db::{proposals, schedules, workspaces},
    error::{AppError, AppResult},
    events::OpenDeskWsEvent,
    recipes::store as recipe_store,
};
use std::sync::Arc;

pub async fn apply(state: &Arc<AppState>, proposal: &proposals::ConfigProposal) -> AppResult<()> {
    let workspace = workspaces::get(&state.db, &proposal.workspace_id).await?;
    let root = std::path::PathBuf::from(&workspace.root_path);
    let paths = StudioPaths::new(Some(root.clone()));
    let payload: serde_json::Value = serde_json::from_str(&proposal.payload_json)
        .map_err(|error| AppError::BadRequest(format!("proposal payload inválido: {error}")))?;
    let backup = state.paths.backups_dir.clone();
    match proposal.kind.as_str() {
        "AGENT" => {
            let id = payload
                .get("id")
                .and_then(|value| value.as_str())
                .ok_or_else(|| AppError::BadRequest("agent proposal sem id".into()))?;
            let scope = payload
                .get("scope")
                .and_then(|value| value.as_str())
                .unwrap_or("workspace");
            if proposal.action == "DELETE" {
                agents::delete(&paths, scope, id)?;
                return Ok(());
            }
            let input: AgentWriteInput = serde_json::from_value(payload.clone())
                .map_err(|error| AppError::BadRequest(format!("agent inválido: {error}")))?;
            agents::write(&paths, scope, id, input, &backup)?;
        }
        "SKILL" => {
            let id = payload
                .get("id")
                .and_then(|value| value.as_str())
                .ok_or_else(|| AppError::BadRequest("skill proposal sem id".into()))?;
            let scope = payload
                .get("scope")
                .and_then(|value| value.as_str())
                .unwrap_or("workspace");
            if proposal.action == "DELETE" {
                skills::delete(&paths, scope, id)?;
                return Ok(());
            }
            let input: SkillWriteInput = serde_json::from_value(payload.clone())
                .map_err(|error| AppError::BadRequest(format!("skill inválida: {error}")))?;
            skills::write(&paths, scope, id, input, &backup)?;
        }
        "COMMAND" => {
            let id = payload
                .get("id")
                .and_then(|value| value.as_str())
                .ok_or_else(|| AppError::BadRequest("command proposal sem id".into()))?;
            let scope = payload
                .get("scope")
                .and_then(|value| value.as_str())
                .unwrap_or("workspace");
            if proposal.action == "DELETE" {
                commands::delete(&paths, scope, id)?;
                return Ok(());
            }
            let input: CommandWriteInput = serde_json::from_value(payload.clone())
                .map_err(|error| AppError::BadRequest(format!("command inválido: {error}")))?;
            commands::write(&paths, scope, id, input, &backup)?;
        }
        "RECIPE" => {
            let id = payload
                .get("id")
                .and_then(|value| value.as_str())
                .ok_or_else(|| AppError::BadRequest("recipe proposal sem id".into()))?;
            if proposal.action == "DELETE" {
                let index = recipe_store::get(&state.db, id).await?;
                std::fs::remove_file(index.file_path)?;
                recipe_store::sync_workspace(&state.db, &proposal.workspace_id, &root).await?;
                return Ok(());
            }
            let yaml = payload
                .get("yaml")
                .and_then(|value| value.as_str())
                .ok_or_else(|| AppError::BadRequest("recipe proposal sem yaml".into()))?;
            recipe_store::save_yaml(&state.db, &proposal.workspace_id, id, yaml).await?;
        }
        "ROUTINE" => {
            let id = payload
                .get("id")
                .and_then(|value| value.as_str())
                .map(str::to_string);
            if proposal.action == "DELETE" {
                if let Some(id) = id {
                    schedules::delete(&state.db, &id).await?;
                }
                return Ok(());
            }
            let mut input: schedules::CreateScheduleInput = serde_json::from_value(payload.clone())
                .map_err(|error| AppError::BadRequest(format!("rotina inválida: {error}")))?;
            input.workspace_id = proposal.workspace_id.clone();
            schedules::create(&state.db, input).await?;
        }
        "MCP" => {
            let name = payload
                .get("name")
                .and_then(|value| value.as_str())
                .ok_or_else(|| AppError::BadRequest("mcp proposal sem name".into()))?;
            let scope = payload
                .get("scope")
                .and_then(|value| value.as_str())
                .unwrap_or("workspace");
            let config_path = paths.config_path(scope)?;
            if proposal.action == "DELETE" {
                jsonc_editor::delete_mcp_server(&config_path, name, None, &backup)?;
                return Ok(());
            }
            let server = payload
                .get("server")
                .cloned()
                .ok_or_else(|| AppError::BadRequest("mcp proposal sem server".into()))?;
            jsonc_editor::upsert_mcp_server(&config_path, name, &server, None, &backup)?;
        }
        "CONFIG" => {
            let relative = payload
                .get("path")
                .and_then(|value| value.as_str())
                .ok_or_else(|| AppError::BadRequest("config proposal sem path".into()))?;
            let relative = file_ops::validate_relative_file(relative)?;
            let content = payload
                .get("content")
                .and_then(|value| value.as_str())
                .ok_or_else(|| AppError::BadRequest("config proposal sem content".into()))?;
            let path = root.join(".opencode").join(relative);
            file_ops::write_text_atomic(&path, content, None, &backup)?;
        }
        other => {
            return Err(AppError::BadRequest(format!(
                "proposal kind não aplicável: {other}"
            )));
        }
    }
    Ok(())
}

pub async fn resolve(
    state: &Arc<AppState>,
    proposal_id: &str,
    approved: bool,
    resolved_by: Option<&str>,
) -> AppResult<proposals::ConfigProposal> {
    let proposal = proposals::get(&state.db, proposal_id).await?;
    if proposal.status != "PENDING" {
        return Err(AppError::Conflict("proposta já resolvida".into()));
    }
    if !approved {
        let result =
            proposals::resolve(&state.db, proposal_id, "REJECTED", resolved_by, None).await?;
        state.events.publish(OpenDeskWsEvent::ProposalResolved {
            proposal_id: result.id.clone(),
            status: result.status.clone(),
            workspace_id: result.workspace_id.clone(),
        });
        return Ok(result);
    }
    proposals::resolve(&state.db, proposal_id, "APPROVED", resolved_by, None).await?;
    match apply(state, &proposal).await {
        Ok(()) => {
            let result = proposals::finalize(&state.db, proposal_id, "APPLIED", None).await?;
            state.events.publish(OpenDeskWsEvent::ProposalResolved {
                proposal_id: result.id.clone(),
                status: result.status.clone(),
                workspace_id: result.workspace_id.clone(),
            });
            Ok(result)
        }
        Err(error) => {
            let result = proposals::finalize(
                &state.db,
                proposal_id,
                "FAILED",
                Some(&format!("{error:#}")),
            )
            .await?;
            state.events.publish(OpenDeskWsEvent::ProposalResolved {
                proposal_id: result.id.clone(),
                status: result.status.clone(),
                workspace_id: result.workspace_id.clone(),
            });
            Ok(result)
        }
    }
}

/// Make the OpenDesk configuration MCP available to every workspace.
pub async fn ensure_workspace_bridge(state: &AppState, workspace_id: &str) -> AppResult<()> {
    let workspace = workspaces::get(&state.db, workspace_id).await?;
    let paths = StudioPaths::new(Some(std::path::PathBuf::from(&workspace.root_path)));
    let config_path = paths.config_path("workspace")?;
    let executable = crate::api::google::stable_executable()?;
    let server = serde_json::json!({
        "type": "local",
        "command": [executable.to_string_lossy(), "mcp-config"],
        "environment": {
            "OPENDESK_WORKSPACE_ID": workspace_id,
            "OPENDESK_DATA_DIR": state.paths.data_dir.to_string_lossy(),
            "OPENDESK_CONFIG_DIR": state.paths.config_dir.to_string_lossy(),
        },
        "disabled": false
    });
    jsonc_editor::upsert_mcp_server(
        &config_path,
        "opendesk-config",
        &server,
        None,
        &state.paths.backups_dir,
    )?;
    Ok(())
}

pub async fn ensure_all(state: &Arc<AppState>) -> AppResult<()> {
    for workspace in workspaces::list(&state.db).await? {
        if let Err(error) = ensure_workspace_bridge(state, &workspace.id).await {
            tracing::warn!(workspace = %workspace.id, error = %error, "config MCP bridge not registered");
        }
    }
    Ok(())
}

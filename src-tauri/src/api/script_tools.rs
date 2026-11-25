use crate::{
    api::bundles::{
        BundleImportBody, SimpleImportMode, envelope, expect_array, validate_envelope,
        validate_items,
    },
    app_state::AppState,
    db::script_tools::{self, CreateScriptToolInput, ScriptTool, UpdateScriptToolInput},
    error::{AppError, AppResult},
    script_tools::{self as executor, ScriptRunResult},
};
use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::Arc;

#[derive(Debug, Deserialize, Default)]
pub struct ToolsQuery {
    pub workspace_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateBody {
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(flatten)]
    pub input: CreateScriptToolInput,
}

#[derive(Debug, Deserialize, Default)]
pub struct UpdateBody {
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(flatten)]
    pub input: UpdateScriptToolInput,
}

#[derive(Debug, Deserialize)]
pub struct TestBody {
    #[serde(default = "empty_object")]
    pub input: Value,
}

fn empty_object() -> Value {
    json!({})
}

fn is_global(scope: Option<&str>) -> bool {
    scope.is_some_and(|value| value.trim().eq_ignore_ascii_case("global"))
}

async fn workspace(
    state: &AppState,
    requested: Option<&str>,
    scope: Option<&str>,
) -> AppResult<Option<String>> {
    if is_global(scope) {
        return Ok(None);
    }
    crate::api::tasks::resolve_workspace_id(state, requested)
        .await
        .map(Some)
}

pub async fn list(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ToolsQuery>,
) -> AppResult<Json<Value>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    let tools = script_tools::list(&state.db, Some(&workspace_id)).await?;
    Ok(Json(json!({
        "tools": tools,
        "runtimes": executor::RUNTIMES,
    })))
}

pub async fn create(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ToolsQuery>,
    Json(body): Json<CreateBody>,
) -> AppResult<Json<ScriptTool>> {
    let workspace_id = workspace(
        &state,
        query
            .workspace_id
            .as_deref()
            .or(body.input.workspace_id.as_deref()),
        body.scope.as_deref(),
    )
    .await?;
    executor::validate_fields(
        &body.input.slug,
        &body.input.name,
        &body.input.runtime,
        body.input.custom_command.as_deref(),
        &body.input.script_body,
        &body.input.input_schema_json,
        body.input.timeout_secs,
    )?;
    executor::validate_env_refs(&body.input.env_secret_refs_json, &state.paths)?;
    let mut input = body.input;
    input.workspace_id = workspace_id;
    Ok(Json(script_tools::create(&state.db, input).await?))
}

pub async fn update(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<UpdateBody>,
) -> AppResult<Json<ScriptTool>> {
    let current = script_tools::get(&state.db, &id).await?;
    if let Some(scope) = body.scope.as_deref() {
        let global = is_global(Some(scope));
        if global != current.workspace_id.is_none() {
            return Err(AppError::BadRequest(
                "não é possível mover a ferramenta entre escopos".into(),
            ));
        }
    }
    let mut preview = current.clone();
    if let Some(value) = &body.input.slug {
        preview.slug = value.clone();
    }
    if let Some(value) = &body.input.name {
        preview.name = value.clone();
    }
    if let Some(value) = &body.input.runtime {
        preview.runtime = value.clone();
    }
    if let Some(value) = &body.input.custom_command {
        preview.custom_command = value.clone();
    }
    if let Some(value) = &body.input.script_body {
        preview.script_body = value.clone();
    }
    if let Some(value) = &body.input.input_schema_json {
        preview.input_schema_json = value.clone();
    }
    if let Some(value) = &body.input.env_secret_refs_json {
        preview.env_secret_refs_json = value.clone();
    }
    if let Some(value) = body.input.timeout_secs {
        preview.timeout_secs = value;
    }
    executor::validate_tool(&preview)?;
    executor::validate_env_refs(&preview.env_secret_refs_json, &state.paths)?;
    Ok(Json(
        script_tools::update(&state.db, &id, body.input).await?,
    ))
}

pub async fn delete(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<Value>> {
    script_tools::delete(&state.db, &id).await?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn test(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<TestBody>,
) -> AppResult<Json<ScriptRunResult>> {
    let tool = script_tools::get(&state.db, &id).await?;
    let workspace_id = match tool.workspace_id.clone() {
        Some(id) => id,
        None => crate::api::tasks::resolve_workspace_id(&state, None).await?,
    };
    Ok(Json(
        executor::execute(&state.db, &state.paths, &workspace_id, &tool, &body.input).await?,
    ))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptToolExport {
    pub scope: String,
    pub slug: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub agent_prompt: String,
    pub runtime: String,
    #[serde(default)]
    pub custom_command: Option<String>,
    pub script_body: String,
    #[serde(default = "default_schema")]
    pub input_schema_json: String,
    #[serde(default = "default_env")]
    pub env_secret_refs_json: String,
    #[serde(default = "default_timeout")]
    pub timeout_secs: i64,
    #[serde(default)]
    pub require_approval: bool,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_schema() -> String {
    r#"{"type":"object","properties":{}}"#.into()
}
fn default_env() -> String {
    "{}".into()
}
fn default_timeout() -> i64 {
    30
}
fn default_true() -> bool {
    true
}

fn export_of(tool: &ScriptTool) -> ScriptToolExport {
    ScriptToolExport {
        scope: if tool.workspace_id.is_none() {
            "global".into()
        } else {
            "workspace".into()
        },
        slug: tool.slug.clone(),
        name: tool.name.clone(),
        description: tool.description.clone(),
        agent_prompt: tool.agent_prompt.clone(),
        runtime: tool.runtime.clone(),
        custom_command: tool.custom_command.clone(),
        script_body: tool.script_body.clone(),
        input_schema_json: tool.input_schema_json.clone(),
        env_secret_refs_json: tool.env_secret_refs_json.clone(),
        timeout_secs: tool.timeout_secs,
        require_approval: tool.require_approval,
        enabled: tool.enabled,
    }
}

pub async fn export(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ToolsQuery>,
) -> AppResult<Json<Value>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    let tools = script_tools::list(&state.db, Some(&workspace_id)).await?;
    Ok(Json(envelope(
        "script_tools",
        Some(&workspace_id),
        json!({ "tools": tools.iter().map(export_of).collect::<Vec<_>>() }),
    )))
}

fn input_of(item: &ScriptToolExport, workspace_id: Option<&str>) -> CreateScriptToolInput {
    CreateScriptToolInput {
        workspace_id: if item.scope == "global" {
            None
        } else {
            workspace_id.map(str::to_string)
        },
        slug: item.slug.clone(),
        name: item.name.clone(),
        description: item.description.clone(),
        agent_prompt: item.agent_prompt.clone(),
        runtime: item.runtime.clone(),
        custom_command: item.custom_command.clone(),
        script_body: item.script_body.clone(),
        input_schema_json: item.input_schema_json.clone(),
        env_secret_refs_json: item.env_secret_refs_json.clone(),
        timeout_secs: item.timeout_secs,
        require_approval: item.require_approval,
        enabled: item.enabled,
    }
}

pub async fn import(
    State(state): State<Arc<AppState>>,
    Json(body): Json<BundleImportBody>,
) -> AppResult<Json<Value>> {
    let items = validate_envelope(&body.bundle, "script_tools")?;
    validate_items(&items)?;
    let tools = expect_array(&items, "tools")?
        .iter()
        .map(|value| {
            serde_json::from_value::<ScriptToolExport>(value.clone())
                .map_err(|error| AppError::BadRequest(format!("ferramenta inválida: {error}")))
        })
        .collect::<AppResult<Vec<_>>>()?;
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, body.workspace_id.as_deref()).await?;
    for item in &tools {
        executor::validate_fields(
            &item.slug,
            &item.name,
            &item.runtime,
            item.custom_command.as_deref(),
            &item.script_body,
            &item.input_schema_json,
            item.timeout_secs,
        )?;
        executor::validate_env_refs(&item.env_secret_refs_json, &state.paths)?;
        if item.scope != "global" && item.scope != "workspace" {
            return Err(AppError::BadRequest(format!(
                "escopo inválido: {}",
                item.scope
            )));
        }
    }
    if body.mode == SimpleImportMode::Replace {
        let existing = script_tools::list(&state.db, Some(&workspace_id)).await?;
        let global = tools.iter().any(|tool| tool.scope == "global");
        for tool in existing {
            let matches = (tool.workspace_id.is_none() && global)
                || tool.workspace_id.as_deref() == Some(workspace_id.as_str());
            if matches {
                script_tools::delete(&state.db, &tool.id).await?;
            }
        }
    }
    let mut created = 0;
    let mut updated = 0;
    for item in &tools {
        let scope_workspace = if item.scope == "global" {
            None
        } else {
            Some(workspace_id.as_str())
        };
        let input = input_of(item, scope_workspace);
        match script_tools::find_by_slug(&state.db, scope_workspace, &item.slug).await? {
            Some(current) => {
                script_tools::update(
                    &state.db,
                    &current.id,
                    UpdateScriptToolInput {
                        slug: Some(input.slug),
                        name: Some(input.name),
                        description: Some(input.description),
                        agent_prompt: Some(input.agent_prompt),
                        runtime: Some(input.runtime),
                        custom_command: Some(input.custom_command),
                        script_body: Some(input.script_body),
                        input_schema_json: Some(input.input_schema_json),
                        env_secret_refs_json: Some(input.env_secret_refs_json),
                        timeout_secs: Some(input.timeout_secs),
                        require_approval: Some(input.require_approval),
                        enabled: Some(input.enabled),
                    },
                )
                .await?;
                updated += 1;
            }
            None => {
                script_tools::create(&state.db, input).await?;
                created += 1;
            }
        }
    }
    Ok(Json(json!({
        "ok": true,
        "kind": "script_tools",
        "created": created,
        "updated": updated,
    })))
}

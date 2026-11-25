//! Versioned export/import envelopes shared by the per-kind endpoints plus the
//! batch pack and workspace bundles.
//!
//! Every import validates the envelope (schema version and kind) before any
//! write, so a wrong file never partially mutates the database.

use std::sync::Arc;

use axum::{
    Json,
    extract::{Query, State},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{
    app_state::AppState,
    config_bundle::{self, BundleSections, ConfigBundle, ImportMode, ImportRequest},
    db::{schedules, workspaces},
    error::{AppError, AppResult},
    recipes::store as recipe_store,
    starter_packs::{self, PackTransfer},
};

pub const BUNDLE_SCHEMA_VERSION: u32 = 1;
pub const MAX_ITEMS: usize = 20_000;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SimpleImportMode {
    #[default]
    Merge,
    Replace,
}

impl From<SimpleImportMode> for ImportMode {
    fn from(value: SimpleImportMode) -> Self {
        match value {
            SimpleImportMode::Merge => ImportMode::MergeOverwrite,
            SimpleImportMode::Replace => ImportMode::ReplaceSection,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct BundleImportBody {
    #[serde(default)]
    pub workspace_id: Option<String>,
    #[serde(default)]
    pub mode: SimpleImportMode,
    pub bundle: Value,
}

pub fn envelope(kind: &str, source_workspace: Option<&str>, items: Value) -> Value {
    json!({
        "schema_version": BUNDLE_SCHEMA_VERSION,
        "kind": kind,
        "exported_at": chrono::Utc::now().to_rfc3339(),
        "source_workspace": source_workspace,
        "items": items,
    })
}

/// Validates schema version and kind, returning the `items` payload.
pub fn validate_envelope(bundle: &Value, expected: &str) -> AppResult<Value> {
    let object = bundle
        .as_object()
        .ok_or_else(|| AppError::BadRequest("bundle deve ser um objeto JSON".into()))?;
    let schema = object
        .get("schema_version")
        .and_then(Value::as_u64)
        .ok_or_else(|| AppError::BadRequest("bundle sem schema_version".into()))?;
    if schema != u64::from(BUNDLE_SCHEMA_VERSION) {
        return Err(AppError::BadRequest(format!(
            "schema_version {schema} não suportado (esperado {BUNDLE_SCHEMA_VERSION})"
        )));
    }
    let kind = object
        .get("kind")
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::BadRequest("bundle sem kind".into()))?;
    if kind != expected {
        return Err(AppError::BadRequest(format!(
            "kind incompatível: bundle é '{kind}', esperado '{expected}'"
        )));
    }
    object
        .get("items")
        .cloned()
        .ok_or_else(|| AppError::BadRequest("bundle sem items".into()))
}

pub fn validate_items(value: &Value) -> AppResult<&Value> {
    if value.is_null() {
        return Err(AppError::BadRequest("items ausente".into()));
    }
    Ok(value)
}

pub fn expect_array<'a>(value: &'a Value, key: &str) -> AppResult<&'a [Value]> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| AppError::BadRequest(format!("items.{key} deve ser uma lista")))
}

pub fn expect_str<'a>(value: &'a Value, key: &str) -> AppResult<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::BadRequest(format!("campo '{key}' é obrigatório")))
}

#[derive(Debug, Deserialize, Default)]
pub struct BundleQuery {
    pub workspace_id: Option<String>,
}

async fn workspace(state: &AppState, requested: Option<&str>) -> AppResult<String> {
    crate::api::tasks::resolve_workspace_id(state, requested).await
}

pub async fn export_packs(
    State(state): State<Arc<AppState>>,
    Query(query): Query<BundleQuery>,
) -> AppResult<Json<Value>> {
    let (_, packs) = starter_packs::list(&state, query.workspace_id.as_deref()).await?;
    let mut transfers = Vec::new();
    for pack in packs {
        transfers.push(starter_packs::export_pack(&pack.id)?);
    }
    Ok(Json(envelope(
        "packs_bundle",
        query.workspace_id.as_deref(),
        json!({ "packs": transfers }),
    )))
}

pub async fn import_packs(
    State(state): State<Arc<AppState>>,
    Json(body): Json<BundleImportBody>,
) -> AppResult<Json<Value>> {
    let items = validate_envelope(&body.bundle, "packs_bundle")?;
    validate_items(&items)?;
    let transfers = expect_array(&items, "packs")?
        .iter()
        .map(|value| {
            serde_json::from_value::<PackTransfer>(value.clone())
                .map_err(|error| AppError::BadRequest(format!("pacote inválido: {error}")))
        })
        .collect::<AppResult<Vec<_>>>()?;
    if transfers.len() > 200 {
        return Err(AppError::BadRequest(
            "bundle de pacotes excede 200 pacotes".into(),
        ));
    }
    for transfer in &transfers {
        starter_packs::validate_pack(transfer)?;
    }
    let workspace_id = workspace(&state, body.workspace_id.as_deref()).await?;
    let mut imported = Vec::new();
    for transfer in transfers {
        imported.push(starter_packs::import_pack(&state, &workspace_id, transfer).await?);
    }
    Ok(Json(
        json!({ "ok": true, "kind": "packs_bundle", "imported": imported }),
    ))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WorkspaceRecipe {
    id: String,
    yaml: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WorkspaceTransfer {
    workspace: workspaces::Workspace,
    sections: BundleSections,
    #[serde(default)]
    recipes: Vec<WorkspaceRecipe>,
    #[serde(default)]
    schedules: Vec<schedules::Schedule>,
}

pub async fn export_workspaces(State(state): State<Arc<AppState>>) -> AppResult<Json<Value>> {
    let mut items = Vec::new();
    for workspace in workspaces::list(&state.db).await? {
        let sections = config_bundle::export(
            &state.db,
            &workspace.id,
            &config_bundle::ExportRequest {
                workspace_id: None,
                sections: None,
            },
        )
        .await?
        .sections;
        let recipes = recipe_store::list(&state.db, &workspace.id)
            .await?
            .into_iter()
            .map(|recipe| {
                let yaml = std::fs::read_to_string(&recipe.file_path).unwrap_or_default();
                WorkspaceRecipe {
                    id: recipe.id,
                    yaml,
                }
            })
            .filter(|recipe| !recipe.yaml.trim().is_empty())
            .collect::<Vec<_>>();
        let schedules = schedules::list(&state.db, &workspace.id).await?;
        items.push(WorkspaceTransfer {
            workspace,
            sections,
            recipes,
            schedules,
        });
    }
    Ok(Json(envelope(
        "workspaces_bundle",
        None,
        json!({ "workspaces": items }),
    )))
}

fn slug(value: &str) -> String {
    let mut slug = value
        .to_lowercase()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    while slug.contains("--") {
        slug = slug.replace("--", "-");
    }
    let slug = slug.trim_matches('-');
    if slug.is_empty() {
        "workspace".into()
    } else {
        slug.chars().take(48).collect()
    }
}

fn unique_root(base: std::path::PathBuf) -> std::path::PathBuf {
    if !base.exists() {
        return base;
    }
    for attempt in 2..1000 {
        let candidate = base.with_file_name(format!(
            "{}-{attempt}",
            base.file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("workspace")
        ));
        if !candidate.exists() {
            return candidate;
        }
    }
    base
}

fn create_input(
    item: &WorkspaceTransfer,
    root: &std::path::Path,
) -> workspaces::CreateWorkspaceInput {
    workspaces::CreateWorkspaceInput {
        name: item.workspace.name.clone(),
        root_path: root.to_string_lossy().to_string(),
        description: item.workspace.description.clone(),
        color_hex: Some(item.workspace.color_hex.clone()),
        default_router_policy: Some(item.workspace.default_router_policy.clone()),
        default_email_recipient: item.workspace.default_email_recipient.clone(),
        google_drive_folder_id: item.workspace.google_drive_folder_id.clone(),
        timezone: Some(item.workspace.timezone.clone()),
    }
}

fn validate_workspaces(items: &[WorkspaceTransfer]) -> AppResult<()> {
    if items.len() > 200 {
        return Err(AppError::BadRequest(
            "bundle de workspaces excede 200 workspaces".into(),
        ));
    }
    for item in items {
        if item.workspace.name.trim().is_empty() {
            return Err(AppError::BadRequest("workspace sem nome no bundle".into()));
        }
        for recipe in &item.recipes {
            let parsed = crate::recipes::parser::parse(&recipe.yaml)
                .map_err(|error| AppError::BadRequest(format!("receita inválida: {error}")))?;
            if parsed.id != recipe.id {
                return Err(AppError::BadRequest(format!(
                    "receita '{}' contém id '{}'",
                    recipe.id, parsed.id
                )));
            }
        }
        for schedule in &item.schedules {
            if schedule.name.trim().is_empty() {
                return Err(AppError::BadRequest("rotina sem nome no bundle".into()));
            }
        }
    }
    Ok(())
}

pub async fn import_workspaces(
    State(state): State<Arc<AppState>>,
    Json(body): Json<BundleImportBody>,
) -> AppResult<Json<Value>> {
    let items = validate_envelope(&body.bundle, "workspaces_bundle")?;
    validate_items(&items)?;
    let transfers = expect_array(&items, "workspaces")?
        .iter()
        .map(|value| {
            serde_json::from_value::<WorkspaceTransfer>(value.clone())
                .map_err(|error| AppError::BadRequest(format!("workspace inválido: {error}")))
        })
        .collect::<AppResult<Vec<_>>>()?;
    validate_workspaces(&transfers)?;

    let existing = workspaces::list(&state.db).await?;
    let mut imported = Vec::new();
    for item in transfers {
        let target_id = existing
            .iter()
            .find(|workspace| workspace.id == item.workspace.id)
            .or_else(|| {
                existing
                    .iter()
                    .find(|workspace| workspace.name == item.workspace.name)
            })
            .map(|workspace| workspace.id.clone());
        let workspace = match target_id {
            Some(id) => {
                let current = workspaces::get(&state.db, &id).await?;
                if body.mode == SimpleImportMode::Replace {
                    workspaces::update(
                        &state.db,
                        &state.paths,
                        &id,
                        workspaces::UpdateWorkspaceInput {
                            name: Some(item.workspace.name.clone()),
                            root_path: None,
                            description: item.workspace.description.clone(),
                            color_hex: Some(item.workspace.color_hex.clone()),
                            default_router_policy: Some(
                                item.workspace.default_router_policy.clone(),
                            ),
                            default_email_recipient: Some(
                                item.workspace.default_email_recipient.clone(),
                            ),
                            google_drive_folder_id: item.workspace.google_drive_folder_id.clone(),
                            timezone: Some(item.workspace.timezone.clone()),
                        },
                    )
                    .await?
                } else {
                    current
                }
            }
            None => {
                let root = unique_root(
                    opendesk_storage::paths::default_workspaces_root()
                        .join(slug(&item.workspace.name)),
                );
                workspaces::create_trusted(&state.db, &state.paths, create_input(&item, &root))
                    .await?
            }
        };
        let mode = body.mode.into();
        let bundle = ConfigBundle {
            schema_version: config_bundle::SCHEMA_VERSION,
            exported_at: chrono::Utc::now().to_rfc3339(),
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            source_workspace: json!({ "id": workspace.id, "name": workspace.name }),
            sections: item.sections,
        };
        config_bundle::import(
            &state.db,
            &workspace.id,
            &ImportRequest {
                workspace_id: None,
                bundle,
                sections: None,
                mode,
            },
        )
        .await?;
        for recipe in &item.recipes {
            recipe_store::save_yaml(&state.db, &workspace.id, &recipe.id, &recipe.yaml).await?;
        }
        let existing_schedules = schedules::list(&state.db, &workspace.id).await?;
        for schedule in &item.schedules {
            if existing_schedules
                .iter()
                .any(|item| item.name == schedule.name)
            {
                continue;
            }
            schedules::create(
                &state.db,
                schedules::CreateScheduleInput {
                    workspace_id: workspace.id.clone(),
                    name: schedule.name.clone(),
                    description: schedule.description.clone(),
                    cron_expression: schedule.cron_expression.clone(),
                    timezone: schedule.timezone.clone(),
                    agent_name: schedule.agent_name.clone(),
                    command_or_prompt: schedule.command_or_prompt.clone(),
                    trigger_type: schedule.trigger_type.clone(),
                    trigger_config_json: Some(schedule.trigger_config_json.clone()),
                    target_kind: schedule.target_kind.clone(),
                    recipe_id: schedule.recipe_id.clone(),
                    router_policy: schedule.router_policy.clone(),
                    output_folder_template: schedule.output_folder_template.clone(),
                    notify_email_recipients: schedule.notify_email_recipients.clone(),
                    notify_signal: schedule.notify_signal,
                    notify_signal_recipients: schedule.notify_signal_recipients.clone(),
                    notify_telegram: schedule.notify_telegram,
                    notify_telegram_recipients: schedule.notify_telegram_recipients.clone(),
                    notify_only_on_change: schedule.notify_only_on_change,
                    recurrence_json: schedule.recurrence_json.clone(),
                    one_time: schedule.one_time,
                    misfire_policy: schedule.misfire_policy.clone(),
                    safe_readonly_mode: schedule.safe_readonly_mode,
                    sync_to_google_docs: schedule.sync_to_google_docs,
                    sync_to_google_calendar: schedule.sync_to_google_calendar,
                    is_active: false,
                },
            )
            .await?;
        }
        imported.push(json!({
            "id": workspace.id,
            "name": workspace.name,
            "recipes": item.recipes.len(),
            "schedules": item.schedules.len(),
        }));
    }
    Ok(Json(
        json!({ "ok": true, "kind": "workspaces_bundle", "imported": imported }),
    ))
}

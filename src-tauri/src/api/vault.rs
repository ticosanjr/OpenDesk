//! Credential Vault API.
//!
//! Every response is metadata-only. There is intentionally no endpoint that
//! returns a decrypted secret; edge injection happens in Rust through
//! `repos::vault::resolve_for_edge`.

use crate::{
    app_state::AppState,
    db::vault::{self, VaultAclInput, VaultEntryInput, VaultMetadataInput},
    error::{AppError, AppResult},
};
use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

#[derive(Debug, Deserialize, Default)]
pub struct VaultQuery {
    pub workspace_id: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct AuditQuery {
    pub entry_id: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize, Default)]
pub struct AclBody {
    #[serde(default)]
    pub acls: Vec<VaultAclInput>,
}

#[derive(Debug, Deserialize)]
pub struct ImportEnvBody {
    pub content: String,
    #[serde(default)]
    pub workspace_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct BitwardenBody {
    pub session: String,
    #[serde(default)]
    pub item_ids: Option<Vec<String>>,
    #[serde(default)]
    pub workspace_id: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct FillBody {
    #[serde(default)]
    pub task_id: Option<String>,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub timeout_minutes: Option<i64>,
}

pub async fn list(
    State(state): State<Arc<AppState>>,
    Query(query): Query<VaultQuery>,
) -> AppResult<Json<Value>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    Ok(Json(json!({
        "entries": vault::list_entries(&state.db, Some(&workspace_id)).await?,
    })))
}

pub async fn get(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<Value>> {
    let entry = vault::get_metadata(&state.db, &id).await?;
    let acls = vault::list_acl(&state.db, &id).await?;
    let audit = vault::list_audit(&state.db, Some(&id), 50).await?;
    Ok(Json(
        json!({ "entry": entry, "acls": acls, "audit": audit }),
    ))
}

pub async fn create(
    State(state): State<Arc<AppState>>,
    Query(query): Query<VaultQuery>,
    Json(mut input): Json<VaultEntryInput>,
) -> AppResult<Json<Value>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    input.workspace_id.get_or_insert(workspace_id);
    let entry = vault::create_entry(&state.db, input).await?;
    Ok(Json(json!({ "entry": entry })))
}

pub async fn update(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(input): Json<VaultMetadataInput>,
) -> AppResult<Json<Value>> {
    let entry = vault::update_entry(&state.db, &id, input).await?;
    Ok(Json(json!({ "entry": entry })))
}

pub async fn delete(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<Value>> {
    vault::delete_entry(&state.db, &id).await?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn get_acl(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<Value>> {
    Ok(Json(json!({
        "acls": vault::list_acl(&state.db, &id).await?,
    })))
}

pub async fn put_acl(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<AclBody>,
) -> AppResult<Json<Value>> {
    let entry = vault::get_entry(&state.db, &id).await?;
    vault::set_acl(&state.db, &id, &entry, &body.acls).await?;
    Ok(Json(json!({
        "acls": vault::list_acl(&state.db, &id).await?,
    })))
}

pub async fn audit(
    State(state): State<Arc<AppState>>,
    Query(query): Query<AuditQuery>,
) -> AppResult<Json<Value>> {
    let limit = query.limit.unwrap_or(50);
    Ok(Json(json!({
        "audit": vault::list_audit(&state.db, query.entry_id.as_deref(), limit).await?,
    })))
}

pub async fn import_env(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ImportEnvBody>,
) -> AppResult<Json<Value>> {
    let workspace_id = match body.workspace_id {
        Some(workspace_id) => workspace_id,
        None => crate::api::tasks::resolve_workspace_id(&state, None).await?,
    };
    let imported = vault::import_env(&state.db, Some(&workspace_id), &body.content).await?;
    Ok(Json(json!({ "imported": imported })))
}

pub async fn import_bitwarden(
    State(state): State<Arc<AppState>>,
    Json(body): Json<BitwardenBody>,
) -> AppResult<Json<Value>> {
    let workspace_id = match body.workspace_id {
        Some(id) => id,
        None => crate::api::tasks::resolve_workspace_id(&state, None).await?,
    };
    let imported =
        crate::vault_bitwarden::import(&state.db, &workspace_id, &body.session, body.item_ids)
            .await?;
    Ok(Json(json!({ "imported": imported })))
}

/// Creates a HITL interaction asking the user to fill the credential. The
/// secret is never returned to the caller or to the agent.
pub async fn request_fill(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<FillBody>,
) -> AppResult<Json<Value>> {
    let entry = vault::get_metadata(&state.db, &id).await?;
    let task_id = body.task_id.ok_or_else(|| {
        AppError::BadRequest("vault_request_fill exige task_id para abrir o HITL".into())
    })?;
    let question = format!(
        "Preencha a credencial '{}' no site e confirme. O agente não recebe o segredo.",
        entry.label
    );
    let context = format!(
        "vault_entry: {}\ndomínios: {}\n{}",
        entry.id,
        entry.allowed_domains_json,
        body.reason.unwrap_or_default()
    );
    let interaction = crate::api::interactions::ask(
        State(state),
        Path(task_id),
        Json(crate::api::interactions::AskInteractionBody {
            question,
            context: Some(context),
            options: Vec::new(),
            kind: Some("TEXT".into()),
            multiple: false,
            allow_custom: true,
            run_id: None,
            timeout_minutes: body.timeout_minutes,
        }),
    )
    .await?;
    Ok(Json(json!({
        "entry_id": entry.id,
        "interaction": interaction.0,
    })))
}

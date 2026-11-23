use super::*;

pub async fn duplicate(
    State(state): State<Arc<AppState>>,
    Path((kind, id)): Path<(String, String)>,
    Query(query): Query<StudioQuery>,
    Json(body): Json<TargetBody>,
) -> AppResult<Json<Value>> {
    let (paths, scope, _) = context(&state, &query).await?;
    let kind = StudioKind::parse(&kind)?;
    let source = StudioItem::new(&paths, &scope, kind, &id);
    let mut target_id = body
        .new_id
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| format!("{id}-copy"));
    let mut counter = 2;
    while StudioItem::new(&paths, &scope, kind, &target_id).exists()? {
        target_id = format!("{id}-copy-{counter}");
        counter += 1;
        if counter > 100 {
            return Err(AppError::Conflict("could not find a free copy id".into()));
        }
    }
    let target = StudioItem::new(&paths, &scope, kind, &target_id);
    source.copy_to(&target, false, &state.paths.backups_dir)?;
    Ok(Json(json!({
        "ok": true,
        "kind": kind.as_str(),
        "id": target_id,
        "scope": scope
    })))
}

pub async fn copy(
    State(state): State<Arc<AppState>>,
    Path((kind, id)): Path<(String, String)>,
    Query(query): Query<StudioQuery>,
    Json(body): Json<TargetBody>,
) -> AppResult<Json<Value>> {
    let (paths, scope, _) = context(&state, &query).await?;
    let target = target(&state, &body, &scope).await?;
    let kind = StudioKind::parse(&kind)?;
    let target_id = body
        .new_id
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| id.clone());
    let source = StudioItem::new(&paths, &scope, kind, &id);
    let destination = StudioItem::new(&target.paths, &target.scope, kind, &target_id);
    source.copy_to(&destination, body.overwrite, &state.paths.backups_dir)?;
    Ok(Json(json!({
        "ok": true,
        "kind": kind.as_str(),
        "id": target_id,
        "scope": target.scope
    })))
}

pub async fn move_item(
    State(state): State<Arc<AppState>>,
    Path((kind, id)): Path<(String, String)>,
    Query(query): Query<StudioQuery>,
    Json(body): Json<TargetBody>,
) -> AppResult<Json<Value>> {
    let (paths, scope, _) = context(&state, &query).await?;
    let target = target(&state, &body, &scope).await?;
    let kind = StudioKind::parse(&kind)?;
    let target_id = body
        .new_id
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| id.clone());
    if target.scope == scope && target_id == id {
        return Err(AppError::BadRequest(
            "source and target are the same item".into(),
        ));
    }
    let source = StudioItem::new(&paths, &scope, kind, &id);
    let destination = StudioItem::new(&target.paths, &target.scope, kind, &target_id);
    source.copy_to(&destination, body.overwrite, &state.paths.backups_dir)?;
    if let Err(error) = source.delete(&state.paths.backups_dir) {
        let _ = destination.delete(&state.paths.backups_dir);
        return Err(error.into());
    }
    Ok(Json(json!({
        "ok": true,
        "kind": kind.as_str(),
        "id": target_id,
        "scope": target.scope,
    })))
}

pub async fn list_backups(
    State(state): State<Arc<AppState>>,
    Path((kind, id)): Path<(String, String)>,
    Query(query): Query<StudioQuery>,
) -> AppResult<Json<Value>> {
    let (paths, scope, _) = context(&state, &query).await?;
    let kind = StudioKind::parse(&kind)?;
    let path = StudioItem::new(&paths, &scope, kind, &id).path()?;
    let entries = crate::config_sync::file_ops::list_backups_for(&path, &state.paths.backups_dir)?;
    Ok(Json(json!({ "path": path, "backups": entries })))
}

#[derive(Debug, Deserialize)]
pub struct RestoreBody {
    pub backup_id: String,
}

pub async fn restore_backup(
    State(state): State<Arc<AppState>>,
    Path((kind, id)): Path<(String, String)>,
    Query(query): Query<StudioQuery>,
    Json(body): Json<RestoreBody>,
) -> AppResult<Json<Value>> {
    let (paths, scope, _) = context(&state, &query).await?;
    let kind = StudioKind::parse(&kind)?;
    let path = StudioItem::new(&paths, &scope, kind, &id).path()?;
    crate::config_sync::file_ops::restore_backup(&body.backup_id, &path, &state.paths.backups_dir)?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn remove_item(
    State(state): State<Arc<AppState>>,
    Path((kind, id)): Path<(String, String)>,
    Query(query): Query<StudioQuery>,
) -> AppResult<Json<Value>> {
    let (paths, scope, _) = context(&state, &query).await?;
    let kind = StudioKind::parse(&kind)?;
    StudioItem::new(&paths, &scope, kind, &id).delete(&state.paths.backups_dir)?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn toggle(
    State(state): State<Arc<AppState>>,
    Path((kind, id)): Path<(String, String)>,
    Query(query): Query<StudioQuery>,
    Json(body): Json<ToggleBody>,
) -> AppResult<Json<Value>> {
    let (paths, scope, _) = context(&state, &query).await?;
    let id = require_id(&StudioQuery {
        id: Some(id),
        ..query.clone()
    })?;
    let kind = StudioKind::parse(&kind)?;
    StudioItem::new(&paths, &scope, kind, &id)
        .set_enabled(body.enabled, &state.paths.backups_dir)?;
    Ok(Json(json!({
        "ok": true,
        "kind": kind.as_str(),
        "id": id,
        "enabled": body.enabled
    })))
}

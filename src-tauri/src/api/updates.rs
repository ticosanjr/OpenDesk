use crate::{
    app_state::AppState,
    error::AppResult,
    updates::{self, DownloadResult, UpdateInfo},
};
use axum::{Json, extract::State};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct TokenBody {
    pub token: String,
}

pub async fn status(State(state): State<Arc<AppState>>) -> Json<UpdateInfo> {
    Json(updates::check(&state).await)
}

pub async fn check(State(state): State<Arc<AppState>>) -> Json<UpdateInfo> {
    Json(updates::check(&state).await)
}

pub async fn put_token(
    State(state): State<Arc<AppState>>,
    Json(body): Json<TokenBody>,
) -> AppResult<Json<UpdateInfo>> {
    updates::save_token(&state, &body.token)?;
    Ok(Json(updates::check(&state).await))
}

pub async fn delete_token(State(state): State<Arc<AppState>>) -> AppResult<Json<UpdateInfo>> {
    updates::save_token(&state, "")?;
    Ok(Json(updates::check(&state).await))
}

pub async fn download(State(state): State<Arc<AppState>>) -> AppResult<Json<DownloadResult>> {
    Ok(Json(updates::download_and_stage(&state).await?))
}

pub async fn rollback(State(state): State<Arc<AppState>>) -> AppResult<Json<DownloadResult>> {
    use crate::error::AppError;
    let target = updates::running_image().ok_or_else(|| {
        AppError::Internal("could not resolve the running OpenDesk binary".into())
    })?;
    let backup = target.with_file_name(format!(
        "{}.bak",
        target
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "opendesk".into())
    ));
    if !backup.is_file() {
        return Err(AppError::BadRequest(format!(
            "não há backup de versão anterior em {}",
            backup.display()
        )));
    }
    let staging = state.paths.data_dir.join("updates").join("rollback");
    std::fs::create_dir_all(&staging)?;
    let staged = staging.join(
        target
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "opendesk".into()),
    );
    std::fs::copy(&backup, &staged)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&staged, std::fs::Permissions::from_mode(0o755))?;
    }
    updates::spawn_replacer(&staged, &target, std::process::id())?;
    Ok(Json(DownloadResult {
        ok: true,
        action: "restart-required".into(),
        path: Some(target.to_string_lossy().to_string()),
        message: format!(
            "Rollback preparado a partir de {}. Encerre o app pela bandeja; ele será restaurado e reaberto.",
            backup.display()
        ),
    }))
}

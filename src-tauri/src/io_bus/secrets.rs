use super::secrets_guard;
use crate::{
    error::{AppError, AppResult},
    paths::AppPaths,
    secrets::SecretStore,
};
use uuid::Uuid;

pub const WEBHOOK_SECRET_NAMESPACE: &str = "io_bus";

/// Resolves a required secret reference. Inline values are never accepted here;
/// legacy configs are migrated once at startup by `secrets_guard`.
pub fn secret(paths: &AppPaths, reference: Option<&str>) -> AppResult<String> {
    secrets_guard::resolve_secret_ref(paths, reference, "io_bus")
}

/// Moves a plaintext webhook secret into the encrypted store and guarantees a
/// `secret_ref` in the config. Every `WEBHOOK_HTTP` source must carry one, or a
/// request could create tasks and start runs without authentication.
///
/// The plain value is removed from the config so it never reaches the database,
/// backups or logs.
pub fn ensure_webhook_secret(
    paths: &AppPaths,
    config: &mut serde_json::Value,
) -> AppResult<String> {
    let reference = format!("webhook-{}", Uuid::now_v7());
    ensure_webhook_secret_with(paths, config, &reference)
}

/// Deterministic variant used by routines and recipes so saving the same form
/// twice overwrites the same stored secret instead of leaving orphans.
pub fn ensure_webhook_secret_with(
    paths: &AppPaths,
    config: &mut serde_json::Value,
    fallback_reference: &str,
) -> AppResult<String> {
    let Some(object) = config.as_object_mut() else {
        return Err(AppError::BadRequest(
            "config do webhook deve ser um objeto JSON".into(),
        ));
    };
    if let Some(reference) = object
        .get("secret_ref")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Ok(reference.to_string());
    }
    let plain = object
        .get("secret")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            AppError::BadRequest(
                "webhook HTTP exige segredo; defina o segredo do webhook na rotina ou receita"
                    .into(),
            )
        })?;
    SecretStore::new(paths).set(WEBHOOK_SECRET_NAMESPACE, fallback_reference, &plain)?;
    object.remove("secret");
    object.insert(
        "secret_ref".into(),
        serde_json::Value::String(fallback_reference.to_string()),
    );
    Ok(fallback_reference.to_string())
}

/// Removes a secret that is no longer referenced by any source. Best effort.
pub fn drop_webhook_secret(paths: &AppPaths, reference: &str) {
    if reference.trim().is_empty() {
        return;
    }
    if let Err(error) = SecretStore::new(paths).delete(WEBHOOK_SECRET_NAMESPACE, reference.trim()) {
        tracing::warn!(error = %error, "could not delete webhook secret");
    }
}

pub fn webhook_secret_ref(config_json: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(config_json)
        .ok()
        .and_then(|config| {
            config
                .get("secret_ref")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        })
}

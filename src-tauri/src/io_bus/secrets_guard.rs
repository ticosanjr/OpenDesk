//! Fail-closed credential rules for the I/O bus.
//!
//! Adapters never accept a secret inline. Configuration JSON may only carry a
//! reference (`token_ref`, `password_ref`, `database_url_ref`, …) that is
//! resolved through the encrypted `SecretStore`. Legacy plaintext is migrated
//! once at startup; anything that cannot be migrated is rejected instead of
//! silently running with a weaker configuration.

use crate::{
    error::{AppError, AppResult},
    paths::AppPaths,
    secrets::SecretStore,
};
use serde_json::Value;
use sqlx::SqlitePool;

/// Keys that must never appear as non-empty strings in an I/O-bus config.
pub const FORBIDDEN_PLAINTEXT_SECRET_KEYS: &[&str] = &[
    "token",
    "access_token",
    "password",
    "secret",
    "api_key",
    "database_url",
    "private_key",
];

fn is_forbidden(key: &str) -> bool {
    FORBIDDEN_PLAINTEXT_SECRET_KEYS
        .iter()
        .any(|candidate| candidate.eq_ignore_ascii_case(key))
}

fn non_empty_string(value: &Value) -> Option<&str> {
    value
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

/// Walks objects and arrays recursively. `null` and empty strings are allowed
/// so older forms that always emitted the field keep parsing, but any non-empty
/// value under a forbidden key is a hard error.
pub fn validate_no_plaintext_secrets(value: &Value) -> AppResult<()> {
    match value {
        Value::Object(map) => {
            for (key, item) in map {
                if is_forbidden(key) && non_empty_string(item).is_some() {
                    return Err(AppError::BadRequest(format!(
                        "segredo em texto plano no campo '{key}'; use '{key}_ref' e o SecretStore"
                    )));
                }
                validate_no_plaintext_secrets(item)?;
            }
        }
        Value::Array(items) => {
            for item in items {
                validate_no_plaintext_secrets(item)?;
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
    Ok(())
}

/// Removes `user:password@` from connection URLs before they reach a log, an
/// audit row or an API error. The scheme and host are preserved.
pub fn redact_connection_url(raw: &str) -> String {
    let Some(scheme_end) = raw.find("://") else {
        return raw.to_string();
    };
    let rest = &raw[scheme_end + 3..];
    let authority_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..authority_end];
    let Some(at) = authority.rfind('@') else {
        return raw.to_string();
    };
    format!("{}://***@{}", &raw[..scheme_end], &rest[at + 1..])
}

/// Resolves a required reference through the encrypted store. Missing
/// references and missing entries are distinct, actionable errors.
pub fn resolve_secret_ref(
    paths: &AppPaths,
    reference: Option<&str>,
    label: &str,
) -> AppResult<String> {
    let reference = reference
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::BadRequest(format!("{label}: informe uma referência de segredo"))
        })?;
    SecretStore::new(paths)
        .get("io_bus", reference)?
        .ok_or_else(|| AppError::NotFound(format!("{label}: segredo '{reference}' não encontrado")))
}

/// Migrates every inbound/outbound config once. Failures are logged per row:
/// the row stays untouched and the runtime rejects it instead of running with
/// inline credentials.
pub async fn migrate_legacy_secrets(pool: &SqlitePool, paths: &AppPaths) -> AppResult<usize> {
    let store = SecretStore::new(paths);
    let mut migrated = 0;

    for source in crate::db::io_bus::all_inbound(pool).await? {
        match migrate_config(&store, &source.config_json, "inbound", &source.id) {
            Ok(Some(updated)) => {
                crate::db::io_bus::set_inbound_config(pool, &source.id, &updated.to_string())
                    .await?;
                migrated += 1;
            }
            Ok(None) => {}
            Err(error) => tracing::warn!(
                source = %source.id,
                error = %error,
                "could not migrate legacy inbound secret; source will fail closed"
            ),
        }
    }

    for sink in crate::db::io_bus::all_outbound(pool).await? {
        match migrate_config(&store, &sink.config_json, "outbound", &sink.id) {
            Ok(Some(updated)) => {
                crate::db::io_bus::set_outbound_config(pool, &sink.id, &updated.to_string())
                    .await?;
                migrated += 1;
            }
            Ok(None) => {}
            Err(error) => tracing::warn!(
                sink = %sink.id,
                error = %error,
                "could not migrate legacy outbound secret; sink will fail closed"
            ),
        }
    }

    Ok(migrated)
}

/// Returns the rewritten JSON when at least one secret moved to the store.
fn migrate_config(
    store: &SecretStore,
    config_json: &str,
    kind: &str,
    id: &str,
) -> AppResult<Option<Value>> {
    let mut value: Value = serde_json::from_str(config_json)
        .map_err(|error| AppError::BadRequest(format!("config_json inválido: {error}")))?;
    let changed = migrate_value(store, &mut value, &format!("{kind}-{id}"), "")?;
    if !changed {
        return Ok(None);
    }
    validate_no_plaintext_secrets(&value)?;
    Ok(Some(value))
}

fn migrate_value(
    store: &SecretStore,
    value: &mut Value,
    context: &str,
    path: &str,
) -> AppResult<bool> {
    match value {
        Value::Object(map) => {
            let mut changed = false;
            let keys = map.keys().cloned().collect::<Vec<_>>();
            for key in keys {
                if is_forbidden(&key) {
                    let Some(plain) = map.get(&key).cloned() else {
                        continue;
                    };
                    match plain {
                        Value::Null => {
                            map.remove(&key);
                            changed = true;
                        }
                        Value::String(value) if value.trim().is_empty() => {
                            map.remove(&key);
                            changed = true;
                        }
                        Value::String(value) => {
                            let reference_key = format!("{key}_ref");
                            let already_set = map
                                .get(&reference_key)
                                .and_then(Value::as_str)
                                .is_some_and(|value| !value.trim().is_empty());
                            if !already_set {
                                let reference = format!("legacy-{context}-{path}{key}")
                                    .replace([':', '/'], "-");
                                store.set("io_bus", &reference, value.trim())?;
                                map.insert(reference_key, Value::String(reference));
                            }
                            map.remove(&key);
                            changed = true;
                        }
                        _ => {
                            return Err(AppError::BadRequest(format!(
                                "campo '{key}' deve ser string para migrar ao SecretStore"
                            )));
                        }
                    }
                } else if let Some(child) = map.get_mut(&key) {
                    changed |= migrate_value(store, child, context, &format!("{path}{key}-"))?;
                }
            }
            Ok(changed)
        }
        Value::Array(items) => {
            let mut changed = false;
            for (index, item) in items.iter_mut().enumerate() {
                changed |= migrate_value(store, item, context, &format!("{path}{index}-"))?;
            }
            Ok(changed)
        }
        _ => Ok(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn rejects_plaintext_anywhere_in_the_tree() {
        let config = json!({
            "nested": [{"token": "abc"}],
            "safe_ref": "token_ref"
        });
        let error = validate_no_plaintext_secrets(&config).expect_err("plaintext");
        assert!(error.to_string().contains("token"));
    }

    #[test]
    fn allows_empty_values_and_references() {
        validate_no_plaintext_secrets(&json!({
            "token": "",
            "password": null,
            "token_ref": "opendesk-token",
            "nested": [{"database_url": ""}]
        }))
        .expect("allowed");
    }

    #[test]
    fn redacts_userinfo_from_connection_urls() {
        assert_eq!(
            redact_connection_url("postgres://alice:s3cret@db.local:5432/app?sslmode=require"),
            "postgres://***@db.local:5432/app?sslmode=require"
        );
        assert_eq!(
            redact_connection_url("sqlite:///tmp/open.db"),
            "sqlite:///tmp/open.db"
        );
        assert_eq!(redact_connection_url("not a url"), "not a url");
    }

    #[test]
    fn resolves_required_references_and_reports_missing_ones() {
        let temp = tempfile::TempDir::new().expect("temp");
        let paths = crate::test_support::paths_in(temp.path());
        let error = resolve_secret_ref(&paths, None, "sink").expect_err("no ref");
        assert!(error.to_string().contains("referência"));

        let error = resolve_secret_ref(&paths, Some("nope"), "sink").expect_err("unknown");
        assert!(error.to_string().contains("não encontrado"));

        SecretStore::new(&paths)
            .set("io_bus", "known", "value")
            .expect("set");
        assert_eq!(
            resolve_secret_ref(&paths, Some("known"), "sink").expect("resolve"),
            "value"
        );
    }

    #[test]
    fn migrates_known_plaintext_to_references() {
        let temp = tempfile::TempDir::new().expect("temp");
        let paths = crate::test_support::paths_in(temp.path());
        let store = SecretStore::new(&paths);
        let updated = migrate_config(
            &store,
            r#"{"token":"abc","nested":{"database_url":"postgres://u:p@h/db"}}"#,
            "inbound",
            "src-1",
        )
        .expect("migrate")
        .expect("changed");

        assert!(updated.get("token").is_none());
        assert!(updated["token_ref"].is_string());
        assert!(updated["nested"].get("database_url").is_none());
        assert!(updated["nested"]["database_url_ref"].is_string());
        let token_ref = updated["token_ref"].as_str().expect("token ref");
        assert_eq!(
            store.get("io_bus", token_ref).expect("get"),
            Some("abc".into())
        );
        validate_no_plaintext_secrets(&updated).expect("clean");
    }

    #[tokio::test]
    async fn migrates_persisted_rows_and_is_idempotent() {
        let ctx = crate::test_support::context().await;
        let source = crate::db::io_bus::create_inbound(
            &ctx.state.db,
            &ctx.workspace_id,
            crate::db::io_bus::InboundInput {
                name: "Legacy SQL".into(),
                source_type: "SQL_POLLER".into(),
                config: json!({
                    "database_url": "postgres://alice:secret@db.local/app",
                    "query": "SELECT 1"
                }),
                target_agent: None,
                target_column: Some("TODO".into()),
                auto_execute: false,
                is_enabled: false,
            },
        )
        .await
        .expect("source");
        let sink = crate::db::io_bus::create_outbound(
            &ctx.state.db,
            &ctx.workspace_id,
            crate::db::io_bus::OutboundInput {
                name: "Legacy insert".into(),
                sink_type: "SQL_INSERT".into(),
                config: json!({
                    "database_url": "postgres://alice:secret@db.local/app",
                    "query": "INSERT INTO x VALUES (?)"
                }),
                is_enabled: false,
            },
        )
        .await
        .expect("sink");

        let migrated = migrate_legacy_secrets(&ctx.state.db, &ctx.state.paths)
            .await
            .expect("migrate");
        assert_eq!(migrated, 2);

        let source = crate::db::io_bus::get_inbound(&ctx.state.db, &source.id)
            .await
            .expect("source");
        let config: Value = serde_json::from_str(&source.config_json).expect("json");
        assert!(config.get("database_url").is_none());
        let reference = config["database_url_ref"].as_str().expect("ref");
        assert_eq!(
            ctx.state.secrets.get("io_bus", reference).expect("get"),
            Some("postgres://alice:secret@db.local/app".into())
        );

        let sink = crate::db::io_bus::get_outbound(&ctx.state.db, &sink.id)
            .await
            .expect("sink");
        let config: Value = serde_json::from_str(&sink.config_json).expect("json");
        assert!(config.get("database_url").is_none());
        assert!(config["database_url_ref"].is_string());

        let again = migrate_legacy_secrets(&ctx.state.db, &ctx.state.paths)
            .await
            .expect("second migrate");
        assert_eq!(again, 0);
    }

    #[test]
    fn keeps_an_existing_reference_and_drops_the_plaintext_copy() {
        let temp = tempfile::TempDir::new().expect("temp");
        let paths = crate::test_support::paths_in(temp.path());
        let store = SecretStore::new(&paths);
        let updated = migrate_config(
            &store,
            r#"{"token":"legacy","token_ref":"already-there"}"#,
            "inbound",
            "src-2",
        )
        .expect("migrate")
        .expect("changed");
        assert_eq!(updated["token_ref"], "already-there");
        assert!(updated.get("token").is_none());
    }
}

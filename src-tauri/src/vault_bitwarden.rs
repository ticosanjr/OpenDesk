//! One-way import from the Bitwarden CLI into the Credential Vault.
//!
//! `BW_SESSION` is passed only through the child environment, never stored or
//! logged. Imported entries keep the original URIs as allowed domains and start
//! with HITL enabled; the operator must add ACLs before injection.

use crate::{
    db::vault::{self, VaultEntryInput, VaultEntryMetadata},
    error::{AppError, AppResult},
};
use serde_json::Value;
use sqlx::SqlitePool;
use std::{process::Stdio, time::Duration};
use tokio::process::Command;

const MAX_ITEMS: usize = 500;

pub async fn import(
    pool: &SqlitePool,
    workspace_id: &str,
    session: &str,
    item_ids: Option<Vec<String>>,
) -> AppResult<Vec<VaultEntryMetadata>> {
    let bw = crate::io_bus::tools::command("bw");
    import_with_bw(pool, workspace_id, session, item_ids, &bw).await
}

pub async fn import_with_bw(
    pool: &SqlitePool,
    workspace_id: &str,
    session: &str,
    item_ids: Option<Vec<String>>,
    bw: &std::path::Path,
) -> AppResult<Vec<VaultEntryMetadata>> {
    let session = session.trim();
    if session.is_empty() {
        return Err(AppError::BadRequest("BW_SESSION é obrigatório".into()));
    }
    let status = run_bw(bw, &["status"], session).await?;
    let status: Value = serde_json::from_str(&status)
        .map_err(|error| AppError::Internal(format!("bw status inválido: {error}")))?;
    match status.get("status").and_then(Value::as_str) {
        Some("unlocked") => {}
        Some("locked") => {
            return Err(AppError::BadRequest(
                "Bitwarden está bloqueado; rode `bw unlock` e use a sessão resultante".into(),
            ));
        }
        other => {
            return Err(AppError::BadRequest(format!(
                "Bitwarden não está desbloqueado (status {}); rode `bw login`/`bw unlock`",
                other.unwrap_or("desconhecido")
            )));
        }
    }
    let raw = run_bw(bw, &["list", "items"], session).await?;
    let items: Vec<Value> = serde_json::from_str(&raw)
        .map_err(|error| AppError::Internal(format!("bw list inválido: {error}")))?;
    let wanted = item_ids.filter(|ids| !ids.is_empty());
    let mut imported = Vec::new();
    for item in items.into_iter().take(MAX_ITEMS) {
        if let Some(ids) = &wanted
            && !ids
                .iter()
                .any(|id| item.get("id").and_then(Value::as_str) == Some(id))
        {
            continue;
        }
        let Some(name) = item.get("name").and_then(Value::as_str) else {
            continue;
        };
        let id = slug(
            name,
            item.get("id").and_then(Value::as_str).unwrap_or("item"),
        );
        if vault::get_metadata(pool, &id).await.is_ok() {
            continue;
        }
        let username = item.pointer("/login/username").and_then(Value::as_str);
        let password = item.pointer("/login/password").and_then(Value::as_str);
        let totp = item.pointer("/login/totp").and_then(Value::as_str);
        let notes = item.get("notes").and_then(Value::as_str);
        let (kind, secret) = match (password, totp, notes) {
            (Some(password), _, _) if !password.trim().is_empty() => ("login", password),
            (None, Some(seed), _) if !seed.trim().is_empty() => ("totp_seed", seed),
            (None, None, Some(notes)) if !notes.trim().is_empty() => ("api_key", notes),
            _ => continue,
        };
        let mut domains = Vec::new();
        if let Some(uris) = item.pointer("/login/uris").and_then(Value::as_array) {
            for uri in uris {
                let raw = uri
                    .get("uri")
                    .and_then(Value::as_str)
                    .or_else(|| uri.as_str());
                if let Some(raw) = raw
                    && let Ok(url) = reqwest::Url::parse(raw)
                    && let Some(host) = url.host_str()
                    && let Some(domain) = vault::normalize_domain(host)
                    && !domains.contains(&domain)
                {
                    domains.push(domain);
                }
            }
        }
        if kind == "login" && domains.is_empty() {
            continue;
        }
        let entry = vault::create_entry(
            pool,
            VaultEntryInput {
                id,
                workspace_id: Some(workspace_id.to_string()),
                kind: kind.into(),
                label: name.trim().to_string(),
                username: username.map(str::to_string),
                allowed_domains: domains,
                secret: secret.to_string(),
                requires_hitl: true,
                expires_at: None,
                acls: Vec::new(),
            },
        )
        .await?;
        imported.push(entry);
    }
    Ok(imported)
}

async fn run_bw(bw: &std::path::Path, args: &[&str], session: &str) -> AppResult<String> {
    let output = tokio::time::timeout(
        Duration::from_secs(30),
        Command::new(bw)
            .args(args)
            .env("BW_SESSION", session)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .output(),
    )
    .await
    .map_err(|_| AppError::Internal("bw não respondeu em 30s".into()))?
    .map_err(|error| {
        AppError::Internal(format!(
            "não foi possível executar bw: {error}; instale Bitwarden CLI ou configure o caminho"
        ))
    })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr)
            .replace(session, "[REDACTED]")
            .chars()
            .take(300)
            .collect::<String>();
        return Err(AppError::BadRequest(format!(
            "bw falhou: {}",
            stderr.trim()
        )));
    }
    String::from_utf8(output.stdout)
        .map_err(|error| AppError::Internal(format!("bw devolveu bytes inválidos: {error}")))
}

fn slug(name: &str, fallback: &str) -> String {
    let value = name
        .trim()
        .to_lowercase()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.') {
                character
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    let value = value.chars().take(100).collect::<String>();
    if value.is_empty() {
        fallback.to_string()
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_is_safe_and_stable() {
        assert_eq!(slug("Stripe Live", "id"), "stripe-live");
        assert_eq!(slug("  ", "fallback"), "fallback");
        assert_eq!(slug("A/B", "id"), "a-b");
    }

    #[tokio::test]
    async fn imports_items_with_a_fake_bw_script() {
        use std::os::unix::fs::PermissionsExt;
        let temp = tempfile::TempDir::new().expect("temp");
        let script = temp.path().join("bw");
        std::fs::write(
            &script,
            r#"#!/bin/sh
case "$1" in
  status) echo '{"status":"unlocked"}' ;;
  list) echo '[{"id":"i1","name":"Stripe Live","login":{"username":"ops","password":"super-secret","uris":[{"uri":"https://api.example.com"}]},"notes":""}]' ;;
esac
"#,
        )
        .expect("script");
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).expect("mode");
        let ctx = crate::test_support::context().await;
        let imported = import_with_bw(&ctx.state.db, &ctx.workspace_id, "session", None, &script)
            .await
            .expect("import");
        assert_eq!(imported.len(), 1);
        assert_eq!(imported[0].kind, "login");
        assert!(imported[0].allowed_domains_json.contains("api.example.com"));
        let again = import_with_bw(&ctx.state.db, &ctx.workspace_id, "session", None, &script)
            .await
            .expect("second import");
        assert!(again.is_empty());
    }

    #[tokio::test]
    async fn rejects_locked_bitwarden_sessions() {
        use std::os::unix::fs::PermissionsExt;
        let temp = tempfile::TempDir::new().expect("temp");
        let script = temp.path().join("bw");
        std::fs::write(&script, "#!/bin/sh\necho '{\"status\":\"locked\"}'\n").expect("script");
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).expect("mode");
        let ctx = crate::test_support::context().await;
        let error = import_with_bw(&ctx.state.db, &ctx.workspace_id, "session", None, &script)
            .await
            .expect_err("locked");
        assert!(error.to_string().contains("bloqueado"));
    }

    #[tokio::test]
    async fn reports_missing_binary_and_empty_session() {
        let ctx = crate::test_support::context().await;
        let error = import_with_bw(
            &ctx.state.db,
            &ctx.workspace_id,
            "",
            None,
            std::path::Path::new("definitely-missing-bw"),
        )
        .await
        .expect_err("empty session");
        assert!(error.to_string().contains("BW_SESSION"));
        let error = import_with_bw(
            &ctx.state.db,
            &ctx.workspace_id,
            "session",
            None,
            std::path::Path::new("/definitely/missing/bw"),
        )
        .await
        .expect_err("missing binary");
        assert!(error.to_string().contains("bw"));
    }

    #[tokio::test]
    async fn imports_only_selected_item_ids() {
        use std::os::unix::fs::PermissionsExt;
        let temp = tempfile::TempDir::new().expect("temp");
        let script = temp.path().join("bw");
        std::fs::write(
            &script,
            r#"#!/bin/sh
case "$1" in
  status) echo '{"status":"unlocked"}' ;;
  list) echo '[{"id":"i1","name":"One","login":{"password":"secret-one","uris":[{"uri":"https://one.example.com"}]}},{"id":"i2","name":"Two","login":{"password":"secret-two","uris":[{"uri":"https://two.example.com"}]}}]' ;;
esac
"#,
        )
        .expect("script");
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).expect("mode");
        let ctx = crate::test_support::context().await;
        let imported = import_with_bw(
            &ctx.state.db,
            &ctx.workspace_id,
            "session",
            Some(vec!["i2".into()]),
            &script,
        )
        .await
        .expect("import");
        assert_eq!(imported.len(), 1);
        assert_eq!(imported[0].label, "Two");
    }
}

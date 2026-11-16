//! Passkey (WebAuthn) registration and login for the remote PWA.
//!
//! Single-user product: every credential belongs to the local operator. The
//! relying party is derived from the request origin (or overridden with
//! `OPENDESK_WEBAUTHN_ORIGIN`/`OPENDESK_WEBAUTHN_RP_ID`) and ceremonies are
//! persisted server-side, single-use, with a ten-minute TTL.

use crate::{
    db::auth_sessions,
    error::{AppError, AppResult},
};
use axum::http::HeaderMap;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;
use webauthn_rs::prelude::{
    Passkey, PasskeyAuthentication, PasskeyRegistration, PublicKeyCredential,
    RegisterPublicKeyCredential, Url, Webauthn, WebauthnBuilder,
};

const CEREMONY_TTL_SECONDS: i64 = 600;
const RP_NAME: &str = "OpenDesk";

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct PasskeyRecord {
    pub id: String,
    pub credential_id: String,
    pub label: String,
    #[serde(skip_serializing)]
    pub passkey_json: String,
    pub created_at: String,
    pub last_used_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PasskeyStatus {
    pub registered: i64,
    pub enabled: bool,
    /// Origin the server expects; surfaced so the UI can explain mismatches.
    pub origin: Option<String>,
}

pub fn origin_from_headers(headers: &HeaderMap) -> AppResult<(String, Url)> {
    let configured_origin = std::env::var("OPENDESK_WEBAUTHN_ORIGIN").ok();
    let configured_rp = std::env::var("OPENDESK_WEBAUTHN_RP_ID").ok();
    let raw = if let Some(origin) = configured_origin {
        origin
    } else if let Some(origin) = headers.get("origin").and_then(|value| value.to_str().ok()) {
        origin.to_string()
    } else {
        let host = headers
            .get("host")
            .and_then(|value| value.to_str().ok())
            .unwrap_or("localhost");
        let proto = headers
            .get("x-forwarded-proto")
            .and_then(|value| value.to_str().ok())
            .unwrap_or("http");
        format!("{proto}://{host}")
    };
    let url = Url::parse(raw.trim())
        .map_err(|error| AppError::BadRequest(format!("origem inválida: {error}")))?;
    let host = url
        .host_str()
        .ok_or_else(|| AppError::BadRequest("origem sem host".into()))?
        .to_string();
    let loopback = matches!(host.as_str(), "localhost" | "127.0.0.1" | "::1");
    if url.scheme() != "https" && !loopback {
        return Err(AppError::BadRequest(
            "passkey exige HTTPS fora de localhost; use um domínio com TLS ou reverse proxy".into(),
        ));
    }
    if host.parse::<std::net::IpAddr>().is_ok() {
        return Err(AppError::BadRequest(
            "WebAuthn não aceita endereço IP; use localhost ou um domínio".into(),
        ));
    }
    let rp_id = configured_rp.unwrap_or(host);
    Ok((rp_id, url))
}

fn instance(rp_id: &str, origin: &Url) -> AppResult<Webauthn> {
    WebauthnBuilder::new(rp_id, origin)
        .and_then(|builder| builder.rp_name(RP_NAME).allow_any_port(true).build())
        .map_err(|error| AppError::BadRequest(format!("WebAuthn inválido: {error}")))
}

fn user_id() -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_OID, b"opendesk-local-operator")
}

pub async fn status(pool: &SqlitePool, origin: Option<&str>) -> AppResult<PasskeyStatus> {
    let registered: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM passkey_credentials")
        .fetch_one(pool)
        .await?;
    Ok(PasskeyStatus {
        registered,
        enabled: registered > 0,
        origin: origin.map(str::to_string),
    })
}

pub async fn list(pool: &SqlitePool) -> AppResult<Vec<PasskeyRecord>> {
    Ok(sqlx::query_as::<_, PasskeyRecord>(
        "SELECT id, credential_id, label, passkey_json, created_at, last_used_at
         FROM passkey_credentials ORDER BY created_at DESC",
    )
    .fetch_all(pool)
    .await?)
}

async fn load_passkeys(pool: &SqlitePool) -> AppResult<Vec<(PasskeyRecord, Passkey)>> {
    let rows = sqlx::query_as::<_, PasskeyRecord>(
        "SELECT id, credential_id, label, passkey_json, created_at, last_used_at
         FROM passkey_credentials ORDER BY created_at DESC",
    )
    .fetch_all(pool)
    .await?;
    let mut passkeys = Vec::new();
    for row in rows {
        let passkey: Passkey = serde_json::from_str(&row.passkey_json)
            .map_err(|error| AppError::Internal(format!("passkey corrompida: {error}")))?;
        passkeys.push((row, passkey));
    }
    Ok(passkeys)
}

async fn store_ceremony(
    pool: &SqlitePool,
    kind: &str,
    state: &impl Serialize,
) -> AppResult<String> {
    let id = Uuid::now_v7().to_string();
    let state_json =
        serde_json::to_string(state).map_err(|error| AppError::Internal(error.to_string()))?;
    sqlx::query(
        "INSERT INTO passkey_ceremonies (id, kind, state_json, expires_at)
         VALUES (?, ?, ?, strftime('%Y-%m-%dT%H:%M:%fZ','now', ?))",
    )
    .bind(&id)
    .bind(kind)
    .bind(state_json)
    .bind(format!("+{CEREMONY_TTL_SECONDS} seconds"))
    .execute(pool)
    .await?;
    Ok(id)
}

async fn take_ceremony(pool: &SqlitePool, id: &str, kind: &str) -> AppResult<String> {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT state_json FROM passkey_ceremonies
         WHERE id = ? AND kind = ? AND expires_at > strftime('%Y-%m-%dT%H:%M:%fZ','now')",
    )
    .bind(id)
    .bind(kind)
    .fetch_optional(pool)
    .await?;
    let _ = sqlx::query("DELETE FROM passkey_ceremonies WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await;
    row.map(|(state,)| state)
        .ok_or_else(|| AppError::BadRequest("cerimônia expirada ou inválida".into()))
}

#[derive(Debug, Serialize)]
pub struct RegistrationStart {
    pub ceremony_id: String,
    pub options: serde_json::Value,
}

pub async fn start_registration(
    pool: &SqlitePool,
    headers: &HeaderMap,
) -> AppResult<RegistrationStart> {
    let (rp_id, origin) = origin_from_headers(headers)?;
    let webauthn = instance(&rp_id, &origin)?;
    let existing = load_passkeys(pool).await?;
    let exclude = existing
        .iter()
        .map(|(_, passkey)| passkey.cred_id().clone())
        .collect::<Vec<_>>();
    let (options, state) = webauthn
        .start_passkey_registration(
            user_id(),
            "opendesk",
            "Operador OpenDesk",
            (!exclude.is_empty()).then_some(exclude),
        )
        .map_err(|error| AppError::Internal(error.to_string()))?;
    let ceremony_id = store_ceremony(pool, "register", &state).await?;
    Ok(RegistrationStart {
        ceremony_id,
        options: serde_json::to_value(options)
            .map_err(|error| AppError::Internal(error.to_string()))?,
    })
}

#[derive(Debug, Deserialize)]
pub struct FinishRegistration {
    pub ceremony_id: String,
    pub credential: serde_json::Value,
    #[serde(default)]
    pub label: Option<String>,
}

pub async fn finish_registration(
    pool: &SqlitePool,
    headers: &HeaderMap,
    input: FinishRegistration,
) -> AppResult<PasskeyRecord> {
    let (rp_id, origin) = origin_from_headers(headers)?;
    let webauthn = instance(&rp_id, &origin)?;
    let state_json = take_ceremony(pool, &input.ceremony_id, "register").await?;
    let state: PasskeyRegistration =
        serde_json::from_str(&state_json).map_err(|error| AppError::Internal(error.to_string()))?;
    let credential: RegisterPublicKeyCredential = serde_json::from_value(input.credential.clone())
        .map_err(|error| AppError::BadRequest(format!("credencial inválida: {error}")))?;
    let passkey = webauthn
        .finish_passkey_registration(&credential, &state)
        .map_err(|error| AppError::BadRequest(format!("registro recusado: {error}")))?;
    let credential_id = URL_SAFE_NO_PAD.encode(passkey.cred_id());
    let id = Uuid::now_v7().to_string();
    let label = input
        .label
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("passkey");
    let passkey_json =
        serde_json::to_string(&passkey).map_err(|error| AppError::Internal(error.to_string()))?;
    sqlx::query(
        "INSERT INTO passkey_credentials (id, credential_id, label, passkey_json)
         VALUES (?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&credential_id)
    .bind(label)
    .bind(passkey_json)
    .execute(pool)
    .await?;
    sqlx::query_as::<_, PasskeyRecord>(
        "SELECT id, credential_id, label, passkey_json, created_at, last_used_at
         FROM passkey_credentials WHERE id = ?",
    )
    .bind(&id)
    .fetch_one(pool)
    .await
    .map_err(AppError::from)
}

#[derive(Debug, Serialize)]
pub struct LoginStart {
    pub ceremony_id: String,
    pub options: serde_json::Value,
}

pub async fn start_login(pool: &SqlitePool, headers: &HeaderMap) -> AppResult<LoginStart> {
    let (rp_id, origin) = origin_from_headers(headers)?;
    let webauthn = instance(&rp_id, &origin)?;
    let passkeys = load_passkeys(pool)
        .await?
        .into_iter()
        .map(|(_, passkey)| passkey)
        .collect::<Vec<_>>();
    if passkeys.is_empty() {
        return Err(AppError::NotFound(
            "nenhuma passkey cadastrada; entre com o token e cadastre uma".into(),
        ));
    }
    let (options, state) = webauthn
        .start_passkey_authentication(&passkeys)
        .map_err(|error| AppError::Internal(error.to_string()))?;
    let ceremony_id = store_ceremony(pool, "login", &state).await?;
    Ok(LoginStart {
        ceremony_id,
        options: serde_json::to_value(options)
            .map_err(|error| AppError::Internal(error.to_string()))?,
    })
}

#[derive(Debug, Deserialize)]
pub struct FinishLogin {
    pub ceremony_id: String,
    pub credential: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct LoginResult {
    pub token: String,
    pub session: auth_sessions::AuthSession,
}

pub async fn finish_login(
    pool: &SqlitePool,
    headers: &HeaderMap,
    input: FinishLogin,
) -> AppResult<LoginResult> {
    let (rp_id, origin) = origin_from_headers(headers)?;
    let webauthn = instance(&rp_id, &origin)?;
    let state_json = take_ceremony(pool, &input.ceremony_id, "login").await?;
    let state: PasskeyAuthentication =
        serde_json::from_str(&state_json).map_err(|error| AppError::Internal(error.to_string()))?;
    let credential: PublicKeyCredential = serde_json::from_value(input.credential.clone())
        .map_err(|error| AppError::BadRequest(format!("credencial inválida: {error}")))?;
    let result = webauthn
        .finish_passkey_authentication(&credential, &state)
        .map_err(|error| AppError::BadRequest(format!("passkey recusada: {error}")))?;
    if !result.user_verified() {
        return Err(AppError::BadRequest(
            "a passkey não confirmou a presença/verificação do usuário".into(),
        ));
    }
    let credential_id = URL_SAFE_NO_PAD.encode(result.cred_id());
    let row: Option<PasskeyRecord> = sqlx::query_as(
        "SELECT id, credential_id, label, passkey_json, created_at, last_used_at
         FROM passkey_credentials WHERE credential_id = ?",
    )
    .bind(&credential_id)
    .fetch_optional(pool)
    .await?;
    let Some(row) = row else {
        return Err(AppError::BadRequest("passkey desconhecida".into()));
    };
    if result.needs_update() {
        let mut passkey: Passkey = serde_json::from_str(&row.passkey_json)
            .map_err(|error| AppError::Internal(error.to_string()))?;
        passkey.update_credential(&result);
        let encoded = serde_json::to_string(&passkey)
            .map_err(|error| AppError::Internal(error.to_string()))?;
        sqlx::query("UPDATE passkey_credentials SET passkey_json = ? WHERE id = ?")
            .bind(encoded)
            .bind(&row.id)
            .execute(pool)
            .await?;
    }
    sqlx::query(
        "UPDATE passkey_credentials SET last_used_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE id = ?",
    )
    .bind(&row.id)
    .execute(pool)
    .await?;
    let (token, session) = auth_sessions::create(pool, "passkey", 30).await?;
    Ok(LoginResult { token, session })
}

pub async fn delete(pool: &SqlitePool, id: &str) -> AppResult<bool> {
    let result = sqlx::query("DELETE FROM passkey_credentials WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn delete_expired_ceremonies(pool: &SqlitePool) -> AppResult<u64> {
    let result = sqlx::query(
        "DELETE FROM passkey_ceremonies
         WHERE expires_at <= strftime('%Y-%m-%dT%H:%M:%fZ','now')",
    )
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    fn headers(origin: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert("origin", HeaderValue::from_str(origin).expect("origin"));
        headers
    }

    #[test]
    fn accepts_localhost_http_but_rejects_ip_and_plain_http() {
        let (rp, url) = origin_from_headers(&headers("http://localhost:1420")).expect("local");
        assert_eq!(rp, "localhost");
        assert_eq!(url.port(), Some(1420));

        assert!(origin_from_headers(&headers("http://127.0.0.1:1420")).is_err());
        assert!(origin_from_headers(&headers("http://example.com")).is_err());
        assert!(origin_from_headers(&headers("https://example.com")).is_ok());
    }

    #[tokio::test]
    async fn ceremonies_are_single_use_and_expire() {
        let ctx = crate::test_support::context().await;
        let state = serde_json::json!({ "challenge": "x" });
        let id = store_ceremony(&ctx.state.db, "login", &state)
            .await
            .expect("store");
        assert!(
            take_ceremony(&ctx.state.db, &id, "login")
                .await
                .expect("take")
                .contains("challenge")
        );
        assert!(take_ceremony(&ctx.state.db, &id, "login").await.is_err());

        let id = store_ceremony(&ctx.state.db, "login", &state)
            .await
            .expect("store");
        sqlx::query(
            "UPDATE passkey_ceremonies SET expires_at = '2000-01-01T00:00:00.000Z' WHERE id = ?",
        )
        .bind(&id)
        .execute(&ctx.state.db)
        .await
        .expect("expire");
        assert!(take_ceremony(&ctx.state.db, &id, "login").await.is_err());
    }

    #[tokio::test]
    async fn status_counts_credentials() {
        let ctx = crate::test_support::context().await;
        assert_eq!(
            status(&ctx.state.db, None)
                .await
                .expect("status")
                .registered,
            0
        );
    }
}

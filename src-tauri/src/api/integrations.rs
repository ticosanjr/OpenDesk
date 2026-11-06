use crate::{
    app_state::AppState,
    db::{
        deliveries,
        integrations::{self, SmtpSettingsInput},
    },
    error::{AppError, AppResult},
    integrations::smtp_mailer,
};
use axum::{
    Json,
    extract::{Query, State},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Deserialize, Default)]
pub struct IntegrationQuery {
    pub workspace_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SmtpSettingsDto {
    pub workspace_id: String,
    pub host: Option<String>,
    pub port: i64,
    pub username: Option<String>,
    pub from_email: Option<String>,
    pub use_tls: bool,
    pub password_set: bool,
    pub configured: bool,
}

#[derive(Debug, Deserialize)]
pub struct SmtpSaveInput {
    pub workspace_id: String,
    pub host: String,
    #[serde(default = "default_port")]
    pub port: i64,
    pub username: Option<String>,
    pub from_email: String,
    #[serde(default = "default_tls")]
    pub use_tls: bool,
    #[serde(default)]
    pub password: Option<String>,
}

fn default_port() -> i64 {
    587
}

fn default_tls() -> bool {
    true
}

#[derive(Debug, Deserialize)]
pub struct SmtpTestInput {
    pub workspace_id: String,
    pub to: Option<String>,
}

pub async fn get_smtp(
    State(state): State<Arc<AppState>>,
    Query(query): Query<IntegrationQuery>,
) -> AppResult<Json<SmtpSettingsDto>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    let settings = integrations::get(&state.db, &workspace_id).await?;
    let password_set = state.secrets.get("smtp", &workspace_id)?.is_some();

    Ok(Json(match settings {
        Some(settings) => {
            let configured = settings
                .smtp_host
                .as_deref()
                .map(|value| !value.trim().is_empty())
                .unwrap_or(false)
                && settings
                    .smtp_from_email
                    .as_deref()
                    .map(|value| !value.trim().is_empty())
                    .unwrap_or(false);
            SmtpSettingsDto {
                workspace_id,
                host: settings.smtp_host,
                port: settings.smtp_port,
                username: settings.smtp_username,
                from_email: settings.smtp_from_email,
                use_tls: settings.smtp_use_tls,
                password_set,
                configured,
            }
        }
        None => SmtpSettingsDto {
            workspace_id,
            host: None,
            port: 587,
            username: None,
            from_email: None,
            use_tls: true,
            password_set,
            configured: false,
        },
    }))
}

pub async fn put_smtp(
    State(state): State<Arc<AppState>>,
    Json(input): Json<SmtpSaveInput>,
) -> AppResult<Json<SmtpSettingsDto>> {
    if input.host.trim().is_empty() {
        return Err(AppError::BadRequest("SMTP host is required".into()));
    }
    if input.from_email.trim().is_empty() || !input.from_email.contains('@') {
        return Err(AppError::BadRequest(
            "valid SMTP from_email is required".into(),
        ));
    }

    let secret_ref = if let Some(password) = input.password.as_deref() {
        if password.is_empty() {
            state.secrets.delete("smtp", &input.workspace_id)?;
            None
        } else {
            state.secrets.set("smtp", &input.workspace_id, password)?;
            Some("smtp".to_string())
        }
    } else {
        None
    };

    integrations::upsert_smtp(
        &state.db,
        &input.workspace_id,
        &SmtpSettingsInput {
            host: input.host,
            port: input.port,
            username: input.username,
            from_email: input.from_email,
            use_tls: input.use_tls,
        },
        secret_ref.as_deref(),
    )
    .await?;

    get_smtp(
        State(state),
        Query(IntegrationQuery {
            workspace_id: Some(input.workspace_id),
        }),
    )
    .await
}

pub async fn test_smtp(
    State(state): State<Arc<AppState>>,
    Json(input): Json<SmtpTestInput>,
) -> AppResult<Json<serde_json::Value>> {
    let Some(config) =
        smtp_mailer::load_config(&state.db, &state.secrets, &input.workspace_id).await?
    else {
        return Err(AppError::BadRequest(
            "SMTP is not configured for this workspace".into(),
        ));
    };

    smtp_mailer::test_connection(&config).await?;

    if let Some(to) = input.to.as_deref().filter(|to| to.contains('@')) {
        smtp_mailer::send_email(
            &config,
            to,
            "[OpenDesk] SMTP test",
            "<p>Se você recebeu este e-mail, o SMTP do OpenDesk está configurado.</p>".into(),
            Vec::new(),
        )
        .await
        .map_err(|error| AppError::Internal(format!("SMTP test email failed: {error:#}")))?;
    }

    Ok(Json(serde_json::json!({ "ok": true })))
}

#[derive(Debug, Deserialize, Default)]
pub struct DeliveryQuery {
    pub workspace_id: Option<String>,
}

pub async fn list_deliveries(
    State(state): State<Arc<AppState>>,
    Query(query): Query<DeliveryQuery>,
) -> AppResult<Json<Vec<deliveries::Delivery>>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    Ok(Json(deliveries::list(&state.db, &workspace_id, 100).await?))
}

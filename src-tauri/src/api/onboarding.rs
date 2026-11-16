use crate::{app_state::AppState, db, error::AppResult};
use axum::{Json, extract::State};
use serde::Serialize;
use serde_json::{Value, json};
use std::{path::Path, sync::Arc};

#[derive(Debug, Clone, Serialize)]
pub struct OnboardingStep {
    pub id: String,
    pub label: String,
    pub detail: String,
    pub status: String,
    pub href: String,
    pub required: bool,
}

pub async fn status(State(state): State<Arc<AppState>>) -> AppResult<Json<Value>> {
    let workspaces = db::workspaces::list(&state.db).await.unwrap_or_default();
    let active = db::settings::active_workspace_id(&state.db)
        .await
        .ok()
        .flatten()
        .and_then(|id| {
            workspaces
                .iter()
                .find(|workspace| workspace.id == id)
                .cloned()
        })
        .or_else(|| workspaces.first().cloned());

    let mut steps = Vec::new();

    steps.push(step(
        "workspace",
        "Workspace ativo",
        active
            .as_ref()
            .map(|workspace| format!("{} · {}", workspace.name, workspace.root_path))
            .unwrap_or_else(|| "Nenhum workspace cadastrado".into()),
        active.is_some(),
        "/kanban",
        true,
    ));

    let daemon = state.daemon.status().await;
    let binary = state.locator.resolve().ok();
    let engine_ok = daemon.online || binary.is_some();
    steps.push(step(
        "engine",
        "Motor OpenCode",
        if daemon.online {
            format!("daemon online na porta {}", daemon.port)
        } else if let Some(binary) = binary {
            format!("binário detectado: {}", binary.path)
        } else {
            "Nenhum binário ou daemon detectado".into()
        },
        engine_ok,
        "/settings?section=engine",
        true,
    ));

    let (mcp_ok, base_pack_ok) = match active.as_ref() {
        Some(workspace) => {
            let root = Path::new(&workspace.root_path);
            let config = std::fs::read_to_string(root.join("opencode.jsonc")).unwrap_or_default();
            let skill = root
                .join(".opencode/skills/operacao-opendesk/SKILL.md")
                .is_file();
            (config.contains("\"opendesk-control\""), skill)
        }
        None => (false, false),
    };
    steps.push(step(
        "mcp-control",
        "MCP opendesk-control",
        if mcp_ok {
            "Registrado no opencode.jsonc".into()
        } else {
            "Adicione o MCP de controle ao workspace".into()
        },
        mcp_ok,
        "/studio",
        true,
    ));
    steps.push(step(
        "base-pack",
        "Pacote base",
        if base_pack_ok {
            "Skill operacao-opendesk instalada".into()
        } else {
            "Instale o pacote controle-opendesk em Pacotes".into()
        },
        base_pack_ok,
        "/packs",
        true,
    ));

    if let Some(workspace) = active.as_ref() {
        let google = db::integrations::get(&state.db, &workspace.id)
            .await
            .ok()
            .flatten()
            .filter(|integration| {
                integration.google_scopes.is_some() && integration.google_account_email.is_some()
            });
        let google_email = google
            .as_ref()
            .and_then(|integration| integration.google_account_email.clone())
            .unwrap_or_else(|| "Nenhuma conta conectada".into());
        steps.push(step(
            "google",
            "Google Workspace",
            google_email,
            google.is_some(),
            "/integrations?section=google",
            false,
        ));
    }

    let telegram = crate::integrations::telegram::load(&state.db)
        .await
        .ok()
        .unwrap_or_default();
    let telegram_token = crate::integrations::telegram::token(&state.secrets).is_some();
    steps.push(step(
        "telegram",
        "Telegram",
        if telegram_token && telegram.enabled {
            "Bot habilitado".into()
        } else {
            "Conecte um bot para chat operacional e HITL".into()
        },
        telegram_token && telegram.enabled,
        "/integrations?section=telegram",
        false,
    ));

    let signal = crate::integrations::signal::load(&state.db).await.ok();
    let signal_ok = signal
        .as_ref()
        .is_some_and(|config| !config.account.trim().is_empty());
    steps.push(step(
        "signal",
        "Signal",
        signal
            .map(|config| config.account)
            .filter(|account| !account.trim().is_empty())
            .unwrap_or_else(|| "Nenhuma conta configurada".into()),
        signal_ok,
        "/integrations?section=signal",
        false,
    ));

    let whatsapp: Option<crate::api::channels::WhatsAppConfig> =
        db::settings::get(&state.db, "channels.whatsapp")
            .await
            .ok()
            .flatten()
            .and_then(|value| serde_json::from_value(value).ok());
    let whatsapp_ok = whatsapp.as_ref().is_some_and(|config| config.enabled);
    steps.push(step(
        "whatsapp",
        "WhatsApp",
        whatsapp
            .map(|config| format!("provider: {}", config.provider))
            .unwrap_or_else(|| "Nenhum canal configurado".into()),
        whatsapp_ok,
        "/integrations?section=whatsapp",
        false,
    ));

    let passkeys: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM passkey_credentials")
        .fetch_one(&state.db)
        .await
        .unwrap_or(0);
    steps.push(step(
        "passkey",
        "Acesso remoto (passkey)",
        if passkeys > 0 {
            format!("{passkeys} passkey(s) registrada(s)")
        } else {
            "Opcional para o PWA; registre em Configurações".into()
        },
        passkeys > 0,
        "/settings?section=security",
        false,
    ));

    let desktop = crate::desktop_integration::status(&state.paths);
    steps.push(step(
        "desktop",
        "Ícone do desktop",
        if !desktop.supported {
            "Não se aplica nesta plataforma".into()
        } else if desktop.installed {
            "Atalho instalado".into()
        } else {
            "Atalho do AppImage ainda não instalado".into()
        },
        !desktop.supported || desktop.installed,
        "/settings?section=about",
        false,
    ));

    let required_done = steps
        .iter()
        .filter(|step| step.required && step.status == "done")
        .count();
    let required_total = steps.iter().filter(|step| step.required).count();
    let optional_done = steps
        .iter()
        .filter(|step| !step.required && step.status == "done")
        .count();
    let optional_total = steps.iter().filter(|step| !step.required).count();

    Ok(Json(json!({
        "steps": steps,
        "required_done": required_done,
        "required_total": required_total,
        "optional_done": optional_done,
        "optional_total": optional_total,
        "complete": required_done == required_total,
    })))
}

fn step(
    id: &str,
    label: &str,
    detail: String,
    done: bool,
    href: &str,
    required: bool,
) -> OnboardingStep {
    OnboardingStep {
        id: id.into(),
        label: label.into(),
        detail,
        status: if done { "done" } else { "todo" }.into(),
        href: href.into(),
        required,
    }
}

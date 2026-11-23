use crate::{
    app_state::AppState, db::schedules, error::AppResult, integrations::smtp_mailer,
    queue_scheduler::cron_engine, secrets::SecretStore,
};
use axum::{
    Json,
    extract::{Path, State},
};
use chrono::Utc;
use std::sync::Arc;

#[derive(Debug, serde::Serialize)]
pub struct DryRunCheck {
    pub name: String,
    pub ok: bool,
    pub detail: String,
}

pub async fn dry_run(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    let schedule = schedules::get(&state.db, &id).await?;
    let workspace = crate::db::workspaces::get(&state.db, &schedule.workspace_id).await?;
    let mut checks = Vec::new();
    let mut ok = true;

    if schedule.target_kind == "RECIPE" {
        let recipe = match schedule.recipe_id.as_deref() {
            Some(recipe_id) => {
                crate::recipes::store::yaml_for(&state.db, &schedule.workspace_id, recipe_id)
                    .await
                    .ok()
                    .and_then(|yaml| crate::recipes::parser::parse(&yaml).ok())
                    .is_some()
            }
            None => false,
        };
        checks.push(DryRunCheck {
            name: "Receita".into(),
            ok: recipe,
            detail: if recipe {
                format!(
                    "receita {} válida",
                    schedule.recipe_id.clone().unwrap_or_default()
                )
            } else {
                "receita ausente ou inválida".into()
            },
        });
        ok &= recipe;
    } else {
        let prompt_ok = !schedule.command_or_prompt.trim().is_empty();
        checks.push(DryRunCheck {
            name: "Prompt".into(),
            ok: prompt_ok,
            detail: if prompt_ok {
                "comando/prompt presente".into()
            } else {
                "comando/prompt vazio".into()
            },
        });
        ok &= prompt_ok;
    }

    if schedule.trigger_type == "CRON" {
        let next =
            cron_engine::next_occurrence(&schedule.cron_expression, &schedule.timezone, Utc::now());
        match &next {
            Ok(value) => checks.push(DryRunCheck {
                name: "Agenda".into(),
                ok: true,
                detail: format!("próximo disparo em {}", value.to_rfc3339()),
            }),
            Err(error) => {
                ok = false;
                checks.push(DryRunCheck {
                    name: "Agenda".into(),
                    ok: false,
                    detail: format!("{error}"),
                });
            }
        }
    } else {
        checks.push(DryRunCheck {
            name: "Gatilho".into(),
            ok: true,
            detail: format!("acionado por {}", schedule.trigger_type),
        });
    }

    let folder = cron_engine::render_output_folder(
        &schedule,
        std::path::Path::new(&workspace.root_path),
        Utc::now(),
    );
    match &folder {
        Ok(path) => {
            let result = std::fs::create_dir_all(path);
            let folder_ok = result.is_ok();
            let detail = result
                .err()
                .map(|error| format!("{error}"))
                .unwrap_or_else(|| path.display().to_string());
            checks.push(DryRunCheck {
                name: "Pasta de saída".into(),
                ok: folder_ok,
                detail,
            });
            ok &= folder_ok;
        }
        Err(error) => {
            ok = false;
            checks.push(DryRunCheck {
                name: "Pasta de saída".into(),
                ok: false,
                detail: format!("{error}"),
            });
        }
    }

    let engine_online = state.daemon.status().await.online;
    checks.push(DryRunCheck {
        name: "Engine".into(),
        ok: engine_online,
        detail: if engine_online {
            "OpenCode online".into()
        } else {
            "OpenCode offline".into()
        },
    });
    ok &= engine_online;

    let secrets = SecretStore::new(&state.paths);
    let smtp = smtp_mailer::load_config(&state.db, &secrets, &schedule.workspace_id).await?;
    checks.push(DryRunCheck {
        name: "SMTP".into(),
        ok: smtp.is_some(),
        detail: if smtp.is_some() {
            "configurado".into()
        } else {
            "não configurado".into()
        },
    });

    let google = state.google.status(&schedule.workspace_id).await?;
    checks.push(DryRunCheck {
        name: "Google".into(),
        ok: google.connected,
        detail: google.email.unwrap_or_else(|| "não conectado".into()),
    });

    Ok(Json(serde_json::json!({
        "ok": ok,
        "checks": checks,
    })))
}

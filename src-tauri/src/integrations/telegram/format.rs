use super::{
    load, resolve_recipients, send_text_with_markup, short_id, token, token_for, truncate,
};
use crate::{
    app_state::AppState,
    db::{interactions::TaskInteraction, tasks::Task},
    error::AppResult,
    hitl::HitlKind,
    secrets::SecretStore,
};
use serde_json::{Value, json};
use std::sync::Arc;

pub(super) fn interaction_options(interaction: &TaskInteraction) -> Vec<String> {
    serde_json::from_str(&interaction.options_json).unwrap_or_default()
}

#[cfg(test)]
pub(super) fn parse_selected_options(
    answer: &str,
    options: &[String],
    multiple: bool,
) -> Vec<String> {
    crate::api::interactions::parse_option_indices(answer, options, multiple)
}

/// Sends a task interaction to every allowlist chat that is already known.
pub async fn notify_task_interaction(
    state: &Arc<AppState>,
    interaction: &TaskInteraction,
    task: &Task,
) -> AppResult<usize> {
    let config = load(&state.db).await?;
    if !config.enabled {
        return Ok(0);
    }
    let token = token_for(state).await?;
    let targets = resolve_recipients(&state.db, &config.allowlist).await;
    if targets.is_empty() {
        return Ok(0);
    }
    let text = format_task_interaction(interaction, task);
    let markup = interaction_markup(interaction);
    for target in &targets {
        send_text_with_markup(&token, target, &text, None, markup.as_ref()).await?;
    }
    Ok(targets.len())
}

fn format_task_interaction(interaction: &TaskInteraction, task: &Task) -> String {
    let options = interaction_options(interaction);
    let kind = HitlKind::effective(&interaction.kind, &options, interaction.multiple);
    let short = short_id(&interaction.id);
    let mut message = format!(
        "[OpenDesk] Decisão necessária\nTarefa: {}\nStatus: {}\nTipo: {}\n\nPergunta: {}\n\nCódigo: #{short}",
        task.title,
        task.status,
        kind.as_str(),
        interaction.question
    );
    if let Some(context) = interaction
        .context_markdown
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        message.push_str("\n\nContexto:\n");
        message.push_str(&truncate(context, 1800));
    }
    match kind {
        HitlKind::Approval => {
            message.push_str(&format!(
                "\n\nAprove ou negue pelos botões, ou responda `#{short} aprovar <payload>` / `#{short} negar <payload>`."
            ));
        }
        HitlKind::SingleChoice | HitlKind::MultiChoice => {
            message.push_str("\n\nOpções:");
            for (index, option) in options.iter().enumerate() {
                message.push_str(&format!("\n{}. {}", index + 1, option));
            }
            if kind == HitlKind::MultiChoice {
                message.push_str(&format!(
                    "\n\nResponda com `#{short} 1,2` (separe as opções por vírgula) ou `#{short} negar`."
                ));
            } else {
                message.push_str(&format!(
                    "\n\nResponda com os botões ou `#{short} <número>`; use `#{short} negar` para recusar."
                ));
            }
            message.push_str(&format!("\nPayload opcional: `#{short} <número> <texto>`."));
        }
        HitlKind::Text => {
            message.push_str(&format!(
                "\n\nResponda com `#{short} <sua resposta>` ou `#{short} negar` para encerrar."
            ));
        }
    }
    message
}

pub(super) fn interaction_markup(interaction: &TaskInteraction) -> Option<Value> {
    let options = interaction_options(interaction);
    let kind = HitlKind::effective(&interaction.kind, &options, interaction.multiple);
    if kind == HitlKind::Approval {
        return Some(json!({
            "inline_keyboard": [[
                { "text": "Aprovar", "callback_data": format!("ti:{}:approve", interaction.id) },
                { "text": "Negar", "callback_data": format!("ti:{}:deny", interaction.id) },
            ]]
        }));
    }
    if interaction.multiple {
        return None;
    }
    if options.is_empty() {
        return None;
    }
    let mut rows = options
        .iter()
        .enumerate()
        .map(|(index, option)| {
            json!([{
                "text": truncate(&option.replace(['\n', '\r'], " "), 48),
                "callback_data": format!("ti:{}:{index}", interaction.id),
            }])
        })
        .collect::<Vec<_>>();
    rows.push(json!([{
        "text": "Negar",
        "callback_data": format!("ti:{}:deny", interaction.id),
    }]));
    Some(json!({ "inline_keyboard": rows }))
}

/// Permission prompts can be approved or rejected directly from Telegram.
pub async fn notify_permission(
    pool: &sqlx::SqlitePool,
    secrets: &SecretStore,
    action: &str,
    resources: &[String],
    request_id: &str,
) -> AppResult<usize> {
    let config = load(pool).await?;
    if !config.enabled {
        return Ok(0);
    }
    let Some(token) = token(secrets) else {
        return Ok(0);
    };
    let targets = resolve_recipients(pool, &config.allowlist).await;
    if targets.is_empty() {
        return Ok(0);
    }
    let short = short_id(request_id);
    let text = format!(
        "OpenDesk aguardando decisão\nAção: {action}\nRecursos: {}\n\nAprove ou negue pelos botões, ou responda `APROVAR {short}` / `NEGAR {short}`.",
        resources.join(", ")
    );
    let markup = json!({
        "inline_keyboard": [[
            { "text": "Aprovar", "callback_data": format!("pm:{request_id}:once") },
            { "text": "Negar", "callback_data": format!("pm:{request_id}:reject") },
        ]]
    });
    for target in &targets {
        send_text_with_markup(&token, target, &text, None, Some(&markup)).await?;
    }
    Ok(targets.len())
}

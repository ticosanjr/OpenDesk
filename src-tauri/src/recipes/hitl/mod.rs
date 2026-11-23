//! Shell implementation of the recipe HITL channel port.
//!
//! The protocol, storage and resolution live in `opendesk_recipes::hitl`.
//! This module only adapts the concrete channels (email, Signal and Telegram)
//! owned by the desktop shell.

pub mod discord;
pub mod email;
pub mod signal;
pub mod slack;
pub mod telegram;

pub use opendesk_recipes::hitl::*;

use crate::{
    app_state::AppState,
    error::AppError,
    recipe_context::{HitlChannelDispatcher, HitlChannelRequest, RecipeContext},
};
use async_trait::async_trait;
use std::sync::Arc;

pub struct ShellHitlChannels {
    pub state: Arc<AppState>,
}

#[async_trait]
impl HitlChannelDispatcher for ShellHitlChannels {
    async fn dispatch(
        &self,
        context: &RecipeContext,
        channel: &str,
        request: &HitlChannelRequest,
    ) -> opendesk_engine::error::AppResult<String> {
        let _ = &self.state;
        match channel {
            "EMAIL" => email::dispatch(context, request).await,
            "SIGNAL" => signal::dispatch(context, request).await,
            "TELEGRAM" => telegram::dispatch(context, request).await,
            "DISCORD" => discord::dispatch(context, request).await,
            "SLACK" => slack::dispatch(context, request).await,
            other => Err(AppError::BadRequest(format!(
                "canal HITL desconhecido: {other}"
            ))),
        }
        .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        recipes::{
            parser,
            store::{self, RecipeRun},
        },
        test_support,
    };
    use serde_json::json;

    async fn fixture() -> (test_support::TestContext, RecipeRun) {
        let ctx = test_support::context().await;
        let yaml = "id: hitl-recipe\nname: Receita HITL\ntrigger:\n  type: MANUAL\n  config: {}\nflow:\n  - type: step\n    id: s1\n    name: Passo\n    prompt: oi\n";
        let recipe = parser::parse(yaml).expect("recipe");
        sqlx::query(
            "INSERT INTO recipes_index (id, workspace_id, name, file_path, trigger_type)
             VALUES ('hitl-recipe', ?, 'Receita HITL', '/tmp/hitl.yaml', 'MANUAL')",
        )
        .bind(&ctx.workspace_id)
        .execute(&ctx.state.db)
        .await
        .expect("index");
        let run = store::create_run(
            &ctx.state.db,
            &recipe,
            &ctx.workspace_id,
            None,
            &json!({}),
            "/tmp/out",
            false,
        )
        .await
        .expect("run");
        (ctx, run)
    }

    #[tokio::test]
    async fn email_and_signal_dispatch_explain_missing_configuration() {
        let (ctx, run) = fixture().await;
        let recipe_context = crate::app_state::recipe_context(&ctx.state);
        let payload = HitlRequestPayload {
            token: "AB12".into(),
            run_id: run.id.clone(),
            workspace_id: ctx.workspace_id.clone(),
            interaction_type: "BINARY_GATE".into(),
            title: "Aprovar".into(),
            message_body: "Seguir?".into(),
            options: vec![],
            attachments: vec![],
        };
        let email = email::dispatch(&recipe_context, &payload)
            .await
            .expect_err("email sem destinatário");
        assert!(format!("{email}").contains("e-mail padrão"));
        let signal = signal::dispatch(&recipe_context, &payload)
            .await
            .expect_err("signal desligado");
        assert!(format!("{signal}").contains("Signal"));
    }
}

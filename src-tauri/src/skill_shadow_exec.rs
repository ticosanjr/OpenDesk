//! Shell model executor for shadow skill replays.
//!
//! The core never calls a model. This adapter builds a read-only prompt from the
//! skill and case, uses OpenCode's stateless generate endpoint (no tools or
//! session history) and returns text plus estimated usage.

use crate::{
    app_state::AppState,
    error::{AppError, AppResult},
    opencode_client::rest_client::ModelRefDto,
    skill_shadow::{ShadowExecutor, ShadowOutput},
};
use async_trait::async_trait;
use std::sync::Arc;

pub struct ShellShadowExecutor {
    pub state: Arc<AppState>,
    pub model: Option<String>,
}

#[async_trait]
impl ShadowExecutor for ShellShadowExecutor {
    async fn execute(
        &self,
        skill: &crate::config_sync::skills::SkillFile,
        case: &crate::skill_cases::SkillEvalCase,
    ) -> AppResult<ShadowOutput> {
        let client = self
            .state
            .daemon
            .client()
            .await
            .ok_or_else(|| AppError::Daemon("OpenCode daemon is not online".into()))?;
        let prompt = format!(
            "Você está executando a skill '{}' em modo somente leitura, sem ferramentas e sem \
             alterar arquivos.\n\nDescrição: {}\n\nInstruções da skill:\n{}\n\nTarefa de avaliação:\n{}",
            skill.name,
            skill.description.as_deref().unwrap_or(""),
            skill.body.trim(),
            case.prompt.trim()
        );
        let model = self.model.as_deref().and_then(ModelRefDto::parse);
        let output = client
            .generate_text(&prompt, model)
            .await
            .map_err(|error| AppError::Daemon(format!("{error:#}")))?;
        let usage = crate::token_estimate::estimate_usage(
            self.model.as_deref().unwrap_or("unknown/model"),
            &prompt,
            &output,
        );
        Ok(ShadowOutput {
            output,
            tokens: usage.input_tokens + usage.output_tokens,
            cost_usd: usage.cost_usd,
        })
    }
}

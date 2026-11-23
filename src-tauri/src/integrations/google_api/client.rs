use crate::integrations::google_oauth::GoogleOAuth;
use anyhow::{Context, Result};
use serde_json::Value;
use std::sync::Arc;

pub struct GoogleApi {
    pub(super) oauth: Arc<GoogleOAuth>,
}

impl GoogleApi {
    pub fn new(oauth: Arc<GoogleOAuth>) -> Self {
        Self { oauth }
    }

    pub(super) async fn token(&self, workspace_id: &str) -> Result<String> {
        Ok(self.oauth.access_token(workspace_id).await?)
    }

    pub(super) async fn post_json(
        &self,
        workspace_id: &str,
        url: &str,
        body: Value,
    ) -> Result<Value> {
        let token = self.token(workspace_id).await?;
        let response = self
            .oauth
            .http()
            .post(url)
            .bearer_auth(token)
            .json(&body)
            .send()
            .await?;
        parse_json(response).await
    }
}

pub(super) async fn parse_json(response: reqwest::Response) -> Result<Value> {
    let status = response.status();
    let text = response.text().await?;
    if !status.is_success() {
        anyhow::bail!("Google API returned {status}: {text}");
    }
    if text.trim().is_empty() {
        Ok(Value::Null)
    } else {
        serde_json::from_str(&text).context("invalid Google API JSON")
    }
}

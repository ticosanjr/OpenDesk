use super::WorkspaceQuery;
use crate::{app_state::AppState, db::channels, error::AppResult};
use axum::{
    Json,
    extract::{Query, State},
};
use opendesk_channels::whatsapp::{self, WEBHOOK_NAMESPACE};
use serde::Serialize;
use std::sync::Arc;

#[derive(Debug, Serialize)]
pub struct ChannelStatus {
    pub channel: String,
    pub routes: usize,
    pub webhook_token_set: bool,
    pub webhook_path: String,
    pub connector: String,
}

pub async fn status(
    State(state): State<Arc<AppState>>,
    Query(query): Query<WorkspaceQuery>,
) -> AppResult<Json<Vec<ChannelStatus>>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    let routes = channels::list(&state.db, &workspace_id).await?;
    let whatsapp_config = whatsapp::load_config(&state.db).await?;
    let whatsapp_connected = whatsapp::connected(&state.secrets, &whatsapp_config)?;
    let list = channels::CHANNELS
        .iter()
        .map(|channel| ChannelStatus {
            channel: (*channel).to_string(),
            routes: routes
                .iter()
                .filter(|route| route.channel == *channel)
                .count(),
            webhook_token_set: state
                .secrets
                .get(WEBHOOK_NAMESPACE, channel)
                .ok()
                .flatten()
                .is_some(),
            webhook_path: format!("/api/webhooks/channels/{channel}"),
            connector: if *channel == "whatsapp" {
                if whatsapp_connected {
                    "cloud-api".into()
                } else if whatsapp_config.provider == "cloud-api" {
                    "cloud-api (incompleto)".into()
                } else {
                    "structure-only".into()
                }
            } else if matches!(*channel, "telegram" | "signal") {
                "native-bridge".into()
            } else {
                "webhook".into()
            },
        })
        .collect();
    Ok(Json(list))
}

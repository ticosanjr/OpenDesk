//! Route definitions for channels, handoffs, memory, MCP hub, sandbox/computer-use, nodes and third-party integrations.

use crate::api::*;
use crate::app_state::AppState;
use axum::Router;
use axum::routing::{delete, get, patch, post, put};
use std::sync::Arc;

pub(super) fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/channels/routes",
            get(channels::list_routes).post(channels::upsert_route),
        )
        .route("/api/channels/routes/{id}", delete(channels::delete_route))
        .route("/api/channels/status", get(channels::status))
        .route(
            "/api/channels/whatsapp",
            get(channels::get_whatsapp).put(channels::put_whatsapp),
        )
        .route(
            "/api/channels/{channel}/webhook-token",
            post(channels::rotate_webhook_token),
        )
        .route("/api/channels/whatsapp/send", post(channels::send_whatsapp))
        .route(
            "/api/channels/discord",
            get(channels::discord::status).put(channels::discord::update),
        )
        .route(
            "/api/channels/discord/start",
            post(channels::discord::start),
        )
        .route("/api/channels/discord/stop", post(channels::discord::stop))
        .route("/api/channels/discord/send", post(channels::discord::send))
        .route(
            "/api/channels/slack",
            get(channels::slack::status).put(channels::slack::update),
        )
        .route("/api/channels/slack/start", post(channels::slack::start))
        .route("/api/channels/slack/stop", post(channels::slack::stop))
        .route("/api/channels/slack/send", post(channels::slack::send))
        .route(
            "/api/channels/whatsapp-qr",
            get(channels::whatsapp_qr::status).put(channels::whatsapp_qr::update),
        )
        .route(
            "/api/channels/whatsapp-qr/start",
            post(channels::whatsapp_qr::start),
        )
        .route(
            "/api/channels/whatsapp-qr/stop",
            post(channels::whatsapp_qr::stop),
        )
        .route(
            "/api/channels/whatsapp-qr/send",
            post(channels::whatsapp_qr::send),
        )
        .route("/api/tickets", get(tickets::list))
        .route(
            "/api/tickets/{id}",
            get(tickets::get).patch(tickets::update),
        )
        .route(
            "/api/webhooks/channels/{channel}",
            get(channels::verify_webhook).post(channels::inbound),
        )
        .route("/api/handoffs", get(handoffs::list).post(handoffs::create))
        .route("/api/handoffs/{id}", get(handoffs::get))
        .route("/api/handoffs/{id}/accept", post(handoffs::accept))
        .route("/api/handoffs/{id}/reject", post(handoffs::reject))
        .route("/api/handoffs/{id}/cancel", post(handoffs::cancel))
        .route("/api/handoffs/{id}/finish", post(handoffs::finish))
        .route(
            "/api/memory/namespaces",
            get(memory::list_namespaces).post(memory::create_namespace),
        )
        .route(
            "/api/memory/namespaces/{id}/acl",
            get(memory::list_acl).put(memory::put_acl),
        )
        .route(
            "/api/memory/namespaces/{id}/acl/{agent}",
            delete(memory::delete_acl),
        )
        .route(
            "/api/memory/notes",
            get(memory::list_notes).post(memory::create_note),
        )
        .route(
            "/api/memory/notes/{id}",
            patch(memory::update_note).delete(memory::delete_note),
        )
        .route("/api/memory/search", get(memory::search))
        .route("/api/memory/context", get(memory::context))
        .route("/api/memory/export", get(memory::export))
        .route(
            "/api/memory/import",
            post(memory::import).layer(axum::extract::DefaultBodyLimit::max(
                crate::config_bundle::MAX_BUNDLE_BYTES,
            )),
        )
        .route("/api/mcp-hub/servers", get(mcp_hub::servers))
        .route("/api/mcp-hub/catalog", get(mcp_hub::catalog))
        .route("/api/mcp-hub/install", post(mcp_hub::install))
        .route("/api/mcp-hub/{scope}/{name}/toggle", post(mcp_hub::toggle))
        .route("/api/mcp-hub/{scope}/{name}/test", post(mcp_hub::test))
        .route("/api/mcp-hub/{scope}/{name}", delete(mcp_hub::delete))
        .route(
            "/api/sandbox",
            get(sandbox::get_sandbox).put(sandbox::put_sandbox),
        )
        .route("/api/computer/status", get(sandbox::computer_status))
        .route(
            "/api/computer/allowlist",
            get(sandbox::get_allowlist).put(sandbox::put_allowlist),
        )
        .route(
            "/api/computer/sessions",
            get(sandbox::list_sessions).post(sandbox::create_session),
        )
        .route(
            "/api/computer/sessions/{id}/approve",
            post(sandbox::approve_session),
        )
        .route(
            "/api/computer/sessions/{id}/end",
            post(sandbox::end_session),
        )
        .route(
            "/api/computer/sessions/{id}/screenshot",
            post(sandbox::screenshot),
        )
        .route("/api/computer/sessions/{id}/action", post(sandbox::action))
        .route(
            "/api/worktrees",
            get(worktrees::list).post(worktrees::prepare),
        )
        .route("/api/worktrees/{id}/diff", get(worktrees::diff))
        .route("/api/worktrees/{id}/merge", post(worktrees::merge))
        .route("/api/worktrees/{id}", delete(worktrees::cleanup))
        .route("/api/voice/status", get(voice::status))
        .route("/api/voice/settings", put(voice::put_settings))
        .route("/api/voice/transcribe", post(voice::transcribe))
        .route("/api/voice/speak", post(voice::speak))
        .route("/api/voice/tts", post(voice::tts))
        .route("/api/voice/stt", post(voice::stt))
        .route(
            "/api/script-tools",
            get(script_tools::list).post(script_tools::create),
        )
        .route("/api/script-tools/export", get(script_tools::export))
        .route(
            "/api/script-tools/import",
            post(script_tools::import).layer(axum::extract::DefaultBodyLimit::max(
                crate::config_bundle::MAX_BUNDLE_BYTES,
            )),
        )
        .route(
            "/api/script-tools/{id}",
            put(script_tools::update).delete(script_tools::delete),
        )
        .route("/api/script-tools/{id}/test", post(script_tools::test))
        .route("/api/nodes", get(nodes::list).post(nodes::create))
        .route(
            "/api/nodes/{id}",
            patch(nodes::update).delete(nodes::delete),
        )
        .route("/api/nodes/{id}/rotate", post(nodes::rotate))
        .route("/api/nodes/heartbeat", post(nodes::heartbeat))
        .route("/api/nodes/next-job", get(nodes::next_job))
        .route("/api/nodes/jobs/{id}/heartbeat", post(nodes::job_heartbeat))
        .route("/api/nodes/jobs/{id}/sync", post(nodes::sync_bundle))
        .route(
            "/api/nodes/jobs/{id}/sync/{bundle_id}",
            get(nodes::download_sync),
        )
        .route("/api/nodes/jobs/{id}/complete", post(nodes::complete_job))
        .route("/api/webhooks/{source_id}", post(io_bus::webhook))
        .route("/api/integrations/overview", get(integrations::overview))
        .route(
            "/api/integrations/smtp",
            get(integrations::get_smtp).put(integrations::put_smtp),
        )
        .route("/api/integrations/smtp/test", post(integrations::test_smtp))
        .route(
            "/api/integrations/smtp/reply",
            post(integrations::send_thread_reply),
        )
        .route(
            "/api/integrations/deliveries",
            get(integrations::list_deliveries),
        )
        .route(
            "/api/integrations/deliveries/{id}/retry",
            post(integrations::retry_delivery),
        )
        .route("/api/integrations/google/status", get(google::status))
        .route(
            "/api/integrations/google/config",
            get(google::get_config).put(google::put_config),
        )
        .route(
            "/api/integrations/google/oauth/start",
            post(google::start_oauth),
        )
        .route(
            "/api/integrations/google/disconnect",
            post(google::disconnect),
        )
        .route(
            "/api/integrations/google/mcp/register",
            post(google::register_mcp),
        )
        .route(
            "/api/integrations/signal",
            get(signal::status).put(signal::put_config),
        )
        .route("/api/integrations/signal/link", post(signal::link))
        .route("/api/integrations/signal/start", post(signal::start))
        .route("/api/integrations/signal/stop", post(signal::stop))
        .route(
            "/api/integrations/telegram",
            get(telegram::status).put(telegram::put_config),
        )
        .route("/api/integrations/telegram/start", post(telegram::start))
        .route("/api/integrations/telegram/stop", post(telegram::stop))
        .route("/api/integrations/telegram/test", post(telegram::test))
        .route("/api/vault", get(vault::list).post(vault::create))
        .route("/api/vault/import-env", post(vault::import_env))
        .route("/api/vault/import-bitwarden", post(vault::import_bitwarden))
        .route("/api/vault/audit", get(vault::audit))
        .route(
            "/api/vault/{id}",
            get(vault::get).patch(vault::update).delete(vault::delete),
        )
        .route(
            "/api/vault/{id}/acl",
            get(vault::get_acl).put(vault::put_acl),
        )
        .route("/api/vault/{id}/request-fill", post(vault::request_fill))
}

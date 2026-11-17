pub mod ab;
pub mod about;
pub mod adapters;
pub mod artifacts;
pub mod attachments;
pub mod attention;
pub mod audit;
pub mod auth;
pub mod backups;
pub mod blueprints;
pub mod browse;
pub mod budget;
pub mod calendar;
pub mod capture;
pub mod channels;
pub mod chat;
pub mod commands;
pub mod core;
pub mod cost;
pub mod dedupe;
pub mod engine;
pub mod files;
pub mod google;
pub mod governance;
pub mod handoffs;
pub mod integrations;
pub mod interactions;
pub mod io_bus;
pub mod learning;
pub mod mcp;
pub mod mcp_hub;
pub mod memory;
pub mod meta;
pub mod metrics;
pub mod missed_runs;
pub mod nodes;
pub mod onboarding;
pub mod passkey;
pub mod permissions;
pub mod personal_context;
pub mod prompt_layer;
pub mod prompt_preview;
pub mod prompt_templates;
pub mod proposals;
pub mod push;
pub mod queue;
pub mod recipes;
pub mod router;
pub mod runs;
pub mod sandbox;
pub mod schedules;
pub mod search;
pub mod seed;
pub mod signal;
pub mod skills;
pub mod starter_packs;
pub mod studio;
pub mod studio_ops;
pub mod sync;
pub mod task_watchers;
pub mod tasks;
pub mod telegram;
pub mod tickets;
pub mod updates;
pub mod voice;
pub mod worktrees;
pub mod ws;

use crate::app_state::AppState;
use axum::{
    Router,
    extract::DefaultBodyLimit,
    http::HeaderValue,
    middleware,
    routing::{delete, get, patch, post, put},
};
use std::sync::Arc;
use tower_http::{
    cors::{AllowOrigin, Any, CorsLayer},
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};

pub async fn spawn_server(state: Arc<AppState>) -> anyhow::Result<()> {
    let listener = state
        .take_listener()
        .ok_or_else(|| anyhow::anyhow!("OpenDesk API listener was already taken"))?;
    let listener = tokio::net::TcpListener::from_std(listener)?;
    let address = listener.local_addr()?;
    let app = router(state);

    tracing::info!(%address, "OpenDesk local API listening");

    tokio::spawn(async move {
        if let Err(error) = axum::serve(listener, app).await {
            tracing::error!(error = %error, "OpenDesk API stopped");
        }
    });

    Ok(())
}

pub(crate) fn router(state: Arc<AppState>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::list([
            HeaderValue::from_static("tauri://localhost"),
            HeaderValue::from_static("http://tauri.localhost"),
            HeaderValue::from_static("http://localhost:1420"),
            HeaderValue::from_static("http://127.0.0.1:1420"),
            HeaderValue::from_static("http://localhost:5173"),
            HeaderValue::from_static("http://127.0.0.1:5173"),
        ]))
        .allow_methods(Any)
        .allow_headers(Any);

    use core::*;

    let router = Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(core::readyz))
        .route("/api/health", get(health))
        .route("/api/auth/session", post(auth::create_session))
        .route("/api/auth/logout", post(auth::logout))
        .route("/api/auth/sessions", get(auth::list_sessions))
        .route("/api/auth/passkey/status", get(passkey::status))
        .route("/api/auth/passkeys", get(passkey::list))
        .route("/api/auth/passkeys/{id}", delete(passkey::delete))
        .route(
            "/api/auth/passkey/register/start",
            post(passkey::register_start),
        )
        .route(
            "/api/auth/passkey/register/finish",
            post(passkey::register_finish),
        )
        .route("/api/auth/passkey/login/start", post(passkey::login_start))
        .route(
            "/api/auth/passkey/login/finish",
            post(passkey::login_finish),
        )
        .route("/api/push/vapid", get(push::vapid_key))
        .route("/api/push/subscribe", post(push::subscribe))
        .route("/api/push/unsubscribe", post(push::unsubscribe))
        .route("/api/push/subscriptions", get(push::subscriptions))
        .route("/api/push/test", post(push::test))
        .route("/api/adapters", get(adapters::list))
        .route("/api/adapters/runtime", get(adapters::runtime_info))
        .route("/api/adapters/default", post(adapters::set_default))
        .route("/api/adapters/{id}", patch(adapters::update))
        .route("/api/adapters/{id}/health", post(adapters::health))
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
        .route("/api/skills/eval", get(skills::evaluate))
        .route(
            "/api/skills/{id}/gate",
            get(skills::gate_latest).post(skills::gate_run),
        )
        .route(
            "/api/skills/{id}/cases",
            get(skills::list_cases).post(skills::create_case),
        )
        .route(
            "/api/skills/{id}/cases/{case_id}",
            axum::routing::delete(skills::delete_case),
        )
        .route(
            "/api/skills/{id}/gate/{gate_id}/decide",
            post(skills::gate_decide),
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
        .route("/api/nodes", get(nodes::list).post(nodes::create))
        .route(
            "/api/nodes/{id}",
            patch(nodes::update).delete(nodes::delete),
        )
        .route("/api/nodes/{id}/rotate", post(nodes::rotate))
        .route("/api/nodes/heartbeat", post(nodes::heartbeat))
        .route("/api/nodes/next-job", get(nodes::next_job))
        .route("/api/nodes/jobs/{id}/heartbeat", post(nodes::job_heartbeat))
        .route("/api/nodes/jobs/{id}/complete", post(nodes::complete_job))
        .route("/api/onboarding", get(onboarding::status))
        .route(
            "/api/mcp",
            get(mcp::stream).post(mcp::rpc).delete(mcp::delete),
        )
        .route("/api/meta", get(meta))
        .route("/api/about", get(about::about))
        .route(
            "/api/about/desktop-integration",
            post(about::repair_desktop),
        )
        .route("/api/meta/build", post(meta::build))
        .route("/api/daemon/status", get(daemon_status))
        .route("/api/daemon/restart", post(restart_daemon))
        .route("/api/engine/status", get(engine::status))
        .route("/api/engine/detect", post(engine::detect))
        .route("/api/engine/binary", post(engine::set_binary))
        .route("/api/engine/restart", post(engine::restart))
        .route("/api/engine/diagnostics", get(engine::diagnostics))
        .route("/api/models/health", get(engine::model_health))
        .route("/api/models/test", post(engine::test_model))
        .route("/api/models/health/clear", post(engine::clear_model_health))
        .route("/api/engine/update/check", post(engine::update_check))
        .route("/api/engine/update", post(engine::update))
        .route("/api/engine/rollback", post(engine::rollback))
        .route("/api/updates/status", get(updates::status))
        .route("/api/updates/check", post(updates::check))
        .route(
            "/api/updates/token",
            axum::routing::put(updates::put_token).delete(updates::delete_token),
        )
        .route("/api/updates/download", post(updates::download))
        .route("/api/updates/rollback", post(updates::rollback))
        .route("/api/models", get(models))
        .route("/api/agents", get(agents))
        .route("/api/dashboard", get(dashboard))
        .route("/api/metrics", get(metrics::get))
        .route("/api/audit", get(audit::list))
        .route("/api/attention", get(attention::attention))
        .route("/api/attention/dismiss", post(attention::dismiss))
        .route("/api/starter-packs", get(starter_packs::list_packs))
        .route(
            "/api/starter-packs/templates",
            get(starter_packs::list_templates),
        )
        .route(
            "/api/starter-packs/templates/create",
            post(starter_packs::create_template),
        )
        .route(
            "/api/starter-packs/{id}",
            get(starter_packs::get_pack).delete(starter_packs::uninstall_pack),
        )
        .route(
            "/api/starter-packs/{id}/preview",
            post(starter_packs::preview_pack),
        )
        .route(
            "/api/starter-packs/{id}/items",
            get(starter_packs::list_items),
        )
        .route(
            "/api/starter-packs/{id}/items/install",
            post(starter_packs::install_item),
        )
        .route(
            "/api/starter-packs/{id}/install",
            post(starter_packs::install_pack),
        )
        .route(
            "/api/attachments",
            post(attachments::save)
                .delete(attachments::delete)
                .layer(DefaultBodyLimit::max(attachments::MAX_BODY_BYTES)),
        )
        .route(
            "/api/capture",
            post(capture::capture).layer(DefaultBodyLimit::max(attachments::MAX_BODY_BYTES)),
        )
        .route("/api/blueprints", get(blueprints::list_blueprints))
        .route(
            "/api/blueprints/{id}/apply",
            post(blueprints::apply_blueprint),
        )
        .route("/api/cost/mix", get(cost::model_mix))
        .route(
            "/api/personal-context",
            get(personal_context::list).post(personal_context::create),
        )
        .route(
            "/api/personal-context/{id}",
            put(personal_context::update).delete(personal_context::delete),
        )
        .route("/api/permissions", get(permissions::list_permissions))
        .route(
            "/api/permissions/{request_id}/reply",
            post(permissions::reply_permission),
        )
        .route("/api/interactions/pending", get(interactions::list_pending))
        .route("/api/missed-runs", get(missed_runs::list_missed))
        .route("/api/missed-runs/run-all", post(missed_runs::run_all))
        .route("/api/missed-runs/ignore-all", post(missed_runs::ignore_all))
        .route("/api/missed-runs/{id}/run", post(missed_runs::run_missed))
        .route(
            "/api/missed-runs/{id}/ignore",
            post(missed_runs::ignore_missed),
        )
        .route("/api/interactions/{id}", get(interactions::get_interaction))
        .route(
            "/api/interactions/{id}/resolve",
            post(interactions::resolve),
        )
        .route(
            "/api/tasks/{id}/interactions",
            get(interactions::list_for_task).post(interactions::ask),
        )
        .route("/api/agents/states", get(permissions::agent_states))
        .route(
            "/api/workspaces",
            get(list_workspaces).post(create_workspace),
        )
        .route(
            "/api/workspaces/{id}",
            get(get_workspace)
                .patch(update_workspace)
                .delete(delete_workspace),
        )
        .route("/api/workspaces/{id}/activate", post(activate_workspace))
        .route("/api/workspaces/{id}/sync/manifest", get(sync::manifest))
        .route("/api/workspaces/{id}/sync/bundle", post(sync::bundle))
        .route("/api/workspaces/{id}/sync/apply", post(sync::apply))
        .route("/api/settings", get(list_settings))
        .route("/api/settings/dedupe", get(dedupe::get).put(dedupe::put))
        .route("/api/settings/{key}", get(get_setting).put(put_setting))
        .route("/api/app/defaults", get(app_defaults))
        .route("/api/studio/agents", get(studio::list_agents))
        .route(
            "/api/studio/agent",
            get(studio::get_agent)
                .put(studio::put_agent)
                .delete(studio::delete_agent),
        )
        .route("/api/studio/skills", get(studio::list_skills))
        .route(
            "/api/studio/skill",
            get(studio::get_skill)
                .put(studio::put_skill)
                .delete(studio::delete_skill),
        )
        .route("/api/studio/commands", get(studio::list_commands))
        .route(
            "/api/studio/command",
            get(studio::get_command)
                .put(studio::put_command)
                .delete(studio::delete_command),
        )
        .route(
            "/api/studio/config",
            get(studio::get_config).put(studio::put_config),
        )
        .route("/api/studio/effective", get(studio_ops::effective))
        .route("/api/studio/catalog", get(studio_ops::catalog))
        .route("/api/studio/runtime", get(studio_ops::runtime))
        .route(
            "/api/studio/runtime/import",
            post(studio_ops::import_runtime),
        )
        .route(
            "/api/studio/install/preview",
            post(studio_ops::install_preview),
        )
        .route("/api/studio/install/apply", post(studio_ops::install_apply))
        .route(
            "/api/studio/{kind}/{id}/duplicate",
            post(studio_ops::duplicate),
        )
        .route("/api/studio/{kind}/{id}/copy", post(studio_ops::copy))
        .route("/api/studio/{kind}/{id}/move", post(studio_ops::move_item))
        .route("/api/studio/{kind}/{id}/toggle", post(studio_ops::toggle))
        .route(
            "/api/studio/{kind}/{id}/backups",
            get(studio_ops::list_backups),
        )
        .route(
            "/api/studio/{kind}/{id}/restore",
            post(studio_ops::restore_backup),
        )
        .route(
            "/api/studio/{kind}/{id}",
            axum::routing::delete(studio_ops::remove_item),
        )
        .route("/api/studio/mcp", get(studio::list_mcp))
        .route(
            "/api/studio/mcp/{name}",
            put(studio::put_mcp).delete(studio::delete_mcp),
        )
        .route(
            "/api/tasks",
            get(tasks::list_tasks).post(tasks::create_task),
        )
        .route(
            "/api/tasks/{id}",
            get(tasks::get_task)
                .patch(tasks::update_task)
                .delete(tasks::delete_task),
        )
        .route("/api/tasks/{id}/session-packet", get(tasks::session_packet))
        .route("/api/tasks/{id}/dispatch", post(tasks::dispatch_task))
        .route("/api/tasks/{id}/approve-plan", post(tasks::approve_plan))
        .route("/api/tasks/{id}/move", post(tasks::move_task))
        .route("/api/tasks/{id}/transition", post(tasks::transition_task))
        .route("/api/tasks/{id}/hold", post(tasks::hold_task))
        .route("/api/tasks/{id}/diagnostics", get(tasks::task_diagnostics))
        .route("/api/tasks/{id}/fork", post(tasks::fork_task))
        .route("/api/tasks/{id}/swarm", post(tasks::swarm_task))
        .route(
            "/api/tasks/{id}/prompt-preview",
            get(prompt_preview::preview),
        )
        .route(
            "/api/prompt/templates",
            get(prompt_templates::get).put(prompt_templates::put),
        )
        .route("/api/prompt/templates/reset", post(prompt_templates::reset))
        .route(
            "/api/tasks/{id}/watchers",
            get(task_watchers::list).put(task_watchers::replace),
        )
        .route("/api/tasks/{id}/context", get(permissions::context))
        .route("/api/tasks/{id}/compact", post(permissions::compact))
        .route(
            "/api/tasks/{id}/memory",
            get(tasks::get_task_memory)
                .post(tasks::post_task_memory)
                .delete(tasks::delete_task_memory),
        )
        .route(
            "/api/tasks/{id}/messages",
            get(tasks::list_task_messages).post(tasks::post_task_message),
        )
        .route(
            "/api/chat/threads",
            get(chat::list_threads).post(chat::create_thread),
        )
        .route(
            "/api/chat/threads/{id}",
            get(chat::get_thread)
                .patch(chat::update_thread)
                .delete(chat::delete_thread),
        )
        .route(
            "/api/chat/threads/{id}/messages",
            post(chat::send_message).layer(DefaultBodyLimit::max(attachments::MAX_BODY_BYTES)),
        )
        .route("/api/chat/threads/{id}/files", post(chat::send_file))
        .route(
            "/api/chat/threads/{id}/email-reply",
            post(chat::send_email_reply),
        )
        .route(
            "/api/chat/threads/{id}/interrupt",
            post(chat::interrupt_thread),
        )
        .route(
            "/api/chat/threads/{id}/turns/latest",
            get(chat::latest_turn),
        )
        .route(
            "/api/chat/threads/{id}/prompt-preview",
            get(chat::prompt_preview),
        )
        .route("/api/queue", get(queue::list_queue))
        .route("/api/queue/status", get(queue::queue_status))
        .route("/api/queue/pause", post(queue::pause_queue))
        .route("/api/queue/resume", post(queue::resume_queue))
        .route("/api/queue/{id}/cancel", post(queue::cancel_job))
        .route("/api/queue/{id}/retry", post(queue::retry_job))
        .route("/api/queue/{id}/priority", patch(queue::update_priority))
        .route("/api/queue/concurrency", put(queue::set_concurrency))
        .route(
            "/api/calendar/events",
            get(calendar::list_events).post(calendar::create_event),
        )
        .route(
            "/api/calendar/events/{id}",
            get(calendar::get_event)
                .patch(calendar::update_event)
                .delete(calendar::delete_event),
        )
        .route("/api/calendar/sources", get(calendar::list_sources))
        .route(
            "/api/calendar/sources/{id}",
            axum::routing::patch(calendar::update_source).delete(calendar::delete_source),
        )
        .route(
            "/api/calendar/sources/{id}/sync",
            post(calendar::sync_source),
        )
        .route(
            "/api/calendar/google/calendars",
            get(calendar::google_calendars),
        )
        .route("/api/calendar/google/import", post(calendar::import_google))
        .route("/api/calendar/sync", post(calendar::sync_calendars))
        .route("/api/calendar/agenda", get(calendar::agenda))
        .route("/api/runs", get(runs::list_runs))
        .route("/api/runs/ab", get(ab::list).post(ab::create))
        .route("/api/runs/ab/{id}", get(ab::get))
        .route("/api/runs/ab/{id}/choose", post(ab::choose))
        .route("/api/runs/{id}", get(runs::get_run))
        .route("/api/runs/{id}/log", get(runs::run_log))
        .route("/api/runs/{id}/timeline", get(runs::run_timeline))
        .route("/api/runs/{id}/thread", get(runs::run_thread_timeline))
        .route("/api/runs/{id}/export", get(runs::export_run))
        .route(
            "/api/runs/{id}/retry-other-model",
            post(tasks::retry_other_model),
        )
        .route("/api/runs/{id}/fork", post(tasks::fork_from_run))
        .route("/api/test/seed", post(seed::seed))
        .route("/api/search/recent", get(search::recent))
        .route("/api/search", get(search::search))
        .route("/api/files/tree", get(files::tree))
        .route("/api/files/search", get(files::search))
        .route("/api/files/content", get(files::content))
        .route("/api/files/archive", get(files::archive))
        .route("/api/files/save", axum::routing::post(files::save_file))
        .route("/api/files/raw", get(files::raw))
        .route("/api/files/media", get(files::media))
        .route(
            "/api/io/inbound",
            get(io_bus::list_inbound).post(io_bus::create_inbound),
        )
        .route(
            "/api/io/inbound/{id}",
            axum::routing::patch(io_bus::update_inbound).delete(io_bus::delete_inbound),
        )
        .route(
            "/api/io/outbound",
            get(io_bus::list_outbound).post(io_bus::create_outbound),
        )
        .route(
            "/api/io/outbound/{id}",
            axum::routing::patch(io_bus::update_outbound).delete(io_bus::delete_outbound),
        )
        .route("/api/io/secrets", post(io_bus::put_secret))
        .route(
            "/api/io/secrets/{key}",
            axum::routing::delete(io_bus::delete_secret),
        )
        .route("/api/recipes", get(recipes::list).post(recipes::sync))
        .route("/api/recipes/sync", post(recipes::sync))
        .route("/api/recipes/runs", get(recipes::runs))
        .route("/api/recipes/runs/{id}", get(recipes::run_detail))
        .route("/api/recipes/runs/{id}/cancel", post(recipes::cancel_run))
        .route("/api/recipes/runs/{id}/retry", post(recipes::retry_run))
        .route("/api/recipes/hitl", get(recipes::hitl_pending))
        .route(
            "/api/recipes/hitl/{token}/resolve",
            post(recipes::resolve_hitl),
        )
        .route(
            "/api/recipes/{id}",
            get(recipes::get)
                .put(recipes::save)
                .delete(recipes::delete_recipe),
        )
        .route("/api/recipes/{id}/enabled", post(recipes::set_enabled))
        .route("/api/recipes/{id}/versions", get(recipes::versions))
        .route(
            "/api/recipes/{id}/versions/{version_id}",
            get(recipes::version),
        )
        .route(
            "/api/recipes/{id}/versions/{version_id}/diff",
            get(recipes::version_diff),
        )
        .route("/api/recipes/{id}/rollback", post(recipes::rollback))
        .route("/api/recipes/{id}/run", post(recipes::run))
        .route("/api/io/tools", get(io_bus::tools))
        .route("/api/io/tools/{name}/install", post(io_bus::install_tool))
        .route("/api/io/tools/{name}/path", put(io_bus::set_tool_path))
        .route(
            "/api/tasks/{id}/sinks",
            get(io_bus::task_sinks).post(io_bus::set_task_sinks),
        )
        .route(
            "/api/schedules/{id}/sinks",
            get(io_bus::schedule_sinks).post(io_bus::set_schedule_sinks),
        )
        .route("/api/webhooks/{source_id}", post(io_bus::webhook))
        .route(
            "/api/proposals",
            get(proposals::list).post(proposals::create),
        )
        .route("/api/proposals/{id}/resolve", post(proposals::resolve))
        .route("/api/backups", get(backups::list).post(backups::create))
        .route("/api/backups/{id}/download", get(backups::download))
        .route("/api/backups/{id}/restore", post(backups::restore))
        .route("/api/backups/{id}", axum::routing::delete(backups::delete))
        .route("/api/browse/status", get(browse::status))
        .route("/api/browse/enable", post(browse::enable))
        .route("/api/browse/disable", post(browse::disable))
        .route("/api/governance", get(governance::overview))
        .route("/api/governance/agents/{id}", put(governance::put_agent))
        .route(
            "/api/governance/agents/{id}/heartbeat",
            post(governance::heartbeat_now),
        )
        .route("/api/governance/goals", post(governance::create_goal))
        .route(
            "/api/governance/goals/{id}",
            put(governance::update_goal).delete(governance::delete_goal),
        )
        .route(
            "/api/governance/delegations",
            post(governance::create_delegation),
        )
        .route(
            "/api/governance/delegations/{id}/resolve",
            post(governance::resolve_delegation),
        )
        .route("/api/learning", get(learning::overview))
        .route("/api/learning/lessons", post(learning::create_lesson))
        .route("/api/learning/settings", put(learning::update_settings))
        .route("/api/learning/scan", post(learning::scan))
        .route(
            "/api/learning/lessons/{id}/dismiss",
            post(learning::dismiss),
        )
        .route(
            "/api/learning/lessons/{id}/restore",
            post(learning::restore),
        )
        .route("/api/learning/lessons/{id}/forget", post(learning::forget))
        .route(
            "/api/learning/lessons/{id}/promote",
            post(learning::promote),
        )
        .route(
            "/api/learning/lessons/{id}/remember",
            post(learning::remember),
        )
        .route("/api/artifacts", get(artifacts::list_artifacts))
        .route("/api/artifacts/pinned", get(artifacts::pinned_artifacts))
        .route("/api/artifacts/{id}", get(artifacts::get_artifact))
        .route("/api/artifacts/{id}/raw", get(artifacts::artifact_raw))
        .route("/api/artifacts/{id}/pin", post(artifacts::pin_artifact))
        .route(
            "/api/artifacts/{id}/rollback",
            post(artifacts::rollback_artifact),
        )
        .route(
            "/api/artifacts/{id}/content",
            get(artifacts::artifact_content),
        )
        .route(
            "/api/artifacts/{id}/archive",
            get(artifacts::artifact_archive),
        )
        .route(
            "/api/artifacts/{id}/save",
            axum::routing::post(artifacts::save_artifact),
        )
        .route("/api/router/config", get(router::get_config))
        .route("/api/router/model", put(router::set_router_model))
        .route("/api/router/models", put(router::set_models))
        .route("/api/router/blend", put(router::set_blend))
        .route("/api/router/analyze", post(router::analyze))
        .route("/api/router/test", post(router::test_decision))
        .route(
            "/api/router/rules",
            get(router::list_rules).post(router::create_rule),
        )
        .route(
            "/api/router/rules/{id}",
            patch(router::update_rule).delete(router::delete_rule),
        )
        .route(
            "/api/budget",
            get(budget::get_budget).put(budget::put_budget),
        )
        .route("/api/budget/status", get(budget::budget_status))
        .route(
            "/api/schedules",
            get(schedules::list_schedules).post(schedules::create_schedule),
        )
        .route("/api/schedules/tick", post(schedules::tick_scheduler))
        .route("/api/schedules/preview", post(schedules::preview_schedule))
        .route("/api/schedules/{id}/runs", get(schedules::schedule_runs))
        .route("/api/schedules/{id}/dry-run", post(schedules::dry_run))
        .route(
            "/api/schedules/{id}/notify-test",
            post(schedules::test_notification),
        )
        .route(
            "/api/schedules/{id}",
            get(schedules::get_schedule)
                .patch(schedules::update_schedule)
                .delete(schedules::delete_schedule),
        )
        .route("/api/schedules/{id}/run", post(schedules::run_schedule_now))
        .route(
            "/api/schedules/{id}/active",
            post(schedules::set_schedule_active),
        )
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
        .route("/ws", get(ws::ws_handler))
        .fallback(not_found)
        .layer(middleware::from_fn_with_state(
            state.clone(),
            guard_middleware,
        ))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let web_dir = std::env::var("OPENDESK_WEB_DIR").unwrap_or_else(|_| "dist".to_string());
    let index = std::path::Path::new(&web_dir).join("index.html");
    let router = if index.is_file() {
        router.fallback_service(ServeDir::new(&web_dir).fallback(ServeFile::new(index)))
    } else {
        router
    };
    router.layer(middleware::from_fn(core::security_headers))
}

#[cfg(test)]
mod tests;

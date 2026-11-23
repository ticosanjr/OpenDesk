//! Route definitions for workspaces, settings, studio SSOT, starter packs, learning and governance.

use crate::api::core::*;
use crate::api::*;
use crate::app_state::AppState;
use axum::Router;
use axum::routing::{get, patch, post, put};
use std::sync::Arc;

pub(super) fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/adapters", get(adapters::list))
        .route("/api/adapters/runtime", get(adapters::runtime_info))
        .route("/api/adapters/default", post(adapters::set_default))
        .route("/api/adapters/{id}", patch(adapters::update))
        .route("/api/adapters/{id}/health", post(adapters::health))
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
        .route(
            "/api/skills/{id}/shadow",
            get(skills::shadow_list).post(skills::shadow_run),
        )
        .route("/api/skills/{id}/shadow/{run_id}", get(skills::shadow_get))
        .route(
            "/api/skills/{id}/shadow/{run_id}/promote",
            post(skills::shadow_promote),
        )
        .route("/api/skills/{id}/rollback", post(skills::rollback))
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
}

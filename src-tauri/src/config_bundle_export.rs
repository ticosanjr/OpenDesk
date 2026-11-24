use crate::{
    db::{budget, channels, io_bus, memory, queue, router, workspaces},
    dedupe,
    dsh_router::prompt_templates::PromptTemplates,
    error::AppResult,
    learning, personal_context,
};
use serde_json::{Map, Value, json};
use sqlx::SqlitePool;

use super::SCHEMA_VERSION;
use super::config_bundle_types::{BundleSections, ConfigBundle, ExportRequest};
use super::wanted;

pub(super) fn sanitize(value: &mut Value) {
    match value {
        Value::Object(object) => {
            object.retain(|key, _| {
                let key = key.to_lowercase();
                !(key.contains("password")
                    || key.contains("token")
                    || key.contains("secret")
                    || key.contains("api_key")
                    || key.contains("apikey")
                    || key.contains("ciphertext")
                    || key == "nonce")
            });
            for child in object.values_mut() {
                sanitize(child);
            }
        }
        Value::Array(items) => {
            for item in items {
                sanitize(item);
            }
        }
        _ => {}
    }
}

pub(super) fn memory_path(root: &str) -> std::path::PathBuf {
    std::path::Path::new(root).join("MEMORY.md")
}

pub async fn export(
    pool: &SqlitePool,
    workspace_id: &str,
    request: &ExportRequest,
) -> AppResult<ConfigBundle> {
    let workspace = workspaces::get(pool, workspace_id).await?;
    let requested = request.sections.as_deref();
    let mut sections = BundleSections::default();

    if wanted("memory_md", requested) {
        sections.memory_md = std::fs::read_to_string(memory_path(&workspace.root_path)).ok();
    }
    if wanted("structured_memory", requested) {
        let namespaces = memory::list_namespaces(pool, workspace_id).await?;
        let notes = memory::list_notes(pool, workspace_id, None, None, 10_000).await?;
        let mut acls = Map::new();
        for namespace in &namespaces {
            let entries = memory::list_acl(pool, &namespace.id).await?;
            acls.insert(namespace.id.clone(), serde_json::to_value(entries)?);
        }
        sections.structured_memory = Some(json!({
            "namespaces": namespaces,
            "notes": notes,
            "acls": acls,
        }));
    }
    if wanted("personal_context", requested) {
        let groups = personal_context::list(pool, workspace_id).await?;
        let (global, local): (Vec<_>, Vec<_>) = groups
            .into_iter()
            .partition(|group| group.scope == "global");
        sections.personal_context = Some(json!({
            "global": global,
            "workspace": local,
        }));
    }
    if wanted("learning", requested) {
        sections.learning = Some(json!({
            "settings": learning::get_settings(pool, workspace_id).await?,
            "lessons": learning::list_lessons(pool, workspace_id, None, 5_000).await?,
        }));
    }
    if wanted("prompt_templates", requested) {
        sections.prompt_templates = Some(json!({
            "global": PromptTemplates::load(pool, None).await?,
            "workspace": PromptTemplates::load(pool, Some(workspace_id)).await?,
        }));
    }
    if wanted("router", requested) {
        let models = router::models(pool, workspace_id).await?;
        let router_model = crate::db::settings::get(pool, "router_model")
            .await?
            .and_then(|value| value.as_str().map(str::to_string));
        sections.router = Some(json!({
            "policy": router::workspace_policy(pool, workspace_id).await?,
            "default_model": models.default_model,
            "manual_model": models.manual_model,
            "router_model": router_model,
            "rules": router::list(pool, workspace_id).await?,
        }));
    }
    if wanted("budget", requested) {
        sections.budget = Some(serde_json::to_value(budget::get(pool).await?)?);
    }
    if wanted("queue", requested) {
        sections.queue = Some(json!({
            "policy": queue::load_policy(pool).await?,
            "dedupe": dedupe::get_settings(pool).await?,
        }));
    }
    if wanted("automation_routes", requested) {
        sections.automation_routes = Some(json!({
            "inbound_sources": io_bus::list_inbound(pool, workspace_id).await?,
            "outbound_sinks": io_bus::list_outbound(pool, workspace_id).await?,
            "channel_routes": channels::list(pool, workspace_id).await?,
        }));
    }
    if wanted("governance", requested) {
        sections.governance = Some(json!({
            "profiles": crate::governance::list_profiles(pool, workspace_id).await?,
            "goals": crate::governance::list_goals(pool, workspace_id).await?,
        }));
    }

    let bundle = ConfigBundle {
        schema_version: SCHEMA_VERSION,
        exported_at: chrono::Utc::now().to_rfc3339(),
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        source_workspace: json!({
            "id": workspace.id,
            "name": workspace.name,
            "timezone": workspace.timezone,
        }),
        sections,
    };
    let mut value = serde_json::to_value(&bundle)?;
    sanitize(&mut value);
    serde_json::from_value(value).map_err(Into::into)
}

pub(super) fn entries(value: &Value, key: &str) -> usize {
    value
        .get(key)
        .map(|value| match value {
            Value::Array(items) => items.len(),
            Value::Object(map) => map.len(),
            Value::String(text) => usize::from(!text.trim().is_empty()),
            _ => 0,
        })
        .unwrap_or(0)
}

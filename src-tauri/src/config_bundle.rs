use crate::{
    db::{budget, queue, workspaces},
    dedupe,
    error::{AppError, AppResult},
};
use serde_json::Value;
use sqlx::SqlitePool;

#[path = "config_bundle_types.rs"]
pub mod config_bundle_types;
#[path = "config_bundle_export.rs"]
mod export;
#[path = "config_bundle_import_memory.rs"]
mod import_memory;
#[path = "config_bundle_import_ops.rs"]
mod import_ops;

pub use config_bundle_types::{
    BundleSections, ConfigBundle, ExportRequest, ImportMode, ImportRequest, PreviewReport,
    SectionPreview,
};
pub use export::export;
use export::{entries, memory_path};
use import_memory::{import_memory, import_personal_context, import_prompts};
use import_ops::{import_automation, import_governance, import_learning, import_router};

pub const SCHEMA_VERSION: u32 = config_bundle_types::SCHEMA_VERSION;
pub const MAX_BUNDLE_BYTES: usize = config_bundle_types::MAX_BUNDLE_BYTES;

const SECTION_NAMES: [&str; 10] = [
    "memory_md",
    "structured_memory",
    "personal_context",
    "learning",
    "prompt_templates",
    "router",
    "budget",
    "queue",
    "automation_routes",
    "governance",
];

pub(super) fn wanted(name: &str, requested: Option<&[String]>) -> bool {
    requested
        .map(|list| list.is_empty() || list.iter().any(|item| item == name))
        .unwrap_or(true)
}

pub async fn preview(
    pool: &SqlitePool,
    workspace_id: &str,
    bundle: &ConfigBundle,
    requested: Option<&[String]>,
) -> AppResult<PreviewReport> {
    validate(bundle)?;
    let current = export(
        pool,
        workspace_id,
        &ExportRequest {
            workspace_id: None,
            sections: None,
        },
    )
    .await?;
    let mut report = PreviewReport::default();
    let incoming_memory = bundle
        .sections
        .memory_md
        .as_ref()
        .map(|text| Value::String(text.clone()));
    let existing_memory = current
        .sections
        .memory_md
        .as_ref()
        .map(|text| Value::String(text.clone()));
    let pairs = [
        (
            "memory_md",
            incoming_memory.as_ref(),
            existing_memory.as_ref(),
        ),
        (
            "structured_memory",
            bundle.sections.structured_memory.as_ref(),
            current.sections.structured_memory.as_ref(),
        ),
        (
            "personal_context",
            bundle.sections.personal_context.as_ref(),
            current.sections.personal_context.as_ref(),
        ),
        (
            "learning",
            bundle.sections.learning.as_ref(),
            current.sections.learning.as_ref(),
        ),
        (
            "prompt_templates",
            bundle.sections.prompt_templates.as_ref(),
            current.sections.prompt_templates.as_ref(),
        ),
        (
            "router",
            bundle.sections.router.as_ref(),
            current.sections.router.as_ref(),
        ),
        (
            "budget",
            bundle.sections.budget.as_ref(),
            current.sections.budget.as_ref(),
        ),
        (
            "queue",
            bundle.sections.queue.as_ref(),
            current.sections.queue.as_ref(),
        ),
        (
            "automation_routes",
            bundle.sections.automation_routes.as_ref(),
            current.sections.automation_routes.as_ref(),
        ),
        (
            "governance",
            bundle.sections.governance.as_ref(),
            current.sections.governance.as_ref(),
        ),
    ];
    for (name, incoming, existing) in pairs {
        if !wanted(name, requested) {
            continue;
        }
        let Some(incoming_value) = incoming else {
            continue;
        };
        let incoming_count = match name {
            "structured_memory" => entries(incoming_value, "notes"),
            "personal_context" => entries(incoming_value, "workspace"),
            "router" => entries(incoming_value, "rules"),
            "automation_routes" => {
                entries(incoming_value, "inbound_sources")
                    + entries(incoming_value, "outbound_sinks")
                    + entries(incoming_value, "channel_routes")
            }
            "governance" => entries(incoming_value, "profiles") + entries(incoming_value, "goals"),
            _ => 1,
        };
        let existing_count = existing
            .as_ref()
            .map(|value| match name {
                "structured_memory" => entries(value, "notes"),
                "personal_context" => entries(value, "workspace"),
                "router" => entries(value, "rules"),
                "automation_routes" => {
                    entries(value, "inbound_sources")
                        + entries(value, "outbound_sinks")
                        + entries(value, "channel_routes")
                }
                "governance" => entries(value, "profiles") + entries(value, "goals"),
                _ => usize::from(value.as_object().is_some_and(|object| !object.is_empty())),
            })
            .unwrap_or(0);
        report.sections.insert(
            name.to_string(),
            SectionPreview {
                incoming: incoming_count,
                existing: existing_count,
                create: incoming_count.saturating_sub(existing_count),
                update: incoming_count.min(existing_count),
                skip: 0,
            },
        );
    }
    Ok(report)
}

fn validate(bundle: &ConfigBundle) -> AppResult<()> {
    if bundle.schema_version != SCHEMA_VERSION {
        return Err(AppError::BadRequest(format!(
            "schema_version {} não suportado",
            bundle.schema_version
        )));
    }
    Ok(())
}

pub async fn import(
    pool: &SqlitePool,
    workspace_id: &str,
    request: &ImportRequest,
) -> AppResult<PreviewReport> {
    validate(&request.bundle)?;
    let report = preview(
        pool,
        workspace_id,
        &request.bundle,
        request.sections.as_deref(),
    )
    .await?;
    let requested = request.sections.as_deref();
    let sections = &request.bundle.sections;
    let workspace = workspaces::get(pool, workspace_id).await?;
    let mode = request.mode;

    if wanted("memory_md", requested)
        && let Some(text) = &sections.memory_md
    {
        let path = memory_path(&workspace.root_path);
        let exists = path.is_file();
        if mode != ImportMode::MergeSkipExisting || !exists {
            std::fs::write(path, text)?;
        }
    }
    if wanted("structured_memory", requested)
        && let Some(value) = &sections.structured_memory
    {
        import_memory(pool, workspace_id, value, mode).await?;
    }
    if wanted("personal_context", requested)
        && let Some(value) = &sections.personal_context
    {
        import_personal_context(pool, workspace_id, value, mode).await?;
    }
    if wanted("learning", requested)
        && let Some(value) = &sections.learning
    {
        import_learning(pool, workspace_id, value, mode).await?;
    }
    if wanted("prompt_templates", requested)
        && let Some(value) = &sections.prompt_templates
    {
        import_prompts(pool, workspace_id, value, mode).await?;
    }
    if wanted("router", requested)
        && let Some(value) = &sections.router
    {
        import_router(pool, workspace_id, value, mode).await?;
    }
    if wanted("budget", requested)
        && let Some(value) = &sections.budget
    {
        let input: budget::UpdateBudgetInput = serde_json::from_value(value.clone())?;
        budget::update(pool, input).await?;
    }
    if wanted("queue", requested)
        && let Some(value) = &sections.queue
    {
        if let Some(policy) = value.get("policy") {
            let parsed: queue::QueuePolicy = serde_json::from_value(policy.clone())?;
            queue::save_policy(pool, &parsed).await?;
        }
        if let Some(dedupe_value) = value.get("dedupe") {
            let parsed: dedupe::DedupeSettings = serde_json::from_value(dedupe_value.clone())?;
            dedupe::set_settings(pool, parsed).await?;
        }
    }
    if wanted("automation_routes", requested)
        && let Some(value) = &sections.automation_routes
    {
        import_automation(pool, workspace_id, value, mode).await?;
    }
    if wanted("governance", requested)
        && let Some(value) = &sections.governance
    {
        import_governance(pool, workspace_id, value, mode).await?;
    }
    Ok(report)
}
pub fn section_names() -> &'static [&'static str] {
    &SECTION_NAMES
}

#[cfg(test)]
#[path = "config_bundle_tests.rs"]
mod tests;

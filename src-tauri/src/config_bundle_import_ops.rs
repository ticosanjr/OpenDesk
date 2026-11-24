use crate::{
    db::{channels, io_bus, router},
    error::AppResult,
    learning,
};
use serde_json::Value;
use sqlx::SqlitePool;

use super::config_bundle_types::ImportMode;

pub(super) async fn import_learning(
    pool: &SqlitePool,
    workspace_id: &str,
    value: &Value,
    mode: ImportMode,
) -> AppResult<()> {
    if let Some(settings) = value.get("settings") {
        let parsed: learning::LearningSettings = serde_json::from_value(settings.clone())?;
        learning::set_settings(pool, workspace_id, parsed).await?;
    }
    if let Some(lessons) = value.get("lessons").and_then(Value::as_array) {
        for lesson in lessons {
            let fingerprint = lesson
                .get("fingerprint")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if fingerprint.is_empty() {
                continue;
            }
            let exists: Option<String> = sqlx::query_scalar(
                "SELECT id FROM learning_lessons WHERE workspace_id = ? AND fingerprint = ?",
            )
            .bind(workspace_id)
            .bind(fingerprint)
            .fetch_optional(pool)
            .await?;
            let title = lesson
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or("Lição importada");
            let detail = lesson
                .get("detail")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let evidence = lesson
                .get("evidence_json")
                .and_then(Value::as_str)
                .unwrap_or("[]");
            let category = lesson
                .get("category")
                .and_then(Value::as_str)
                .unwrap_or("general");
            let severity = lesson
                .get("severity")
                .and_then(Value::as_str)
                .unwrap_or("INFO");
            let occurrences = lesson
                .get("occurrences")
                .and_then(Value::as_i64)
                .unwrap_or(1);
            if let Some(id) = exists {
                if mode != ImportMode::MergeOverwrite {
                    continue;
                }
                sqlx::query(
                    "UPDATE learning_lessons SET category = ?, severity = ?, title = ?,
                        detail = ?, evidence_json = ?, occurrences = ?,
                        updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
                     WHERE id = ?",
                )
                .bind(category)
                .bind(severity)
                .bind(title)
                .bind(detail)
                .bind(evidence)
                .bind(occurrences)
                .bind(id)
                .execute(pool)
                .await?;
                continue;
            }
            sqlx::query(
                "INSERT INTO learning_lessons
                 (id, workspace_id, category, severity, title, detail, evidence_json, fingerprint, occurrences)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(uuid::Uuid::now_v7().to_string())
            .bind(workspace_id)
            .bind(category)
            .bind(severity)
            .bind(title)
            .bind(detail)
            .bind(evidence)
            .bind(fingerprint)
            .bind(occurrences)
            .execute(pool)
            .await?;
        }
    }
    Ok(())
}

pub(super) async fn import_router(
    pool: &SqlitePool,
    workspace_id: &str,
    value: &Value,
    mode: ImportMode,
) -> AppResult<()> {
    if let Some(policy) = value.get("policy").and_then(Value::as_str) {
        sqlx::query("UPDATE workspaces SET default_router_policy = ? WHERE id = ?")
            .bind(policy)
            .bind(workspace_id)
            .execute(pool)
            .await?;
    }
    let models = router::RouterModels {
        default_model: value
            .get("default_model")
            .and_then(Value::as_str)
            .map(str::to_string),
        manual_model: value
            .get("manual_model")
            .and_then(Value::as_str)
            .map(str::to_string),
    };
    router::set_models(pool, workspace_id, &models).await?;
    if let Some(rule) = value.get("router_model").and_then(Value::as_str) {
        crate::db::settings::set(pool, "router_model", &Value::String(rule.to_string())).await?;
    }
    if let Some(rules) = value.get("rules").and_then(Value::as_array) {
        let existing = router::list(pool, workspace_id).await?;
        if mode == ImportMode::ReplaceSection {
            for rule in &existing {
                if rule.workspace_id.as_deref() == Some(workspace_id) {
                    let _ = router::delete(pool, &rule.id).await;
                }
            }
        }
        for rule in rules {
            let name = rule.get("name").and_then(Value::as_str).unwrap_or_default();
            if name.is_empty() || existing.iter().any(|current| current.name == name) {
                continue;
            }
            router::create(
                pool,
                router::CreateRuleInput {
                    workspace_id: Some(workspace_id.to_string()),
                    name: name.to_string(),
                    priority_order: rule
                        .get("priority_order")
                        .and_then(Value::as_i64)
                        .unwrap_or(0),
                    condition_json: rule
                        .get("condition_json")
                        .and_then(Value::as_str)
                        .unwrap_or("{}")
                        .to_string(),
                    target_mode: rule
                        .get("target_mode")
                        .and_then(Value::as_str)
                        .unwrap_or("build")
                        .to_string(),
                    target_agent: rule
                        .get("target_agent")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    target_model: rule
                        .get("target_model")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    target_effort: rule
                        .get("target_effort")
                        .and_then(Value::as_str)
                        .unwrap_or("medium")
                        .to_string(),
                    is_enabled: rule
                        .get("is_enabled")
                        .and_then(Value::as_bool)
                        .unwrap_or(true),
                },
            )
            .await?;
        }
    }
    Ok(())
}

pub(super) async fn import_automation(
    pool: &SqlitePool,
    workspace_id: &str,
    value: &Value,
    mode: ImportMode,
) -> AppResult<()> {
    let existing_inbound = io_bus::list_inbound(pool, workspace_id).await?;
    let existing_outbound = io_bus::list_outbound(pool, workspace_id).await?;
    if let Some(items) = value.get("inbound_sources").and_then(Value::as_array) {
        for item in items {
            let name = item.get("name").and_then(Value::as_str).unwrap_or_default();
            if name.is_empty() || existing_inbound.iter().any(|current| current.name == name) {
                continue;
            }
            let config = item
                .get("config_json")
                .and_then(Value::as_str)
                .and_then(|raw| serde_json::from_str(raw).ok())
                .unwrap_or(Value::Null);
            io_bus::create_inbound(
                pool,
                workspace_id,
                io_bus::InboundInput {
                    name: name.to_string(),
                    source_type: item
                        .get("source_type")
                        .and_then(Value::as_str)
                        .unwrap_or("WEBHOOK_HTTP")
                        .to_string(),
                    config,
                    target_agent: item
                        .get("target_agent")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    target_column: item
                        .get("target_column")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    target_kind: item
                        .get("target_kind")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    target_id: item
                        .get("target_id")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    auto_execute: item
                        .get("auto_execute")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                    is_enabled: mode != ImportMode::MergeSkipExisting
                        && item.get("is_enabled").and_then(Value::as_bool) != Some(false),
                },
            )
            .await?;
        }
    }
    if let Some(items) = value.get("outbound_sinks").and_then(Value::as_array) {
        for item in items {
            let name = item.get("name").and_then(Value::as_str).unwrap_or_default();
            if name.is_empty() || existing_outbound.iter().any(|current| current.name == name) {
                continue;
            }
            let config = item
                .get("config_json")
                .and_then(Value::as_str)
                .and_then(|raw| serde_json::from_str(raw).ok())
                .unwrap_or(Value::Null);
            io_bus::create_outbound(
                pool,
                workspace_id,
                io_bus::OutboundInput {
                    name: name.to_string(),
                    sink_type: item
                        .get("sink_type")
                        .and_then(Value::as_str)
                        .unwrap_or("HTTP_WEBHOOK")
                        .to_string(),
                    config,
                    is_enabled: item
                        .get("is_enabled")
                        .and_then(Value::as_bool)
                        .unwrap_or(true),
                },
            )
            .await?;
        }
    }
    if let Some(items) = value.get("channel_routes").and_then(Value::as_array) {
        for item in items {
            let channel = item
                .get("channel")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let agent = item
                .get("agent")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if channel.is_empty() || agent.is_empty() {
                continue;
            }
            channels::upsert(
                pool,
                workspace_id,
                channels::UpsertRouteInput {
                    channel: channel.to_string(),
                    identity: item
                        .get("identity")
                        .and_then(Value::as_str)
                        .unwrap_or("*")
                        .to_string(),
                    agent: agent.to_string(),
                    permission_profile: item
                        .get("permission_profile")
                        .and_then(Value::as_str)
                        .unwrap_or("standard")
                        .to_string(),
                    trust_level: item
                        .get("trust_level")
                        .and_then(Value::as_str)
                        .unwrap_or("untrusted")
                        .to_string(),
                },
            )
            .await?;
        }
    }
    Ok(())
}

pub(super) async fn import_governance(
    pool: &SqlitePool,
    workspace_id: &str,
    value: &Value,
    mode: ImportMode,
) -> AppResult<()> {
    if let Some(profiles) = value.get("profiles").and_then(Value::as_array) {
        for profile in profiles {
            let agent_id = profile
                .get("agent_id")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if agent_id.is_empty() {
                continue;
            }
            if mode == ImportMode::MergeSkipExisting
                && crate::governance::get_profile(pool, workspace_id, agent_id)
                    .await?
                    .is_some()
            {
                continue;
            }
            crate::governance::upsert_profile(
                pool,
                workspace_id,
                agent_id,
                crate::governance::AgentProfileInput {
                    role: profile
                        .get("role")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    team: profile
                        .get("team")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    manager_id: profile
                        .get("manager_id")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    monthly_budget_usd: profile.get("monthly_budget_usd").and_then(Value::as_f64),
                    concurrency_quota: profile.get("concurrency_quota").and_then(Value::as_i64),
                    heartbeat_enabled: profile.get("heartbeat_enabled").and_then(Value::as_bool),
                    heartbeat_minutes: profile.get("heartbeat_minutes").and_then(Value::as_i64),
                    notes: profile
                        .get("notes")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                },
            )
            .await?;
        }
    }
    let existing = crate::governance::list_goals(pool, workspace_id).await?;
    if mode == ImportMode::ReplaceSection {
        for goal in &existing {
            crate::governance::delete_goal(pool, workspace_id, &goal.id).await?;
        }
    }
    if let Some(goals) = value.get("goals").and_then(Value::as_array) {
        for goal in goals {
            let agent_id = goal
                .get("agent_id")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let title = goal
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if agent_id.is_empty() || title.is_empty() {
                continue;
            }
            if let Some(current) = existing
                .iter()
                .find(|current| current.agent_id == agent_id && current.title == title)
            {
                if mode != ImportMode::MergeOverwrite {
                    continue;
                }
                crate::governance::delete_goal(pool, workspace_id, &current.id).await?;
            }
            crate::governance::create_goal(
                pool,
                workspace_id,
                crate::governance::AgentGoalInput {
                    agent_id: agent_id.to_string(),
                    title: title.to_string(),
                    description: goal
                        .get("description")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    metric: goal
                        .get("metric")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    target_value: goal.get("target_value").and_then(Value::as_f64),
                    current_value: goal.get("current_value").and_then(Value::as_f64),
                    unit: goal.get("unit").and_then(Value::as_str).map(str::to_string),
                    due_at: goal
                        .get("due_at")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    status: goal
                        .get("status")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                },
            )
            .await?;
        }
    }
    Ok(())
}

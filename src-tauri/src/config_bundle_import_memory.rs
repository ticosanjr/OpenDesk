use crate::{db::memory, error::AppResult, personal_context};
use serde_json::Value;
use sqlx::SqlitePool;
use std::collections::{BTreeMap, HashSet};

use super::config_bundle_types::ImportMode;
use crate::dsh_router::prompt_templates::PromptTemplates;

pub(crate) async fn import_memory(
    pool: &SqlitePool,
    workspace_id: &str,
    value: &Value,
    mode: ImportMode,
) -> AppResult<()> {
    let namespaces: Vec<memory::MemoryNamespace> =
        serde_json::from_value(value.get("namespaces").cloned().unwrap_or_default())?;
    let notes: Vec<memory::MemoryNote> =
        serde_json::from_value(value.get("notes").cloned().unwrap_or_default())?;
    if mode == ImportMode::ReplaceSection {
        let existing = memory::list_notes(pool, workspace_id, None, None, 10_000).await?;
        for note in existing {
            memory::delete_note(pool, &note.id).await?;
        }
    }
    let mut by_name = BTreeMap::new();
    let mut by_old_id = BTreeMap::new();
    for namespace in namespaces {
        let old_id = namespace.id.clone();
        let created = memory::ensure_namespace(
            pool,
            workspace_id,
            &namespace.name,
            namespace.description.as_deref(),
        )
        .await?;
        by_old_id.insert(old_id, created.id.clone());
        by_name.insert(namespace.name, created.id);
    }
    let existing = memory::list_notes(pool, workspace_id, None, None, 10_000).await?;
    let mut keys = existing
        .iter()
        .map(|note| (note.namespace.clone(), note.title.clone()))
        .collect::<HashSet<_>>();
    for note in notes {
        let tags = note.tags();
        let key = (note.namespace.clone(), note.title.clone());
        if keys.contains(&key) {
            if mode == ImportMode::MergeOverwrite
                && let Some(current) = existing.iter().find(|current| {
                    current.namespace == note.namespace && current.title == note.title
                })
            {
                memory::update_note(
                    pool,
                    &current.id,
                    memory::UpdateNoteInput {
                        body: Some(note.body.clone()),
                        tags: Some(tags.clone()),
                        confidence: Some(note.confidence),
                        trust_level: Some(note.trust_level.clone()),
                        ..Default::default()
                    },
                )
                .await?;
            }
            continue;
        }
        memory::create_note(
            pool,
            workspace_id,
            memory::CreateNoteInput {
                namespace: note.namespace,
                kind: note.kind,
                title: note.title,
                body: note.body,
                tags,
                confidence: note.confidence,
                trust_level: note.trust_level,
                expires_at: note.expires_at,
                source: note.source,
            },
        )
        .await?;
        keys.insert(key);
    }
    if let Some(acls) = value.get("acls").and_then(Value::as_object) {
        for (namespace_id, entries) in acls {
            for acl in entries.as_array().into_iter().flatten() {
                if let Some(agent) = acl.get("agent").and_then(Value::as_str) {
                    let namespace = by_old_id
                        .get(namespace_id)
                        .cloned()
                        .unwrap_or_else(|| namespace_id.clone());
                    memory::set_acl(
                        pool,
                        &namespace,
                        agent,
                        acl.get("can_read").and_then(Value::as_bool).unwrap_or(true),
                        acl.get("can_write")
                            .and_then(Value::as_bool)
                            .unwrap_or(false),
                    )
                    .await?;
                }
            }
        }
    }
    Ok(())
}

pub(crate) async fn import_personal_context(
    pool: &SqlitePool,
    workspace_id: &str,
    value: &Value,
    mode: ImportMode,
) -> AppResult<()> {
    let mut groups = Vec::new();
    for key in ["global", "workspace"] {
        if let Some(items) = value.get(key).and_then(Value::as_array) {
            groups.extend(items.iter().cloned());
        }
    }
    if mode == ImportMode::ReplaceSection {
        for group in personal_context::list(pool, workspace_id).await? {
            if group.scope == "workspace" {
                personal_context::delete(pool, &group.id).await?;
            }
        }
    }
    let existing = personal_context::list(pool, workspace_id).await?;
    for group in groups {
        let scope = group
            .get("scope")
            .and_then(Value::as_str)
            .unwrap_or("workspace");
        let name = group
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("Contexto");
        let current = existing
            .iter()
            .find(|item| item.scope == scope && item.name == name);
        if let Some(current) = current {
            if mode == ImportMode::MergeOverwrite {
                personal_context::update(
                    pool,
                    &current.id,
                    personal_context::PersonalContextInput {
                        scope: scope.to_string(),
                        workspace_id: if scope == "workspace" {
                            Some(workspace_id.to_string())
                        } else {
                            None
                        },
                        name: Some(name.to_string()),
                        instructions: group
                            .get("instructions")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string(),
                        is_enabled: group
                            .get("is_enabled")
                            .and_then(Value::as_bool)
                            .unwrap_or(true),
                        position: group.get("position").and_then(Value::as_i64),
                        source_quote: group
                            .get("source_quote")
                            .and_then(Value::as_str)
                            .map(str::to_string),
                    },
                )
                .await?;
            }
            continue;
        }
        personal_context::create(
            pool,
            personal_context::PersonalContextInput {
                scope: scope.to_string(),
                workspace_id: if scope == "workspace" {
                    Some(workspace_id.to_string())
                } else {
                    None
                },
                name: Some(name.to_string()),
                instructions: group
                    .get("instructions")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                is_enabled: group
                    .get("is_enabled")
                    .and_then(Value::as_bool)
                    .unwrap_or(true),
                position: group.get("position").and_then(Value::as_i64),
                source_quote: group
                    .get("source_quote")
                    .and_then(Value::as_str)
                    .map(str::to_string),
            },
        )
        .await?;
    }
    Ok(())
}

pub(crate) async fn import_prompts(
    pool: &SqlitePool,
    workspace_id: &str,
    value: &Value,
    mode: ImportMode,
) -> AppResult<()> {
    if mode == ImportMode::MergeSkipExisting {
        return Ok(());
    }
    if let Some(global) = value.get("global") {
        let parsed: PromptTemplates = serde_json::from_value(global.clone())?;
        PromptTemplates::save(pool, None, &parsed).await?;
    }
    if let Some(local) = value.get("workspace") {
        let parsed: PromptTemplates = serde_json::from_value(local.clone())?;
        PromptTemplates::save(pool, Some(workspace_id), &parsed).await?;
    }
    Ok(())
}

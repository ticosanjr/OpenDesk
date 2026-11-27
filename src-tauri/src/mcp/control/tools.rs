use serde_json::{Value, json};

pub(super) fn tool(name: &str, description: &str, input_schema: Value) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": input_schema,
    })
}

pub(super) fn empty() -> Value {
    json!({ "type": "object", "properties": {} })
}

mod chat;
mod handoffs;
mod interactions;
mod learning;
mod memory;
mod ops;
mod recipes;
mod schedules;
mod status;
mod tasks;
mod vault;
mod voice;
mod worktrees;

/// Aggregates the per-domain tool schemas in their historical order.
pub(super) fn tools() -> Vec<Value> {
    vec![
        status::opendesk_status(),
        tasks::task_list(),
        tasks::task_create(),
        tasks::task_update(),
        tasks::task_move(),
        tasks::task_dispatch(),
        interactions::ask_user(),
        interactions::elicit(),
        interactions::sampling(),
        chat::chat_send_file(),
        learning::learning_save(),
        learning::personal_context_save(),
        learning::personal_context_list(),
        learning::personal_context_create(),
        learning::personal_context_update(),
        learning::personal_context_set_scope(),
        tasks::task_message_list(),
        tasks::task_message_post(),
        tasks::run_list(),
        schedules::routine_list(),
        schedules::routine_create(),
        schedules::routine_update(),
        schedules::routine_set_active(),
        schedules::routine_run(),
        recipes::recipe_list(),
        recipes::recipe_save(),
        recipes::recipe_validate(),
        recipes::recipe_set_enabled(),
        recipes::recipe_run(),
        ops::attention_list(),
        ops::queue_control(),
        ops::artifact_list(),
        ops::artifact_read(),
        ops::artifact_pin(),
        vault::vault_list_refs(),
        vault::vault_request_fill(),
        memory::memory_save(),
        memory::memory_create(),
        memory::memory_list(),
        memory::memory_update(),
        memory::memory_set_scope(),
        memory::memory_search(),
        handoffs::handoff_create(),
        handoffs::handoff_list(),
        handoffs::handoff_respond(),
        handoffs::handoff_finish(),
        ops::mcp_discover_tools(),
        recipes::recipe_resume(),
        recipes::recipe_cancel(),
        voice::voice_speak(),
        ops::artifact_rollback(),
        recipes::recipe_versions(),
        recipes::recipe_diff(),
        recipes::recipe_rollback(),
        worktrees::worktree_list(),
        worktrees::worktree_prepare(),
        worktrees::worktree_diff(),
        worktrees::worktree_merge(),
        worktrees::worktree_cleanup(),
    ]
}

/// Phase-aware visibility. `ALL` keeps the historical behavior (everything).
pub(super) fn filtered_tools(phase: &str, _role: Option<&str>) -> Vec<Value> {
    let phase = phase.trim().to_uppercase();
    tools()
        .into_iter()
        .filter(|tool| {
            let name = tool.get("name").and_then(Value::as_str).unwrap_or("");
            visible_in(name, &phase)
        })
        .collect()
}

/// Workspace-specific tools: every enabled recipe and every enabled agent
/// becomes callable by name. Discovery is best-effort; failures simply leave
/// the static tool set untouched.
pub(super) fn dynamic_tools(recipes: &[Value], agents: &[Value]) -> Vec<Value> {
    let mut tools = Vec::new();
    for recipe in recipes.iter().take(50) {
        let Some(id) = recipe.get("id").and_then(Value::as_str) else {
            continue;
        };
        if !recipe
            .get("is_enabled")
            .and_then(Value::as_bool)
            .unwrap_or(true)
        {
            continue;
        }
        let name = tool_name("recipe", id);
        let label = recipe.get("name").and_then(Value::as_str).unwrap_or(id);
        tools.push(tool(
            &name,
            &format!("Executa a receita '{label}' ({id}) com contexto opcional. Retorna o run durável criado."),
            json!({
                "type": "object",
                "properties": {
                    "trigger": {
                        "type": "object",
                        "description": "Contexto de trigger; fica disponível como {{trigger.*}} na receita."
                    },
                    "dry_run": { "type": "boolean" }
                }
            }),
        ));
    }
    for agent in agents.iter().take(50) {
        let Some(id) = agent.get("id").and_then(Value::as_str) else {
            continue;
        };
        if agent
            .get("disabled")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            continue;
        }
        let name = tool_name("agent", id);
        let label = agent.get("name").and_then(Value::as_str).unwrap_or(id);
        tools.push(tool(
            &name,
            &format!("Cria uma tarefa para o agente '{label}' ({id}) e tenta iniciá-la. Retorna a tarefa criada."),
            json!({
                "type": "object",
                "required": ["prompt"],
                "properties": {
                    "prompt": { "type": "string" },
                    "title": { "type": "string" },
                    "model": { "type": "string" },
                    "mode": { "type": "string" }
                }
            }),
        ));
    }
    tools
}

pub(super) fn tool_name(prefix: &str, id: &str) -> String {
    let sanitized: String = id
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect();
    format!("{prefix}_{sanitized}")
}

fn visible_in(name: &str, phase: &str) -> bool {
    if matches!(phase, "" | "ALL") {
        return true;
    }
    if matches!(
        name,
        "opendesk_status"
            | "task_list"
            | "ask_user"
            | "elicit"
            | "sampling"
            | "memory_search"
            | "memory_list"
            | "personal_context_list"
            | "mcp_discover_tools"
    ) {
        return true;
    }
    let category = if matches!(
        name,
        "queue_control"
            | "routine_list"
            | "routine_create"
            | "routine_update"
            | "routine_set_active"
            | "routine_run"
            | "recipe_list"
            | "recipe_save"
            | "recipe_validate"
            | "recipe_set_enabled"
            | "recipe_run"
            | "recipe_resume"
            | "recipe_cancel"
            | "recipe_versions"
            | "recipe_diff"
            | "recipe_rollback"
            | "attention_list"
            | "worktree_cleanup"
    ) {
        "OPS"
    } else if matches!(name, "worktree_prepare") {
        "BUILD"
    } else if matches!(name, "worktree_merge") {
        "AUDIT"
    } else if matches!(
        name,
        "task_create"
            | "task_update"
            | "task_move"
            | "task_dispatch"
            | "learning_save"
            | "personal_context_save"
            | "personal_context_create"
            | "personal_context_update"
            | "personal_context_set_scope"
            | "memory_save"
            | "memory_create"
            | "memory_update"
            | "memory_set_scope"
            | "chat_send_file"
            | "task_message_post"
            | "voice_speak"
            | "handoff_create"
            | "handoff_respond"
            | "handoff_finish"
            | "vault_request_fill"
    ) {
        "BUILD"
    } else if matches!(name, "artifact_pin" | "artifact_rollback") {
        "AUDIT"
    } else {
        "READ"
    };
    match phase {
        "PLAN" => matches!(category, "READ" | "BUILD"),
        "BUILD" => matches!(category, "READ" | "BUILD"),
        "AUDIT" => matches!(category, "READ"),
        "CHAT" => matches!(category, "READ" | "BUILD"),
        "OPS" => true,
        _ => true,
    }
}

pub(super) fn discover(query: &str, phase: Option<&str>) -> Vec<Value> {
    let query = query.trim().to_lowercase();
    let phase = phase.unwrap_or("ALL");
    tools()
        .into_iter()
        .filter(|tool| {
            let name = tool.get("name").and_then(Value::as_str).unwrap_or("");
            let description = tool
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or("");
            let matches_query = query.is_empty()
                || name.to_lowercase().contains(&query)
                || description.to_lowercase().contains(&query);
            matches_query && visible_in(name, &phase.to_uppercase())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_the_expected_control_tools() {
        let names = tools()
            .into_iter()
            .filter_map(|tool| tool.get("name").and_then(Value::as_str).map(str::to_string))
            .collect::<Vec<_>>();
        for expected in [
            "opendesk_status",
            "task_list",
            "task_create",
            "task_update",
            "task_move",
            "task_dispatch",
            "ask_user",
            "chat_send_file",
            "learning_save",
            "personal_context_save",
            "personal_context_list",
            "personal_context_create",
            "personal_context_update",
            "personal_context_set_scope",
            "task_message_list",
            "task_message_post",
            "run_list",
            "routine_list",
            "routine_create",
            "routine_update",
            "routine_set_active",
            "routine_run",
            "recipe_list",
            "recipe_save",
            "recipe_set_enabled",
            "recipe_run",
            "attention_list",
            "queue_control",
            "artifact_list",
            "artifact_read",
            "artifact_pin",
            "artifact_rollback",
            "mcp_discover_tools",
            "recipe_resume",
            "recipe_cancel",
            "voice_speak",
            "handoff_create",
            "handoff_list",
            "memory_save",
            "memory_create",
            "memory_list",
            "memory_update",
            "memory_set_scope",
            "memory_search",
            "handoff_respond",
            "handoff_finish",
            "vault_list_refs",
            "vault_request_fill",
            "recipe_validate",
        ] {
            assert!(names.iter().any(|name| name == expected), "{expected}");
        }
        for expected in [
            "recipe_versions",
            "recipe_diff",
            "recipe_rollback",
            "worktree_list",
            "worktree_prepare",
            "worktree_diff",
            "worktree_merge",
            "worktree_cleanup",
        ] {
            assert!(names.iter().any(|name| name == expected), "{expected}");
        }
        assert_eq!(names.len(), 59);
    }

    #[test]
    fn builds_dynamic_recipe_and_agent_tools() {
        let recipes = vec![
            json!({ "id": "daily-brief", "name": "Daily", "is_enabled": true }),
            json!({ "id": "off", "name": "Off", "is_enabled": false }),
        ];
        let agents = vec![
            json!({ "id": "build.agent", "name": "Build", "disabled": false }),
            json!({ "id": "off", "name": "Off", "disabled": true }),
        ];
        let tools = dynamic_tools(&recipes, &agents);
        let names = tools
            .iter()
            .filter_map(|tool| tool.get("name").and_then(Value::as_str))
            .collect::<Vec<_>>();
        assert!(names.contains(&"recipe_daily-brief"));
        assert!(!names.contains(&"recipe_off"));
        assert!(names.contains(&"agent_build_agent"));
        assert!(!names.contains(&"agent_off"));
        assert_eq!(tool_name("recipe", "a/b c"), "recipe_a_b_c");
    }

    #[test]
    fn filters_tools_by_phase_and_discovers() {
        let plan = filtered_tools("PLAN", None);
        let names = plan
            .iter()
            .filter_map(|tool| tool.get("name").and_then(Value::as_str))
            .collect::<Vec<_>>();
        assert!(names.contains(&"task_create"));
        assert!(names.contains(&"memory_save"));
        assert!(!names.contains(&"queue_control"));
        assert!(names.contains(&"mcp_discover_tools"));

        let audit = filtered_tools("AUDIT", None);
        let names = audit
            .iter()
            .filter_map(|tool| tool.get("name").and_then(Value::as_str))
            .collect::<Vec<_>>();
        assert!(!names.contains(&"task_create"));
        assert!(names.contains(&"artifact_read"));

        let found = discover("recipe", Some("OPS"));
        assert!(found.iter().any(|tool| tool["name"] == "recipe_run"));
        assert!(
            !discover("recipe", Some("PLAN"))
                .iter()
                .any(|tool| tool["name"] == "recipe_run")
        );
    }
}

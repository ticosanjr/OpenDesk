use super::ranking::{snippet, urlencode};
use super::scope::{add_file_hits, add_studio_hits, recent_files};
use super::*;

pub async fn search(
    State(state): State<Arc<AppState>>,
    Query(query): Query<SearchQuery>,
) -> AppResult<Json<Vec<SearchHit>>> {
    let needle = query.q.trim();
    if needle.len() < 2 {
        return Ok(Json(Vec::new()));
    }
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    let limit = query.limit.unwrap_or(40).clamp(1, 100);
    let like = format!("%{}%", needle.replace('%', "\\%").replace('_', "\\_"));
    let mut hits = Vec::new();

    let tasks = sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT id, title, description, updated_at FROM tasks
         WHERE workspace_id = ? AND (title LIKE ? ESCAPE '\\' OR description LIKE ? ESCAPE '\\' OR tags_json LIKE ? ESCAPE '\\')
         ORDER BY updated_at DESC LIMIT ?",
    )
    .bind(&workspace_id)
    .bind(&like)
    .bind(&like)
    .bind(&like)
    .bind(limit)
    .fetch_all(&state.db)
    .await?;
    for (id, title, description, updated_at) in tasks {
        hits.push(SearchHit {
            link: format!("/kanban?view=board&task={}", urlencode(&id)),
            id,
            kind: "task".into(),
            title,
            snippet: snippet(&description, needle),
            updated_at: Some(updated_at),
        });
    }

    let runs = sqlx::query_as::<_, (String, Option<String>, String, String, String)>(
        "SELECT r.id, t.title, r.summary_response, r.error_message, r.started_at
         FROM task_runs r LEFT JOIN tasks t ON t.id = r.task_id
         WHERE r.workspace_id = ?
           AND (COALESCE(t.title,'') LIKE ? ESCAPE '\\' OR r.agent_name LIKE ? ESCAPE '\\'
                OR r.model_id LIKE ? ESCAPE '\\' OR COALESCE(r.summary_response,'') LIKE ? ESCAPE '\\'
                OR COALESCE(r.error_message,'') LIKE ? ESCAPE '\\')
         ORDER BY r.started_at DESC LIMIT ?",
    )
    .bind(&workspace_id)
    .bind(&like)
    .bind(&like)
    .bind(&like)
    .bind(&like)
    .bind(&like)
    .bind(limit)
    .fetch_all(&state.db)
    .await?;
    for (id, title, summary, error, started_at) in runs {
        hits.push(SearchHit {
            link: format!("/runs/{}", urlencode(&id)),
            id,
            kind: "run".into(),
            title: title.unwrap_or_else(|| "Execução".into()),
            snippet: snippet(&format!("{summary} {error}"), needle),
            updated_at: Some(started_at),
        });
    }

    let artifacts = sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT id, file_path, creation_reason, created_at FROM artifacts_provenance
         WHERE workspace_id = ? AND (file_path LIKE ? ESCAPE '\\' OR creation_reason LIKE ? ESCAPE '\\' OR created_by_agent LIKE ? ESCAPE '\\')
         ORDER BY created_at DESC LIMIT ?",
    )
    .bind(&workspace_id)
    .bind(&like)
    .bind(&like)
    .bind(&like)
    .bind(limit)
    .fetch_all(&state.db)
    .await?;
    for (id, file_path, reason, created_at) in artifacts {
        hits.push(SearchHit {
            link: format!("/artifacts?id={}", urlencode(&id)),
            id,
            kind: "artifact".into(),
            title: file_path,
            snippet: snippet(&reason, needle),
            updated_at: Some(created_at),
        });
    }

    let schedules = sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT id, name, command_or_prompt, created_at FROM schedules
         WHERE workspace_id = ? AND (name LIKE ? ESCAPE '\\' OR command_or_prompt LIKE ? ESCAPE '\\' OR COALESCE(description,'') LIKE ? ESCAPE '\\')
         ORDER BY created_at DESC LIMIT ?",
    )
    .bind(&workspace_id)
    .bind(&like)
    .bind(&like)
    .bind(&like)
    .bind(limit)
    .fetch_all(&state.db)
    .await?;
    for (id, name, command, created_at) in schedules {
        hits.push(SearchHit {
            link: format!("/routines?routine={}", urlencode(&id)),
            id,
            kind: "schedule".into(),
            title: name,
            snippet: snippet(&command, needle),
            updated_at: Some(created_at),
        });
    }

    let workspaces = sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT id, name, root_path, last_opened_at FROM workspaces
         WHERE name LIKE ? ESCAPE '\\' OR root_path LIKE ? ESCAPE '\\' OR COALESCE(description,'') LIKE ? ESCAPE '\\'
         ORDER BY last_opened_at DESC LIMIT ?",
    )
    .bind(&like)
    .bind(&like)
    .bind(&like)
    .bind(limit)
    .fetch_all(&state.db)
    .await?;
    for (id, name, root, last_opened_at) in workspaces {
        hits.push(SearchHit {
            link: "/files".into(),
            id,
            kind: "project".into(),
            title: name,
            snippet: snippet(&root, needle),
            updated_at: Some(last_opened_at),
        });
    }

    let task_messages = sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT id, task_id, body, created_at FROM messages
         WHERE task_id IN (SELECT id FROM tasks WHERE workspace_id = ?)
           AND kind NOT IN ('chat', 'turn', 'hitl_request')
           AND body LIKE ? ESCAPE '\\'
         ORDER BY created_at DESC LIMIT ?",
    )
    .bind(&workspace_id)
    .bind(&like)
    .bind(limit)
    .fetch_all(&state.db)
    .await?;
    for (id, task_id, content, created_at) in task_messages {
        hits.push(SearchHit {
            link: format!("/kanban?view=board&task={}", urlencode(&task_id)),
            id,
            kind: "comment".into(),
            title: "Comentário em tarefa".into(),
            snippet: snippet(&content, needle),
            updated_at: Some(created_at),
        });
    }

    let chat_threads = sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT id, title, COALESCE(agent, ''), updated_at FROM conversations
         WHERE workspace_id = ? AND channel = 'chat'
           AND (title LIKE ? ESCAPE '\\' OR COALESCE(agent, '') LIKE ? ESCAPE '\\')
         ORDER BY updated_at DESC LIMIT ?",
    )
    .bind(&workspace_id)
    .bind(&like)
    .bind(&like)
    .bind(limit)
    .fetch_all(&state.db)
    .await?;
    for (id, title, agent, updated_at) in chat_threads {
        hits.push(SearchHit {
            link: format!("/chat?thread={}", urlencode(&id)),
            id,
            kind: "comment".into(),
            title,
            snippet: format!("conversa com @{agent}"),
            updated_at: Some(updated_at),
        });
    }

    let chat_messages = sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT m.id, m.conversation_id, m.body, m.created_at
         FROM messages m JOIN conversations c ON c.id = m.conversation_id
         WHERE c.workspace_id = ? AND c.channel = 'chat' AND m.kind = 'chat'
           AND m.body LIKE ? ESCAPE '\\'
         ORDER BY m.created_at DESC LIMIT ?",
    )
    .bind(&workspace_id)
    .bind(&like)
    .bind(limit)
    .fetch_all(&state.db)
    .await?;
    for (id, thread_id, content, created_at) in chat_messages {
        hits.push(SearchHit {
            link: format!("/chat?thread={}", urlencode(&thread_id)),
            id,
            kind: "comment".into(),
            title: "Mensagem de conversa".into(),
            snippet: snippet(&content, needle),
            updated_at: Some(created_at),
        });
    }

    add_studio_hits(&state, &workspace_id, needle, &mut hits).await;
    add_file_hits(&state, &workspace_id, needle, &mut hits, limit as usize).await?;

    hits.sort_by(|left, right| {
        right
            .updated_at
            .as_deref()
            .unwrap_or_default()
            .cmp(left.updated_at.as_deref().unwrap_or_default())
    });
    hits.truncate(limit as usize * 3);
    Ok(Json(hits))
}

/// Recent activity shown when the search box is empty. It intentionally uses
/// the same hit shape as a normal search so the UI can reuse the row.
pub async fn recent(
    State(state): State<Arc<AppState>>,
    Query(query): Query<SearchQuery>,
) -> AppResult<Json<Vec<SearchHit>>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    let limit = query.limit.unwrap_or(24).clamp(1, 50);
    let per_source = limit.max(4);
    let mut hits = Vec::new();

    let tasks = sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT id, title, description, updated_at FROM tasks
         WHERE workspace_id = ? ORDER BY updated_at DESC LIMIT ?",
    )
    .bind(&workspace_id)
    .bind(per_source)
    .fetch_all(&state.db)
    .await?;
    for (id, title, description, updated_at) in tasks {
        hits.push(SearchHit {
            link: format!("/kanban?view=board&task={}", urlencode(&id)),
            id,
            kind: "task".into(),
            title,
            snippet: snippet(&description, ""),
            updated_at: Some(updated_at),
        });
    }

    let runs = sqlx::query_as::<_, (String, Option<String>, Option<String>, String)>(
        "SELECT r.id, t.title, r.summary_response, r.started_at
         FROM task_runs r LEFT JOIN tasks t ON t.id = r.task_id
         WHERE r.workspace_id = ? ORDER BY r.started_at DESC LIMIT ?",
    )
    .bind(&workspace_id)
    .bind(per_source)
    .fetch_all(&state.db)
    .await?;
    for (id, title, summary, started_at) in runs {
        hits.push(SearchHit {
            link: format!("/runs/{}", urlencode(&id)),
            id,
            kind: "run".into(),
            title: title.unwrap_or_else(|| "Execução".into()),
            snippet: summary.unwrap_or_else(|| "sem resumo".into()),
            updated_at: Some(started_at),
        });
    }

    let artifacts = sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT id, file_path, creation_reason, created_at FROM artifacts_provenance
         WHERE workspace_id = ? ORDER BY created_at DESC LIMIT ?",
    )
    .bind(&workspace_id)
    .bind(per_source)
    .fetch_all(&state.db)
    .await?;
    for (id, file_path, reason, created_at) in artifacts {
        hits.push(SearchHit {
            link: format!("/artifacts?id={}", urlencode(&id)),
            id,
            kind: "artifact".into(),
            title: file_path,
            snippet: reason,
            updated_at: Some(created_at),
        });
    }

    let comments = sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT m.id, m.task_id, m.body, m.created_at
         FROM messages m JOIN tasks t ON t.id = m.task_id
         WHERE t.workspace_id = ? AND m.kind NOT IN ('chat', 'turn', 'hitl_request')
         ORDER BY m.created_at DESC LIMIT ?",
    )
    .bind(&workspace_id)
    .bind(per_source)
    .fetch_all(&state.db)
    .await?;
    for (id, task_id, content, created_at) in comments {
        hits.push(SearchHit {
            link: format!("/kanban?view=board&task={}", urlencode(&task_id)),
            id,
            kind: "comment".into(),
            title: "Comentário em tarefa".into(),
            snippet: content,
            updated_at: Some(created_at),
        });
    }

    let chats = sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT id, title, COALESCE(agent, ''), updated_at FROM conversations
         WHERE workspace_id = ? AND channel = 'chat' ORDER BY updated_at DESC LIMIT ?",
    )
    .bind(&workspace_id)
    .bind(per_source)
    .fetch_all(&state.db)
    .await?;
    for (id, title, agent, updated_at) in chats {
        hits.push(SearchHit {
            link: format!("/chat?thread={}", urlencode(&id)),
            id,
            kind: "comment".into(),
            title,
            snippet: format!("conversa com @{agent}"),
            updated_at: Some(updated_at),
        });
    }

    let schedules = sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT id, name, command_or_prompt, created_at FROM schedules
         WHERE workspace_id = ? ORDER BY created_at DESC LIMIT ?",
    )
    .bind(&workspace_id)
    .bind(per_source)
    .fetch_all(&state.db)
    .await?;
    for (id, name, command, created_at) in schedules {
        hits.push(SearchHit {
            link: format!("/routines?routine={}", urlencode(&id)),
            id,
            kind: "schedule".into(),
            title: name,
            snippet: command,
            updated_at: Some(created_at),
        });
    }

    if let Ok(workspace) = crate::db::workspaces::get(&state.db, &workspace_id).await {
        for hit in recent_files(std::path::Path::new(&workspace.root_path), 8) {
            hits.push(hit);
        }
    }

    hits.sort_by(|left, right| {
        right
            .updated_at
            .as_deref()
            .unwrap_or_default()
            .cmp(left.updated_at.as_deref().unwrap_or_default())
    });
    hits.truncate(limit as usize);
    Ok(Json(hits))
}


pub async fn update(pool: &SqlitePool, id: &str, input: UpdateTaskInput) -> AppResult<Task> {
    let current = get(pool, id).await?;

    let title = match input.title {
        Some(value) if !value.trim().is_empty() => value.trim().to_string(),
        Some(_) => return Err(AppError::BadRequest("task title cannot be empty".into())),
        None => current.title,
    };
    let status = match input.status {
        Some(value) => validate_status(&value)?,
        None => current.status,
    };
    let phase = match input.phase {
        Some(value) if value.trim().is_empty() => None,
        Some(value) => Some(validate_phase(&value)?),
        None => current.phase,
    };
    let mode = match input.execution_mode {
        Some(value) => validate_mode(&value)?,
        None => current.execution_mode,
    };
    let effort = match input.effort_level {
        Some(value) => validate_effort(&value)?,
        None => current.effort_level,
    };
    let care = match input.care_level {
        Some(value) => validate_care(&value)?,
        None => current.care_level,
    };
    let tags_json = match input.tags {
        Some(tags) => serde_json::to_string(&tags).unwrap_or_else(|_| "[]".into()),
        None => current.tags_json,
    };

    sqlx::query(
        "UPDATE tasks SET
            title = ?, description = ?, status = ?, phase = ?, phase_status = ?,
            priority = ?, execution_mode = ?, assigned_agent = ?, preferred_model = ?, preferred_harness = ?,
            agent_override = ?, model_override = ?, effort_level = ?, care_level = ?, plan_markdown = ?, output_dir_path = ?, notify_email = ?,
            export_to_gdocs = ?, tags_json = ?, position_index = ?,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?",
    )
    .bind(&title)
    .bind(input.description.unwrap_or(current.description))
    .bind(&status)
    .bind(&phase)
    .bind(input.phase_status.or(current.phase_status))
    .bind(input.priority.unwrap_or(current.priority).clamp(1, 4))
    .bind(&mode)
    .bind(if input.clear_agent {
        None
    } else {
        input.assigned_agent.or(current.assigned_agent)
    })
    .bind(if input.clear_model {
        None
    } else {
        input.preferred_model.or(current.preferred_model)
    })
    .bind(if input.clear_harness {
        None
    } else {
        input.preferred_harness.or(current.preferred_harness)
    })
    .bind(if input.clear_agent {
        false
    } else {
        input.agent_override.unwrap_or(current.agent_override)
    })
    .bind(if input.clear_model {
        false
    } else {
        input.model_override.unwrap_or(current.model_override)
    })
    .bind(&effort)
    .bind(&care)
    .bind(if input.clear_plan {
        None
    } else {
        input.plan_markdown.or(current.plan_markdown)
    })
    .bind(input.output_dir_path.or(current.output_dir_path))
    .bind(input.notify_email.or(current.notify_email))
    .bind(input.export_to_gdocs.unwrap_or(current.export_to_gdocs))
    .bind(&tags_json)
    .bind(input.position_index.unwrap_or(current.position_index))
    .bind(id)
    .execute(pool)
    .await?;

    // Keep the fingerprint aligned with the edited content. If another active
    // task already owns it, surface a conflict instead of silently duplicating.
    let updated = get(pool, id).await?;
    let dedupe_key = task_dedupe::build_key(
        &updated.workspace_id,
        &updated.title,
        &updated.description,
        &updated.execution_mode,
        updated.assigned_agent.as_deref(),
        updated.preferred_model.as_deref(),
        &updated.effort_level,
        &updated.care_level,
        &updated.tags(),
        &updated.origin,
    );
    if let Err(error) = sqlx::query("UPDATE tasks SET dedupe_key = ? WHERE id = ?")
        .bind(&dedupe_key)
        .bind(id)
        .execute(pool)
        .await
    {
        if let sqlx::Error::Database(database) = &error
            && database.message().contains("UNIQUE constraint failed")
        {
            return Err(AppError::Conflict(
                "já existe uma tarefa ativa com os mesmos parâmetros".into(),
            ));
        }
        return Err(error.into());
    }

    get(pool, id).await
}

pub async fn move_to(
    pool: &SqlitePool,
    id: &str,
    status: &str,
    position_index: Option<i64>,
) -> AppResult<Task> {
    let status = validate_status(status)?;
    let position = match position_index {
        Some(position) => position,
        None => {
            sqlx::query_scalar(
                "SELECT COALESCE(MAX(position_index), -1) + 1 FROM tasks
                 WHERE workspace_id = (SELECT workspace_id FROM tasks WHERE id = ?)
                   AND status = ?",
            )
            .bind(id)
            .bind(&status)
            .fetch_one(pool)
            .await?
        }
    };

    let result = sqlx::query(
        "UPDATE tasks SET status = ?, position_index = ?,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?",
    )
    .bind(&status)
    .bind(position)
    .bind(id)
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("task {id}")));
    }
    if status == "TODO" {
        set_held(pool, id, false).await?;
    }

    get(pool, id).await
}

pub async fn set_held(pool: &SqlitePool, id: &str, held: bool) -> AppResult<()> {
    sqlx::query(
        "UPDATE tasks SET held = ?,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?",
    )
    .bind(held)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn fork(pool: &SqlitePool, id: &str) -> AppResult<Task> {
    fork_with_model(pool, id, None).await
}

/// Fork a task, optionally pinning the copy to a specific model. Used by the
/// comparative A/B swarm so both cards stay independent.
pub async fn fork_with_model(pool: &SqlitePool, id: &str, model: Option<&str>) -> AppResult<Task> {
    let source = get(pool, id).await?;
    let new_id = Uuid::now_v7().to_string();
    let suffix = model
        .map(|model| {
            let short = model
                .split('#')
                .next()
                .unwrap_or(model)
                .rsplit('/')
                .next()
                .unwrap_or(model);
            format!(" · {short}")
        })
        .unwrap_or_default();
    sqlx::query(
        "INSERT INTO tasks (
            id, workspace_id, title, description, status, phase, phase_status, priority,
            execution_mode, assigned_agent, preferred_model, agent_override, model_override,
            effort_level, care_level, plan_markdown, notify_email, export_to_gdocs, tags_json,
            position_index, forked_from_task_id, origin
         ) VALUES (?, ?, ?, ?, 'TODO', NULL, 'forked', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'MANUAL')",
    )
    .bind(&new_id)
    .bind(&source.workspace_id)
    .bind(format!("{} (fork{suffix})", source.title))
    .bind(&source.description)
    .bind(source.priority)
    .bind(&source.execution_mode)
    .bind(&source.assigned_agent)
    .bind(model.map(str::to_string).or(source.preferred_model.clone()))
    .bind(source.agent_override)
    .bind(model.is_some() || source.model_override)
    .bind(&source.effort_level)
    .bind(&source.care_level)
    .bind(&source.plan_markdown)
    .bind(&source.notify_email)
    .bind(source.export_to_gdocs)
    .bind(&source.tags_json)
    .bind(source.position_index)
    .bind(&source.id)
    .execute(pool)
    .await?;
    get(pool, &new_id).await
}

pub async fn set_plan(pool: &SqlitePool, id: &str, plan_markdown: &str) -> AppResult<Task> {
    sqlx::query(
        "UPDATE tasks SET plan_markdown = ?, status = 'TODO', phase = 'plan',
            phase_status = 'awaiting_approval',
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?",
    )
    .bind(plan_markdown)
    .bind(id)
    .execute(pool)
    .await?;
    get(pool, id).await
}

pub async fn set_phase_status(
    pool: &SqlitePool,
    id: &str,
    phase_status: Option<&str>,
    status: Option<&str>,
) -> AppResult<()> {
    let status = match status {
        Some(value) => Some(validate_status(value)?),
        None => None,
    };
    sqlx::query(
        "UPDATE tasks SET
            phase_status = COALESCE(?, phase_status),
            status = COALESCE(?, status),
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?",
    )
    .bind(phase_status)
    .bind(status)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Replace (not merge) the phase status. Used by explicit workflow
/// transitions such as moving a card back to Backlog or to Done.
pub async fn replace_phase_status(
    pool: &SqlitePool,
    id: &str,
    phase_status: Option<&str>,
    status: Option<&str>,
) -> AppResult<Task> {
    let status = match status {
        Some(value) => Some(validate_status(value)?),
        None => None,
    };
    sqlx::query(
        "UPDATE tasks SET
            phase_status = ?,
            status = COALESCE(?, status),
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?",
    )
    .bind(phase_status)
    .bind(status)
    .bind(id)
    .execute(pool)
    .await?;
    get(pool, id).await
}

pub async fn set_phase(pool: &SqlitePool, id: &str, phase: Option<&str>) -> AppResult<()> {
    sqlx::query(
        "UPDATE tasks SET phase = ?,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?",
    )
    .bind(phase)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn set_last_run(
    pool: &SqlitePool,
    id: &str,
    run_id: Option<&str>,
    status: Option<&str>,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE tasks SET last_run_id = ?, last_run_status = ?,
            last_run_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?",
    )
    .bind(run_id)
    .bind(status)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn set_blocked_reason(
    pool: &SqlitePool,
    id: &str,
    reason: Option<&str>,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE tasks SET blocked_reason = ?,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?",
    )
    .bind(reason)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn set_cancelled(pool: &SqlitePool, id: &str) -> AppResult<Task> {
    sqlx::query(
        "UPDATE tasks SET status = 'CANCELLED', phase_status = 'cancelled',
            cancelled_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?",
    )
    .bind(id)
    .execute(pool)
    .await?;
    get(pool, id).await
}

pub async fn set_output_dir(pool: &SqlitePool, id: &str, output_dir: &str) -> AppResult<()> {
    sqlx::query(
        "UPDATE tasks SET output_dir_path = ?,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?",
    )
    .bind(output_dir)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn delete(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let result = sqlx::query("DELETE FROM tasks WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("task {id}")));
    }
    Ok(())
}

#[allow(dead_code)]
pub async fn count_artifacts(pool: &SqlitePool, task_id: &str) -> AppResult<i64> {
    Ok(
        sqlx::query_scalar("SELECT COUNT(*) FROM artifacts_provenance WHERE task_id = ?")
            .bind(task_id)
            .fetch_one(pool)
            .await?,
    )
}

use opendesk_engine::run_timeline::TimelineInput;
use opendesk_storage::repos::runs::TaskRun;

use sqlx::SqlitePool;

pub(super) async fn load_run_inputs(
    pool: &SqlitePool,
    run: &TaskRun,
    workspace: Option<&opendesk_storage::repos::workspaces::Workspace>,
) -> Vec<opendesk_engine::run_timeline::TimelineInput> {
    let mut inputs = Vec::new();
    let root = workspace.map(|value| value.root_path.clone());
    let mut push = |name: String,
                    path: String,
                    size: Option<u64>,
                    source: Option<String>,
                    trigger: Option<String>| {
        if inputs.iter().any(|item: &TimelineInput| item.path == path) {
            return;
        }
        let absolute = if std::path::Path::new(&path).is_absolute() {
            path.clone()
        } else if let Some(root) = &root {
            std::path::Path::new(root)
                .join(&path)
                .to_string_lossy()
                .to_string()
        } else {
            path.clone()
        };
        inputs.push(TimelineInput {
            name,
            path,
            absolute_path: absolute,
            size,
            source,
            trigger_type: trigger,
        });
    };

    if let Some(task_id) = run.task_id.as_deref()
        && let Ok(Some(raw)) = sqlx::query_scalar::<_, String>(
            "SELECT trigger_context_json FROM recipe_runs WHERE task_id = ? ORDER BY started_at DESC LIMIT 1",
        )
        .bind(task_id)
        .fetch_optional(pool)
        .await
        && let Ok(context) = serde_json::from_str::<serde_json::Value>(&raw)
    {
        if let Some(items) = context.get("attachments").and_then(|value| value.as_array()) {
            for item in items {
                if let Some(path) = item.as_str() {
                    let name = std::path::Path::new(path)
                        .file_name()
                        .and_then(|value| value.to_str())
                        .unwrap_or(path)
                        .to_string();
                    push(name, path.to_string(), None, context.get("source_id").and_then(|value| value.as_str()).map(str::to_string), Some("attachment".into()));
                }
            }
        }
        if let Some(path) = context.get("file_path").and_then(|value| value.as_str()) {
            let name = context
                .get("file_name")
                .and_then(|value| value.as_str())
                .unwrap_or_else(|| {
                    std::path::Path::new(path)
                        .file_name()
                        .and_then(|value| value.to_str())
                        .unwrap_or(path)
                })
                .to_string();
            let size = context.get("file_size").and_then(|value| value.as_u64());
            push(
                name,
                path.to_string(),
                size,
                context
                    .get("source_name")
                    .or_else(|| context.get("recipe_id"))
                    .and_then(|value| value.as_str())
                    .map(str::to_string),
                context
                    .get("trigger_type")
                    .and_then(|value| value.as_str())
                    .map(str::to_string),
            );
        }
    }

    drop(push);

    if inputs.is_empty()
        && let Some(task_id) = run.task_id.as_deref()
        && let Ok(task) = opendesk_storage::repos::tasks::get(pool, task_id).await
        && let Some(marker) = task.description.rfind("## Arquivos de entrada")
    {
        for line in task.description[marker..].lines().skip(1) {
            if line.trim_start().starts_with("## ") {
                break;
            }
            let trimmed = line.trim_start();
            let Some(rest) = trimmed.strip_prefix("- `") else {
                continue;
            };
            let Some(end) = rest.find('`') else {
                continue;
            };
            let path = &rest[..end];
            if path.is_empty() {
                continue;
            }
            let name = std::path::Path::new(path)
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or(path)
                .to_string();
            let absolute = if std::path::Path::new(path).is_absolute() {
                path.to_string()
            } else if let Some(root) = &root {
                std::path::Path::new(root)
                    .join(path)
                    .to_string_lossy()
                    .to_string()
            } else {
                path.to_string()
            };
            inputs.push(TimelineInput {
                name,
                path: path.to_string(),
                absolute_path: absolute,
                size: None,
                source: Some("task".into()),
                trigger_type: Some("attachment".into()),
            });
        }
    }

    inputs
}

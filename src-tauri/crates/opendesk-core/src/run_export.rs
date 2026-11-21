//! Portable session export: Markdown, HTML and JSON derived from a run timeline.

use crate::{
    db::{
        artifacts, queue,
        runs::{self, TaskRun},
        tasks, workspaces,
    },
    error::{AppError, AppResult},
    run_timeline::{RunAttempt, RunTimeline, TimelineArtifact, timeline_from_events},
};
use sqlx::SqlitePool;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::SystemTime,
};

/// Parsed JSONL cache for run timelines.
///
/// The UI polls the timeline every second while a run is active. Re-reading and
/// re-parsing the whole log on each poll makes a long run quadratic, so the
/// cache keeps the parsed events and reads only the bytes appended since the
/// previous call. Logs are append-only; any shrink or rewrite rebuilds.
#[derive(Default)]
struct LogCache {
    /// Byte offset after the last complete line already parsed.
    len: u64,
    modified: Option<SystemTime>,
    events: Vec<serde_json::Value>,
}

const LOG_CACHE_LIMIT: usize = 32;

fn log_cache() -> &'static Mutex<HashMap<PathBuf, LogCache>> {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, LogCache>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn parse_lines(text: &str, into: &mut Vec<serde_json::Value>) {
    for line in text.lines() {
        if let Ok(value) = serde_json::from_str(line) {
            into.push(value);
        }
    }
}

fn read_bytes_from(path: &Path, offset: u64) -> Option<Vec<u8>> {
    use std::io::{Read, Seek, SeekFrom};
    let mut file = std::fs::File::open(path).ok()?;
    if offset > 0 {
        file.seek(SeekFrom::Start(offset)).ok()?;
    }
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).ok()?;
    Some(bytes)
}

/// Events of an append-only JSONL log, reading only what changed since the
/// previous call. Returns `None` when the file cannot be opened.
fn cached_events(path: &Path) -> Option<Vec<serde_json::Value>> {
    let metadata = std::fs::metadata(path).ok()?;
    let len = metadata.len();
    let modified = metadata.modified().ok();
    let mut cache = log_cache()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    if let Some(entry) = cache.get(path)
        && entry.len == len
        && entry.modified == modified
    {
        return Some(entry.events.clone());
    }

    let previous = cache.remove(path);
    // Only trust the cache for a strict append. A rewrite with the same or a
    // larger size is not detectable from metadata alone.
    let (mut events, base) = match previous {
        Some(entry) if entry.len < len => (entry.events, entry.len),
        _ => (Vec::new(), 0),
    };
    let bytes = read_bytes_from(path, base)?;
    let text = String::from_utf8_lossy(&bytes);
    // A writer may still be in the middle of the last line; keep it for later.
    let complete = text.rfind('\n').map_or(0, |index| index + 1);
    parse_lines(&text[..complete], &mut events);
    let consumed = base + complete as u64;

    if cache.len() >= LOG_CACHE_LIMIT {
        cache.clear();
    }
    cache.insert(
        path.to_path_buf(),
        LogCache {
            len: consumed,
            modified,
            events: events.clone(),
        },
    );
    Some(events)
}

pub async fn load_timeline(pool: &SqlitePool, id: &str) -> AppResult<RunTimeline> {
    let run = runs::get(pool, id).await?;
    let mut timeline = build_timeline(pool, run.clone()).await?;
    timeline.root_run_id = Some(run.id.clone());
    timeline.attempts = vec![attempt_from_run(&run)];
    Ok(timeline)
}

/// One logical execution can contain several attempts (follow-ups, retries).
/// Merge all attempts of the task into a single chronological timeline while
/// keeping per-attempt summaries for the header.
pub async fn load_thread_timeline(pool: &SqlitePool, id: &str) -> AppResult<RunTimeline> {
    let requested = runs::get(pool, id).await?;
    let mut attempts = match requested.task_id.as_deref() {
        Some(task_id) => runs::list(pool, &requested.workspace_id, Some(task_id))
            .await
            .unwrap_or_else(|_| vec![requested.clone()]),
        None => vec![requested.clone()],
    };
    attempts.sort_by(|left, right| {
        left.started_at
            .cmp(&right.started_at)
            .then(left.attempt_number.cmp(&right.attempt_number))
    });
    if attempts.is_empty() {
        attempts.push(requested);
    }

    let mut merged = RunTimeline {
        root_run_id: attempts.first().map(|run| run.id.clone()),
        attempts: attempts.iter().map(attempt_from_run).collect(),
        ..Default::default()
    };
    let mut first_started: Option<String> = None;
    let mut last_completed: Option<String> = None;
    let mut events_count = 0;
    let mut duration_ms = 0;
    let mut log_missing = false;

    for run in &attempts {
        let mut part = build_timeline(pool, run.clone()).await?;
        for event in &mut part.events {
            event.run_id = Some(run.id.clone());
            event.attempt_number = Some(run.attempt_number);
        }
        for step in &mut part.steps {
            step.attempt_number = Some(run.attempt_number);
        }
        events_count += part.events_count;
        duration_ms += part.duration_ms;
        log_missing |= part.log_missing;
        if first_started.is_none() {
            first_started = part.started_at.clone();
        }
        if part.completed_at.is_some() {
            last_completed = part.completed_at.clone();
        }
        merged.events.append(&mut part.events);
        merged.steps.append(&mut part.steps);
        merged.artifacts.append(&mut part.artifacts);
        if !part.result.text.trim().is_empty() {
            merged.result = part.result;
        }
        if !part.thinking.text.trim().is_empty() {
            merged.thinking = part.thinking;
        }
        if !part.executed.tools.is_empty() {
            merged.executed.tools.extend(part.executed.tools);
        }
        merged.workspace_root = part.workspace_root.or(merged.workspace_root);
        merged.output_directory = part.output_directory.or(merged.output_directory);
        merged.output_directory_relative = part
            .output_directory_relative
            .or(merged.output_directory_relative);
    }

    merged.events.sort_by(|left, right| {
        left.started_at
            .as_deref()
            .unwrap_or_default()
            .cmp(right.started_at.as_deref().unwrap_or_default())
    });
    merged.steps.sort_by(|left, right| {
        left.started_at
            .as_deref()
            .unwrap_or_default()
            .cmp(right.started_at.as_deref().unwrap_or_default())
    });
    merged.executed.tools.sort_by(|left, right| {
        left.started_at
            .as_deref()
            .unwrap_or_default()
            .cmp(right.started_at.as_deref().unwrap_or_default())
    });
    merged.run = attempts.last().cloned();
    merged.status = merged
        .run
        .as_ref()
        .map(|run| run.status.clone())
        .unwrap_or_default();
    merged.started_at = first_started;
    merged.completed_at = last_completed;
    merged.duration_ms = duration_ms;
    merged.events_count = events_count;
    merged.log_missing = log_missing;
    Ok(merged)
}

fn attempt_from_run(run: &TaskRun) -> RunAttempt {
    RunAttempt {
        id: run.id.clone(),
        attempt_number: run.attempt_number,
        status: run.status.clone(),
        phase: run.phase.clone(),
        started_at: run.started_at.clone(),
        completed_at: run.completed_at.clone(),
        duration_ms: run.duration_ms,
        tokens_input: run.tokens_input,
        tokens_output: run.tokens_output,
        cost_usd: run.estimated_cost_usd,
        error_message: run.error_message.clone(),
    }
}

async fn load_run_inputs(
    pool: &SqlitePool,
    run: &TaskRun,
    workspace: Option<&crate::db::workspaces::Workspace>,
) -> Vec<crate::run_timeline::TimelineInput> {
    use crate::run_timeline::TimelineInput;
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
        && let Ok(task) = crate::db::tasks::get(pool, task_id).await
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

async fn build_timeline(pool: &SqlitePool, run: TaskRun) -> AppResult<RunTimeline> {
    let path = run.log_path.clone();
    let events = path
        .as_deref()
        .and_then(|path| cached_events(Path::new(path)));
    let log_missing = events.is_none();
    let mut timeline =
        timeline_from_events(Some(run.clone()), events.unwrap_or_default(), log_missing);

    let workspace = workspaces::get(pool, &run.workspace_id).await.ok();
    timeline.inputs = load_run_inputs(pool, &run, workspace.as_ref()).await;
    if let Some(workspace) = &workspace {
        timeline.workspace_root = Some(workspace.root_path.clone());
    }
    let mut output_directory = match run.task_id.as_deref() {
        Some(task_id) => tasks::get(pool, task_id)
            .await
            .ok()
            .and_then(|task| task.output_dir_path),
        None => None,
    };
    if output_directory.is_none()
        && let Some(job_id) = run.queue_job_id.as_deref()
    {
        output_directory = queue::get(pool, job_id)
            .await
            .ok()
            .and_then(|job| job.output_dir);
    }
    if let Some(output) = &output_directory {
        timeline.output_directory = Some(output.clone());
        if let Some(workspace) = &workspace {
            timeline.output_directory_relative = output
                .strip_prefix(&workspace.root_path)
                .map(str::to_string);
        }
    }
    timeline.artifacts = artifacts::list_by_run(pool, &run.id, 200)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|artifact| TimelineArtifact {
            id: artifact.id,
            file_path: artifact.file_path,
            absolute_path: artifact.absolute_path,
            action_type: artifact.action_type,
            created_at: artifact.created_at,
        })
        .collect();
    Ok(timeline)
}

fn format_duration(ms: i64) -> String {
    if ms < 1000 {
        format!("{ms} ms")
    } else if ms < 60_000 {
        format!("{:.1} s", ms as f64 / 1000.0)
    } else {
        format!("{}m {:02}s", ms / 60_000, (ms % 60_000) / 1000)
    }
}

fn task_title(timeline: &RunTimeline) -> &str {
    timeline
        .run
        .as_ref()
        .and_then(|run| run.task_title.as_deref())
        .unwrap_or("Execução")
}

pub fn render_markdown(timeline: &RunTimeline) -> String {
    let mut out = String::new();
    let run = timeline.run.as_ref();
    out.push_str(&format!("# {}\n\n", task_title(timeline)));
    if let Some(run) = run {
        out.push_str(&format!("- **Run:** `{}`\n", run.id));
        out.push_str(&format!("- **Status:** {}\n", run.status));
        out.push_str(&format!("- **Agente:** {}\n", run.agent_name));
        out.push_str(&format!("- **Modelo:** {}\n", run.model_id));
        out.push_str(&format!("- **Início:** {}\n", run.started_at));
        if let Some(completed) = &run.completed_at {
            out.push_str(&format!("- **Fim:** {completed}\n"));
        }
        out.push_str(&format!(
            "- **Duração:** {}\n",
            format_duration(timeline.duration_ms)
        ));
        out.push_str(&format!(
            "- **Tokens:** {} entrada / {} saída\n",
            run.tokens_input, run.tokens_output
        ));
        out.push_str(&format!("- **Custo:** US$ {:.4}\n", run.estimated_cost_usd));
        if let Some(error) = &run.error_message {
            out.push_str(&format!("- **Erro:** {error}\n"));
        }
    }
    if let Some(output) = &timeline.output_directory_relative {
        out.push_str(&format!("- **Saída:** `{output}`\n"));
    }
    out.push('\n');

    let result = if timeline.result.text.trim().is_empty() {
        run.and_then(|run| run.summary_response.as_deref())
            .unwrap_or("")
    } else {
        timeline.result.text.trim()
    };
    if !result.is_empty() {
        out.push_str("## Resultado\n\n");
        out.push_str(result);
        out.push_str("\n\n");
    }

    if !timeline.steps.is_empty() {
        out.push_str("## Passos\n\n");
        out.push_str("| # | Agente | Modelo | Duração | Tokens | Custo |\n");
        out.push_str("|---|---|---|---|---|---|\n");
        for step in &timeline.steps {
            out.push_str(&format!(
                "| {} | {} | {} | {} | {}/{} | US$ {:.4} |\n",
                step.index,
                step.agent.as_deref().unwrap_or("—"),
                step.model.as_deref().unwrap_or("—"),
                format_duration(step.duration_ms),
                step.tokens_input,
                step.tokens_output,
                step.cost_usd
            ));
        }
        out.push('\n');
    }

    if !timeline.thinking.text.trim().is_empty() {
        out.push_str("## Raciocínio\n\n");
        out.push_str(timeline.thinking.text.trim());
        out.push_str("\n\n");
    }

    if !timeline.executed.tools.is_empty() {
        out.push_str("## Ferramentas\n\n");
        for tool in &timeline.executed.tools {
            out.push_str(&format!(
                "- **{}** — {} ({}, {})\n",
                tool.name,
                tool.summary,
                tool.status,
                format_duration(tool.duration_ms)
            ));
        }
        out.push('\n');
    }

    if !timeline.artifacts.is_empty() {
        out.push_str("## Artefatos\n\n");
        for artifact in &timeline.artifacts {
            out.push_str(&format!(
                "- `{}` — {} ({})\n",
                artifact.file_path, artifact.action_type, artifact.created_at
            ));
        }
        out.push('\n');
    }

    out.push_str(&format!(
        "---\n\nGerado pelo OpenDesk · {} eventos · {}\n",
        timeline.events_count,
        chrono::Utc::now().to_rfc3339()
    ));
    out
}

pub fn render_html(timeline: &RunTimeline) -> String {
    use html_escape::encode_text;
    let run = timeline.run.as_ref();
    let result = if timeline.result.text.trim().is_empty() {
        run.and_then(|run| run.summary_response.as_deref())
            .unwrap_or("")
    } else {
        timeline.result.text.trim()
    };
    let mut steps = String::new();
    if !timeline.steps.is_empty() {
        steps.push_str("<h2>Passos</h2><table><thead><tr><th>#</th><th>Agente</th><th>Modelo</th><th>Duração</th><th>Tokens</th><th>Custo</th></tr></thead><tbody>");
        for step in &timeline.steps {
            steps.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}/{}</td><td>US$ {:.4}</td></tr>",
                step.index,
                encode_text(step.agent.as_deref().unwrap_or("—")),
                encode_text(step.model.as_deref().unwrap_or("—")),
                format_duration(step.duration_ms),
                step.tokens_input,
                step.tokens_output,
                step.cost_usd
            ));
        }
        steps.push_str("</tbody></table>");
    }
    let mut tools = String::new();
    if !timeline.executed.tools.is_empty() {
        tools.push_str("<h2>Ferramentas</h2><ul>");
        for tool in &timeline.executed.tools {
            tools.push_str(&format!(
                "<li><strong>{}</strong> — {} <span class=\"muted\">({}, {})</span></li>",
                encode_text(&tool.name),
                encode_text(&tool.summary),
                encode_text(&tool.status),
                format_duration(tool.duration_ms)
            ));
        }
        tools.push_str("</ul>");
    }
    let mut artifacts = String::new();
    if !timeline.artifacts.is_empty() {
        artifacts.push_str("<h2>Artefatos</h2><ul>");
        for artifact in &timeline.artifacts {
            artifacts.push_str(&format!(
                "<li><code>{}</code> — {} <span class=\"muted\">{}</span></li>",
                encode_text(&artifact.file_path),
                encode_text(&artifact.action_type),
                encode_text(&artifact.created_at)
            ));
        }
        artifacts.push_str("</ul>");
    }
    let thinking = if timeline.thinking.text.trim().is_empty() {
        String::new()
    } else {
        format!(
            "<h2>Raciocínio</h2><pre>{}</pre>",
            encode_text(timeline.thinking.text.trim())
        )
    };
    let error = run
        .and_then(|run| run.error_message.as_deref())
        .map(|error| format!("<p class=\"error\">{}</p>", encode_text(error)))
        .unwrap_or_default();
    let meta = run
        .map(|run| {
            format!(
                "<dl><dt>Run</dt><dd><code>{}</code></dd><dt>Status</dt><dd>{}</dd><dt>Agente</dt><dd>{}</dd><dt>Modelo</dt><dd>{}</dd><dt>Duração</dt><dd>{}</dd><dt>Tokens</dt><dd>{} / {}</dd><dt>Custo</dt><dd>US$ {:.4}</dd></dl>",
                encode_text(&run.id),
                encode_text(&run.status),
                encode_text(&run.agent_name),
                encode_text(&run.model_id),
                format_duration(timeline.duration_ms),
                run.tokens_input,
                run.tokens_output,
                run.estimated_cost_usd
            )
        })
        .unwrap_or_default();
    format!(
        "<!doctype html><html lang=\"pt-BR\"><head><meta charset=\"utf-8\"><title>{title}</title><style>body{{font:14px/1.55 system-ui,sans-serif;max-width:900px;margin:40px auto;padding:0 20px;color:#18181b}}h1{{font-size:22px}}h2{{font-size:15px;margin-top:28px;border-bottom:1px solid #e4e4e7;padding-bottom:4px}}table{{border-collapse:collapse;width:100%}}th,td{{border-bottom:1px solid #e4e4e7;padding:6px 8px;text-align:left;font-size:12px}}pre{{background:#f4f4f5;padding:12px;border-radius:8px;white-space:pre-wrap;word-break:break-word}}.muted{{color:#71717a}}.error{{color:#b91c1c;background:#fef2f2;padding:10px;border-radius:8px}}dl{{display:grid;grid-template-columns:max-content 1fr;gap:2px 14px;font-size:12px}}dt{{color:#71717a}}dd{{margin:0}}</style></head><body><h1>{title}</h1>{meta}{error}{result}{steps}{thinking}{tools}{artifacts}<p class=\"muted\">Gerado pelo OpenDesk · {events} eventos</p></body></html>",
        title = encode_text(task_title(timeline)),
        meta = meta,
        error = error,
        result = if result.is_empty() {
            String::new()
        } else {
            format!("<h2>Resultado</h2><pre>{}</pre>", encode_text(result))
        },
        steps = steps,
        thinking = thinking,
        tools = tools,
        artifacts = artifacts,
        events = timeline.events_count
    )
}

pub fn render_json(timeline: &RunTimeline) -> AppResult<String> {
    serde_json::to_string_pretty(timeline)
        .map_err(|error| AppError::Internal(format!("could not encode export: {error}")))
}

pub fn render(
    timeline: &RunTimeline,
    format: &str,
) -> AppResult<(String, &'static str, &'static str)> {
    match format.trim().to_lowercase().as_str() {
        "md" | "markdown" => Ok((
            render_markdown(timeline),
            "text/markdown; charset=utf-8",
            "md",
        )),
        "html" | "htm" => Ok((render_html(timeline), "text/html; charset=utf-8", "html")),
        "json" => Ok((
            render_json(timeline)?,
            "application/json; charset=utf-8",
            "json",
        )),
        other => Err(AppError::BadRequest(format!(
            "formato de export inválido '{other}'; use md, html ou json"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::runs::{CreateRunInput, RunUsage};
    use crate::test_support;
    use std::io::Write;

    #[test]
    fn timeline_cache_reads_only_complete_appended_lines() {
        let dir = tempfile::TempDir::new().expect("temp");
        let path = dir.path().join("run.jsonl");
        std::fs::write(
            &path,
            "{\"created\":1,\"type\":\"a\"}\n{\"created\":2,\"type\":\"b\"}\n",
        )
        .expect("log");
        assert_eq!(cached_events(&path).expect("events").len(), 2);

        // A half-written line is not consumed.
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .expect("append");
        write!(file, "{{\"created\":3,").expect("partial");
        file.flush().expect("flush");
        drop(file);
        assert_eq!(cached_events(&path).expect("events").len(), 2);

        // Completing the line appends exactly one event.
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .expect("append");
        writeln!(file, "\"type\":\"c\"}}").expect("complete");
        drop(file);
        assert_eq!(cached_events(&path).expect("events").len(), 3);

        // A smaller rewrite forces a rebuild.
        std::fs::write(&path, "{\"created\":9,\"type\":\"z\"}\n").expect("rewrite");
        let rebuilt = cached_events(&path).expect("events");
        assert_eq!(rebuilt.len(), 1);
        assert_eq!(rebuilt[0]["type"], "z");
    }

    #[tokio::test]
    async fn exposes_recipe_trigger_files_as_run_inputs() {
        let ctx = test_support::context().await;
        let yaml = "id: inputs-recipe\nname: Inputs\ntrigger:\n  type: MANUAL\n  config: {}\nflow:\n  - type: step\n    id: s1\n    name: Passo\n    prompt: oi\n";
        let recipe = crate::recipes::parser::parse(yaml).expect("recipe");
        sqlx::query(
            "INSERT INTO recipes_index (id, workspace_id, name, file_path, trigger_type)
             VALUES ('inputs-recipe', ?, 'Inputs', '/tmp/inputs.yaml', 'MANUAL')",
        )
        .bind(&ctx.workspace_id)
        .execute(&ctx.db)
        .await
        .expect("index");
        let task = crate::db::tasks::create(
            &ctx.db,
            &ctx.workspace_id,
            crate::db::tasks::CreateTaskInput {
                title: "Entrada".into(),
                description: "Processe o arquivo".into(),
                execution_mode: Some("BUILD_DIRECT".into()),
                priority: Some(2),
                assigned_agent: None,
                preferred_model: None,
                preferred_harness: None,
                effort_level: None,
                care_level: None,
                notify_email: None,
                export_to_gdocs: false,
                tags: vec![],
                origin: None,
                parent_task_id: None,
                dedupe: Some(false),
            },
        )
        .await
        .expect("task");
        crate::recipes::store::create_run(
            &ctx.db,
            &recipe,
            &ctx.workspace_id,
            Some(&task.id),
            &serde_json::json!({
                "file_path": "input/dropzone/relatorio.pdf",
                "file_name": "relatorio.pdf",
                "file_size": 2048,
                "source_name": "Pasta monitorada",
                "trigger_type": "FOLDER_WATCH",
                "attachments": ["input/extra.txt"]
            }),
            "/tmp/out",
            false,
        )
        .await
        .expect("recipe run");
        let run = crate::db::runs::create(
            &ctx.db,
            CreateRunInput {
                id: "run-inputs".into(),
                queue_job_id: None,
                workspace_id: ctx.workspace_id.clone(),
                task_id: Some(task.id.clone()),
                schedule_id: None,
                parent_run_id: None,
                phase: Some("build".into()),
                agent_name: "build".into(),
                execution_mode: "build".into(),
                provider_id: None,
                model_id: "glm".into(),
                effort_level: "medium".into(),
                pricing_snapshot_json: None,
                log_path: None,
            },
        )
        .await
        .expect("run");
        let workspace = crate::db::workspaces::get(&ctx.db, &ctx.workspace_id)
            .await
            .expect("workspace");
        let inputs = load_run_inputs(&ctx.db, &run, Some(&workspace)).await;
        assert_eq!(inputs.len(), 2);
        let report = inputs
            .iter()
            .find(|input| input.name == "relatorio.pdf")
            .expect("report");
        assert_eq!(report.size, Some(2048));
        assert_eq!(report.trigger_type.as_deref(), Some("FOLDER_WATCH"));
        assert!(
            report
                .absolute_path
                .ends_with("input/dropzone/relatorio.pdf")
        );
        assert!(inputs.iter().any(|input| input.path == "input/extra.txt"));
    }

    #[tokio::test]
    async fn reads_task_description_inputs_when_no_recipe_context_exists() {
        let ctx = test_support::context().await;
        let task = crate::db::tasks::create(
            &ctx.db,
            &ctx.workspace_id,
            crate::db::tasks::CreateTaskInput {
                title: "Entrada manual".into(),
                description:
                    "Processe os arquivos.\n\n## Arquivos de entrada\n\n- `input/manual.pdf`\n"
                        .into(),
                execution_mode: Some("BUILD_DIRECT".into()),
                priority: Some(2),
                assigned_agent: None,
                preferred_model: None,
                preferred_harness: None,
                effort_level: None,
                care_level: None,
                notify_email: None,
                export_to_gdocs: false,
                tags: vec![],
                origin: None,
                parent_task_id: None,
                dedupe: Some(false),
            },
        )
        .await
        .expect("task");
        let run = crate::db::runs::create(
            &ctx.db,
            CreateRunInput {
                id: "run-task-inputs".into(),
                queue_job_id: None,
                workspace_id: ctx.workspace_id.clone(),
                task_id: Some(task.id.clone()),
                schedule_id: None,
                parent_run_id: None,
                phase: Some("build".into()),
                agent_name: "build".into(),
                execution_mode: "build".into(),
                provider_id: None,
                model_id: "glm".into(),
                effort_level: "medium".into(),
                pricing_snapshot_json: None,
                log_path: None,
            },
        )
        .await
        .expect("run");
        let workspace = crate::db::workspaces::get(&ctx.db, &ctx.workspace_id)
            .await
            .expect("workspace");
        let inputs = load_run_inputs(&ctx.db, &run, Some(&workspace)).await;
        assert_eq!(inputs.len(), 1);
        assert_eq!(inputs[0].name, "manual.pdf");
        assert_eq!(inputs[0].source.as_deref(), Some("task"));
    }

    #[tokio::test]
    async fn renders_all_formats_from_a_run() {
        let ctx = test_support::context().await;
        crate::db::runs::create(
            &ctx.db,
            CreateRunInput {
                id: "run-export".into(),
                queue_job_id: None,
                workspace_id: ctx.workspace_id.clone(),
                task_id: None,
                schedule_id: None,
                parent_run_id: None,
                phase: None,
                agent_name: "build".into(),
                execution_mode: "build".into(),
                provider_id: Some("opencode-go".into()),
                model_id: "glm-5.3".into(),
                effort_level: "high".into(),
                pricing_snapshot_json: None,
                log_path: None,
            },
        )
        .await
        .expect("run");
        crate::db::runs::finish(
            &ctx.db,
            "run-export",
            "SUCCESS",
            &RunUsage::default(),
            1200,
            Some("resultado final"),
            None,
        )
        .await
        .expect("finish");

        let timeline = load_timeline(&ctx.db, "run-export")
            .await
            .expect("timeline");
        let md = render_markdown(&timeline);
        assert!(md.contains("resultado final"));
        assert!(md.contains("run-export"));
        let html = render_html(&timeline);
        assert!(html.starts_with("<!doctype html>"));
        assert!(html.contains("resultado final"));
        let (body, mime, ext) = render(&timeline, "json").expect("json");
        assert_eq!(mime, "application/json; charset=utf-8");
        assert_eq!(ext, "json");
        assert!(body.contains("\"run-export\""));
    }

    #[test]
    fn rejects_unknown_formats() {
        let timeline = RunTimeline::default();
        assert!(render(&timeline, "pdf").is_err());
    }
}

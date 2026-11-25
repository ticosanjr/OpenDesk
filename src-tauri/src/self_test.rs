//! "Testar tudo": deterministic, offline-capable diagnostics for the app.

use crate::{
    app_state::AppState,
    db::{recipe_files, schedules, script_tools, tasks},
    error::{AppError, AppResult},
    recipes::{engine, parser, store},
    script_tools as executor,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::SqlitePool;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use uuid::Uuid;

static RUNNING: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum SelfTestMode {
    #[default]
    Fast,
    FullAi,
}

#[derive(Debug, Clone, Serialize)]
pub struct SelfTestStep {
    pub id: String,
    pub label: String,
    pub status: String,
    pub duration_ms: u64,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SelfTestReport {
    pub ok: bool,
    pub mode: SelfTestMode,
    pub total_ms: u64,
    pub steps: Vec<SelfTestStep>,
}

#[derive(Default)]
struct Cleanup {
    task: Option<String>,
    schedule: Option<String>,
    recipe: Option<String>,
    script: Option<String>,
}

fn step(
    id: &str,
    label: &str,
    status: &str,
    started: Instant,
    detail: impl Into<String>,
) -> SelfTestStep {
    SelfTestStep {
        id: id.into(),
        label: label.into(),
        status: status.into(),
        duration_ms: started.elapsed().as_millis() as u64,
        detail: detail.into(),
    }
}

pub async fn run(
    state: &Arc<AppState>,
    workspace_id: &str,
    mode: SelfTestMode,
) -> AppResult<SelfTestReport> {
    if RUNNING.swap(true, Ordering::SeqCst) {
        return Err(AppError::Conflict(
            "um teste completo já está em execução".into(),
        ));
    }
    let result = run_inner(state, workspace_id, mode).await;
    RUNNING.store(false, Ordering::SeqCst);
    result
}

async fn run_inner(
    state: &Arc<AppState>,
    workspace_id: &str,
    mode: SelfTestMode,
) -> AppResult<SelfTestReport> {
    let total = Instant::now();
    let tag = format!("__selftest_{}__", Uuid::new_v4().simple());
    let mut cleanup = Cleanup::default();
    let mut steps = Vec::new();

    let started = Instant::now();
    let check = sqlx::query_scalar::<_, String>("PRAGMA quick_check(1)")
        .fetch_one(&state.db)
        .await;
    let journal = sqlx::query_scalar::<_, String>("PRAGMA journal_mode")
        .fetch_one(&state.db)
        .await
        .unwrap_or_default();
    match check {
        Ok(value) if value.eq_ignore_ascii_case("ok") && journal.eq_ignore_ascii_case("wal") => {
            steps.push(step(
                "db",
                "Banco SQLite",
                "pass",
                started,
                format!("quick_check ok · journal {journal}"),
            ));
        }
        Ok(value) => steps.push(step(
            "db",
            "Banco SQLite",
            "fail",
            started,
            format!("quick_check: {value} · journal {journal}"),
        )),
        Err(error) => steps.push(step(
            "db",
            "Banco SQLite",
            "fail",
            started,
            error.to_string(),
        )),
    }

    let started = Instant::now();
    let daemon = state.daemon.status().await;
    let engine_status = if daemon.online {
        "pass"
    } else if mode == SelfTestMode::Fast {
        "warn"
    } else {
        "fail"
    };
    steps.push(step(
        "engine",
        "Motor OpenCode",
        engine_status,
        started,
        if daemon.online {
            format!(
                "online {} · {}",
                daemon.version.unwrap_or_else(|| "?".into()),
                daemon.state
            )
        } else {
            daemon.message.unwrap_or_else(|| "offline".into())
        },
    ));

    let started = Instant::now();
    match create_task(&state.db, workspace_id, &tag).await {
        Ok(task) => {
            cleanup.task = Some(task.id.clone());
            let transition = tasks::move_to(&state.db, &task.id, "TODO", None).await;
            let persisted = tasks::get(&state.db, &task.id).await;
            let basic_ok = transition.is_ok()
                && persisted
                    .as_ref()
                    .map(|value| value.title == task.title)
                    .unwrap_or(false);
            if mode == SelfTestMode::FullAi && basic_ok && daemon.online {
                match run_task_ai(state, &task.id).await {
                    Ok(detail) => steps.push(step("task", "Tarefa", "pass", started, detail)),
                    Err(error) => steps.push(step("task", "Tarefa", "fail", started, error)),
                }
            } else if basic_ok {
                steps.push(step(
                    "task",
                    "Tarefa",
                    "pass",
                    started,
                    "criada e transicionada",
                ));
            } else {
                steps.push(step(
                    "task",
                    "Tarefa",
                    "fail",
                    started,
                    "não persistiu ou não transicionou",
                ));
            }
        }
        Err(error) => steps.push(step("task", "Tarefa", "fail", started, error.to_string())),
    }

    let started = Instant::now();
    match create_routine(&state.db, workspace_id, &tag).await {
        Ok(schedule) => {
            cleanup.schedule = Some(schedule.id.clone());
            let next = crate::queue_scheduler::cron_engine::next_occurrence(
                &schedule.cron_expression,
                &schedule.timezone,
                chrono::Utc::now(),
            );
            let prompt_ok = !schedule.command_or_prompt.trim().is_empty();
            match next {
                Ok(next) if prompt_ok => steps.push(step(
                    "routine",
                    "Rotina",
                    "pass",
                    started,
                    format!("dry-run ok · próximo {}", next.to_rfc3339()),
                )),
                Ok(_) => steps.push(step("routine", "Rotina", "fail", started, "prompt vazio")),
                Err(error) => steps.push(step(
                    "routine",
                    "Rotina",
                    "fail",
                    started,
                    format!("cron inválido: {error}"),
                )),
            }
        }
        Err(error) => steps.push(step(
            "routine",
            "Rotina",
            "fail",
            started,
            error.to_string(),
        )),
    }

    let started = Instant::now();
    let recipe_id = format!("{tag}-recipe");
    cleanup.recipe = Some(recipe_id.clone());
    match create_recipe(state, workspace_id, &recipe_id, mode).await {
        Ok(id) => steps.push(step(
            "recipe",
            "Receita",
            "pass",
            started,
            format!("{id} criada e dry-run ok"),
        )),
        Err(error) => steps.push(step("recipe", "Receita", "fail", started, error)),
    }

    let started = Instant::now();
    match create_script(&state.db, workspace_id, &tag).await {
        Ok(tool) => {
            cleanup.script = Some(tool.id.clone());
            match executor::execute(&state.db, &state.paths, workspace_id, &tool, &json!({})).await
            {
                Ok(result) if result.ok && result.stdout.contains("\"ok\":true") => {
                    steps.push(step(
                        "script_tool",
                        "Ferramenta (script MCP)",
                        "pass",
                        started,
                        format!(
                            "exit {} em {} ms",
                            result.exit_code.unwrap_or(0),
                            result.duration_ms
                        ),
                    ))
                }
                Ok(result) => steps.push(step(
                    "script_tool",
                    "Ferramenta (script MCP)",
                    "fail",
                    started,
                    format!("exit {:?}: {}", result.exit_code, result.stderr),
                )),
                Err(error) => steps.push(step(
                    "script_tool",
                    "Ferramenta (script MCP)",
                    "fail",
                    started,
                    error.to_string(),
                )),
            }
        }
        Err(error) => steps.push(step(
            "script_tool",
            "Ferramenta (script MCP)",
            "fail",
            started,
            error.to_string(),
        )),
    }

    let started = Instant::now();
    match crate::voice::get(&state.db).await {
        Ok(settings) => {
            let capabilities = crate::voice::capabilities(&settings);
            if capabilities.available {
                steps.push(step("voice", "Voz", "pass", started, "STT e TTS prontos"));
            } else {
                steps.push(step(
                    "voice",
                    "Voz",
                    "warn",
                    started,
                    capabilities
                        .install_hint
                        .or(capabilities.reason)
                        .unwrap_or_else(|| "provedor de voz opcional ausente".into()),
                ));
            }
        }
        Err(error) => steps.push(step("voice", "Voz", "warn", started, error.to_string())),
    }

    let started = Instant::now();
    match cleanup_all(state, workspace_id, &cleanup).await {
        Ok(()) => steps.push(step(
            "cleanup",
            "Limpeza",
            "pass",
            started,
            "registros temporários removidos",
        )),
        Err(error) => steps.push(step("cleanup", "Limpeza", "fail", started, error)),
    }

    let ok = !steps.iter().any(|step| step.status == "fail");
    Ok(SelfTestReport {
        ok,
        mode,
        total_ms: total.elapsed().as_millis() as u64,
        steps,
    })
}

async fn create_task(pool: &SqlitePool, workspace_id: &str, tag: &str) -> AppResult<tasks::Task> {
    tasks::create(
        pool,
        workspace_id,
        tasks::CreateTaskInput {
            title: format!("{tag} task"),
            description: "Teste automático do OpenDesk. Pode apagar.".into(),
            tags: vec!["selftest".into()],
            ..Default::default()
        },
    )
    .await
    .map_err(Into::into)
}

async fn run_task_ai(state: &Arc<AppState>, task_id: &str) -> Result<String, String> {
    let body = crate::api::tasks::DispatchBody {
        prompt_append: Some("Responda apenas OK".into()),
        ..Default::default()
    };
    if let Err(error) =
        crate::api::tasks::task_dispatch::dispatch_task_internal(state, task_id, body, None).await
    {
        return Err(format!("dispatch: {error}"));
    }
    for _ in 0..90 {
        tokio::time::sleep(Duration::from_millis(500)).await;
        let task = tasks::get(&state.db, task_id)
            .await
            .map_err(|error| error.to_string())?;
        if task.last_run_status.as_deref() == Some("SUCCESS") {
            return Ok("tarefa executada com IA".into());
        }
        if task.last_run_status.as_deref() == Some("FAILED") {
            return Err("execução da tarefa falhou".into());
        }
    }
    Err("execução da tarefa excedeu 45s".into())
}

async fn create_routine(
    pool: &SqlitePool,
    workspace_id: &str,
    tag: &str,
) -> AppResult<schedules::Schedule> {
    let input: schedules::CreateScheduleInput = serde_json::from_value(json!({
        "workspace_id": workspace_id,
        "name": format!("{tag} routine"),
        "description": "Teste automático do OpenDesk.",
        "cron_expression": "0 9 * * *",
        "timezone": "UTC",
        "command_or_prompt": "Responda apenas OK",
        "trigger_type": "CRON",
        "target_kind": "PROMPT",
        "router_policy": "COST_BENEFIT",
        "output_folder_template": "output/{YYYY}-{MM}/{schedule_slug}"
    }))
    .map_err(|error| AppError::BadRequest(error.to_string()))?;
    schedules::create(pool, input).await.map_err(Into::into)
}

async fn create_recipe(
    state: &Arc<AppState>,
    workspace_id: &str,
    id: &str,
    mode: SelfTestMode,
) -> Result<String, String> {
    let yaml = format!(
        "id: {id}\nname: {id}\ntrigger:\n  type: MANUAL\n  config: {{}}\nsteps:\n  - id: s1\n    name: Um\n    prompt: Responda apenas OK\n"
    );
    let recipe = parser::parse(&yaml).map_err(|error| error.to_string())?;
    store::save_yaml(&state.db, workspace_id, id, &yaml)
        .await
        .map_err(|error| error.to_string())?;
    store::set_enabled(&state.db, workspace_id, id, true)
        .await
        .map_err(|error| error.to_string())?;
    let run = engine::start(
        crate::app_state::recipe_context(state),
        workspace_id,
        recipe,
        json!({ "manual": true }),
        mode == SelfTestMode::Fast,
    )
    .await
    .map_err(|error| error.to_string())?;
    if mode == SelfTestMode::FullAi {
        for _ in 0..90 {
            tokio::time::sleep(Duration::from_millis(500)).await;
            if let Ok(current) = store::get_run(&state.db, &run.id).await {
                if current.status == "SUCCESS" {
                    return Ok(id.to_string());
                }
                if current.status == "FAILED" {
                    return Err("execução da receita falhou".into());
                }
            }
        }
        return Err("execução da receita excedeu 45s".into());
    }
    Ok(id.to_string())
}

async fn create_script(
    pool: &SqlitePool,
    workspace_id: &str,
    tag: &str,
) -> AppResult<script_tools::ScriptTool> {
    script_tools::create(
        pool,
        script_tools::CreateScriptToolInput {
            workspace_id: Some(workspace_id.to_string()),
            slug: format!("selftest_{}", Uuid::new_v4().simple()),
            name: format!("{tag} script"),
            description: "Teste automático".into(),
            agent_prompt: String::new(),
            runtime: "bash".into(),
            custom_command: None,
            script_body: "echo '{\"ok\":true}'".into(),
            input_schema_json: r#"{"type":"object","properties":{}}"#.into(),
            env_secret_refs_json: "{}".into(),
            timeout_secs: 30,
            require_approval: false,
            enabled: true,
        },
    )
    .await
    .map_err(Into::into)
}

async fn cleanup_all(
    state: &Arc<AppState>,
    workspace_id: &str,
    cleanup: &Cleanup,
) -> Result<(), String> {
    let mut errors = Vec::new();
    if let Some(id) = &cleanup.task {
        let _ = tasks::delete(&state.db, id).await;
    }
    if let Some(id) = &cleanup.schedule {
        let _ = schedules::delete(&state.db, id).await;
    }
    if let Some(id) = &cleanup.recipe {
        let _ = sqlx::query("DELETE FROM recipe_runs WHERE recipe_id = ? AND workspace_id = ?")
            .bind(id)
            .bind(workspace_id)
            .execute(&state.db)
            .await;
        let _ = recipe_files::delete_for_recipe(&state.db, id).await;
        if let Ok(index) = store::get(&state.db, workspace_id, id).await {
            let _ = std::fs::remove_file(&index.file_path);
        }
        if let Err(error) = crate::recipes::runtime::sync_workspace(state, workspace_id).await {
            errors.push(error.to_string());
        }
    }
    if let Some(id) = &cleanup.script {
        let _ = script_tools::delete(&state.db, id).await;
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

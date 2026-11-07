use crate::{
    app_state::AppState,
    db,
    error::{AppError, AppResult},
    io_bus::{IoContext, RunArtifact, RunOutput, outbound},
    recipes::{
        parser::{self, Recipe, RecipeSink},
        store::RecipeRun,
    },
};
use std::sync::Arc;

pub async fn execute(
    state: &Arc<AppState>,
    recipe: &Recipe,
    run: &RecipeRun,
    trigger: &serde_json::Value,
    outputs: &serde_json::Value,
) -> AppResult<Vec<String>> {
    if recipe.sinks.is_empty() {
        return Ok(Vec::new());
    }
    let task = run
        .task_id
        .as_deref()
        .map(str::to_string)
        .ok_or_else(|| AppError::BadRequest("recipe run sem task para sinks".into()))?;
    let artifacts = db::artifacts::list(&state.db, &run.workspace_id, None, Some(&task), None, 500)
        .await
        .unwrap_or_default();
    let latest_run = db::runs::list_with_limit(&state.db, &run.workspace_id, Some(&task), 1)
        .await
        .ok()
        .and_then(|runs| runs.first().cloned());
    let output = RunOutput {
        workspace_id: run.workspace_id.clone(),
        workspace_root: db::workspaces::get(&state.db, &run.workspace_id)
            .await?
            .root_path,
        output_directory: Some(run.output_dir_path.clone()),
        task_id: Some(task),
        run_id: latest_run
            .as_ref()
            .map(|value| value.id.clone())
            .unwrap_or_else(|| run.id.clone()),
        title: recipe.name.clone(),
        summary: latest_run
            .and_then(|value| value.summary_response)
            .unwrap_or_default(),
        artifacts: artifacts
            .into_iter()
            .map(|artifact| RunArtifact {
                file_path: artifact.file_path,
                absolute_path: artifact.absolute_path,
                action_type: artifact.action_type,
            })
            .collect(),
    };
    let ctx = IoContext {
        pool: state.db.clone(),
        paths: state.paths.clone(),
    };
    let run_vars = serde_json::json!({
        "id": run.id,
        "output_dir": run.output_dir_path,
        "date": chrono::Utc::now().format("%Y-%m-%d").to_string(),
    });
    let mut results = Vec::new();
    for (index, sink) in recipe.sinks.iter().enumerate() {
        let sink = render_sink(sink, trigger, &run_vars, outputs)?;
        let transient = db::io_bus::OutboundSink {
            id: format!("recipe:{}:{}", run.id, index),
            workspace_id: run.workspace_id.clone(),
            name: format!("recipe sink {index}"),
            sink_type: sink.sink_type.clone(),
            config_json: sink.config.to_string(),
            is_enabled: true,
            last_run_at: None,
            last_status: None,
            last_error: None,
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        let paths = outbound::execute(&ctx, &transient, &output).await?;
        results.extend(paths);
    }
    Ok(results)
}

fn render_sink(
    sink: &RecipeSink,
    trigger: &serde_json::Value,
    run: &serde_json::Value,
    outputs: &serde_json::Value,
) -> AppResult<RecipeSink> {
    let mut sink = sink.clone();
    sink.sink_type = match sink.sink_type.as_str() {
        "EMAIL_SMTP" => "SMTP_EMAIL".into(),
        "RSYNC_FS" => {
            let mut config = sink.config.as_object().cloned().unwrap_or_default();
            config.insert("transport".into(), serde_json::json!("rsync"));
            sink.config = serde_json::Value::Object(config);
            "ATOMIC_FS".into()
        }
        other => other.to_string(),
    };
    sink.config = render_value(&sink.config, trigger, run, outputs)?;
    Ok(sink)
}

fn render_value(
    value: &serde_json::Value,
    trigger: &serde_json::Value,
    run: &serde_json::Value,
    outputs: &serde_json::Value,
) -> AppResult<serde_json::Value> {
    Ok(match value {
        serde_json::Value::String(text) => {
            serde_json::Value::String(parser::render(text, trigger, run, outputs)?)
        }
        serde_json::Value::Array(items) => serde_json::Value::Array(
            items
                .iter()
                .map(|item| render_value(item, trigger, run, outputs))
                .collect::<AppResult<Vec<_>>>()?,
        ),
        serde_json::Value::Object(map) => serde_json::Value::Object(
            map.iter()
                .map(|(key, value)| Ok((key.clone(), render_value(value, trigger, run, outputs)?)))
                .collect::<AppResult<serde_json::Map<_, _>>>()?,
        ),
        other => other.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_recipe_sink_types_and_renders_config() {
        let sink = RecipeSink {
            sink_type: "RSYNC_FS".into(),
            config: serde_json::json!({
                "destination": "user@host:{{run.output_dir}}",
                "delete": false
            }),
        };
        let rendered = render_sink(
            &sink,
            &serde_json::json!({}),
            &serde_json::json!({"output_dir": "/tmp/out"}),
            &serde_json::json!({}),
        )
        .expect("render");
        assert_eq!(rendered.sink_type, "ATOMIC_FS");
        assert_eq!(rendered.config["transport"], "rsync");
        assert_eq!(rendered.config["destination"], "user@host:/tmp/out");
    }
}

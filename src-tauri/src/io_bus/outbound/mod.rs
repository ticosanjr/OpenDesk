use crate::{
    db::io_bus::OutboundSink,
    error::{AppError, AppResult},
    io_bus::{IoContext, RunOutput},
};
use std::path::Path;

pub mod atomic_fs;
pub mod compiler;
pub mod git;
pub mod messaging;
pub mod rsync;
pub mod s3;
pub mod sql_sink;

pub async fn execute(
    ctx: &IoContext,
    sink: &OutboundSink,
    output: &RunOutput,
) -> AppResult<Vec<String>> {
    let config: serde_json::Value = crate::io_bus::config_value(sink)?;
    crate::io_bus::secrets_guard::validate_no_plaintext_secrets(&config)?;
    match sink.sink_type.as_str() {
        "DOC_COMPILER" => compiler::run(ctx, sink, output).await,
        "ATOMIC_FS" => atomic_fs::run(ctx, sink, output).await,
        "MINIO_S3" => s3::run(ctx, sink, output).await,
        "GIT_COMMIT" => git::run(ctx, sink, output).await,
        "SQL_INSERT" => sql_sink::run(ctx, sink, output).await,
        "SMTP_EMAIL" | "SIGNAL_MSG" | "TELEGRAM_MSG" | "MATRIX_MSG" | "MATTERMOST_MSG" => {
            messaging::run(ctx, sink, output).await
        }
        other => Err(AppError::BadRequest(format!("sink não suportado: {other}"))),
    }
}

pub fn output_root(output: &RunOutput) -> &Path {
    output
        .output_directory
        .as_deref()
        .map(Path::new)
        .unwrap_or_else(|| Path::new(&output.workspace_root))
}

pub fn artifact_paths(output: &RunOutput) -> Vec<&Path> {
    output
        .artifacts
        .iter()
        .filter(|artifact| artifact.action_type != "DELETED")
        .map(|artifact| Path::new(&artifact.absolute_path))
        .filter(|path| path.is_file())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn resolves_output_root_and_artifacts() {
        let output = RunOutput {
            workspace_id: "ws".into(),
            workspace_root: "/tmp/ws".into(),
            output_directory: Some("/tmp/ws/output".into()),
            task_id: None,
            run_id: "run".into(),
            title: "T".into(),
            summary: "S".into(),
            artifacts: Vec::new(),
        };
        assert_eq!(output_root(&output), Path::new("/tmp/ws/output"));
        let fallback = RunOutput {
            output_directory: None,
            ..output
        };
        assert_eq!(output_root(&fallback), Path::new("/tmp/ws"));
        assert!(artifact_paths(&fallback).is_empty());
        let _ = PathBuf::from("x");
    }
}

use crate::{
    db::io_bus::OutboundSink,
    error::{AppError, AppResult},
    io_bus::{IoContext, RunOutput, config_value, outbound::artifact_paths},
};
use serde::Deserialize;
use std::{
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Debug, Deserialize)]
struct Config {
    destination: String,
    #[serde(default)]
    prefix: Option<String>,
}

pub async fn run(
    _ctx: &IoContext,
    sink: &OutboundSink,
    output: &RunOutput,
) -> AppResult<Vec<String>> {
    let config: Config = config_value(sink)?;
    let destination = PathBuf::from(&config.destination);
    std::fs::create_dir_all(&destination)?;
    let mut written = Vec::new();
    for source in artifact_paths(output) {
        let Some(name) = source.file_name() else {
            continue;
        };
        let name = match &config.prefix {
            Some(prefix) => format!("{prefix}{}", name.to_string_lossy()),
            None => name.to_string_lossy().to_string(),
        };
        let target = destination.join(name);
        atomic_copy(source, &target)?;
        written.push(target.to_string_lossy().to_string());
    }
    if written.is_empty() {
        return Err(AppError::BadRequest(
            "ATOMIC_FS não encontrou artefatos para copiar".into(),
        ));
    }
    Ok(written)
}

/// Copy to a temporary file in the destination directory, fsync it and rename.
/// The temp file never crosses filesystems, so the rename stays atomic.
pub fn atomic_copy(source: &Path, target: &Path) -> AppResult<()> {
    let parent = target.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)?;
    let file_name = target
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("artifact");
    let temporary = parent.join(format!(".tmp_{file_name}"));
    {
        let bytes = std::fs::read(source)?;
        let mut file = std::fs::File::create(&temporary)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
    }
    if target.exists() {
        let backup = parent.join(format!(".bak_{file_name}"));
        let _ = std::fs::remove_file(&backup);
        std::fs::rename(target, &backup)?;
        if let Err(error) = std::fs::rename(&temporary, target) {
            let _ = std::fs::rename(&backup, target);
            return Err(error.into());
        }
        let _ = std::fs::remove_file(backup);
    } else {
        std::fs::rename(&temporary, target)?;
    }
    Ok(())
}

#[allow(dead_code)]
pub fn atomic_write(target: &Path, bytes: &[u8]) -> AppResult<()> {
    let parent = target.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)?;
    let file_name = target
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("file");
    let temporary = parent.join(format!(".tmp_{file_name}"));
    {
        let mut file = std::fs::File::create(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    std::fs::rename(&temporary, target)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn run_copies_artifacts_to_destination() {
        let temp = TempDir::new().expect("temp");
        let source = temp.path().join("a.txt");
        std::fs::write(&source, "x").expect("source");
        let output = crate::io_bus::RunOutput {
            workspace_id: "ws".into(),
            workspace_root: temp.path().to_string_lossy().to_string(),
            output_directory: None,
            task_id: None,
            run_id: "run".into(),
            title: "T".into(),
            summary: String::new(),
            artifacts: vec![crate::io_bus::RunArtifact {
                file_path: "a.txt".into(),
                absolute_path: source.to_string_lossy().to_string(),
                action_type: "CREATED".into(),
            }],
        };
        let sink = crate::db::io_bus::OutboundSink {
            id: "s".into(),
            workspace_id: "ws".into(),
            name: "fs".into(),
            sink_type: "ATOMIC_FS".into(),
            config_json: format!(
                "{{\"destination\":\"{}\"}}",
                temp.path().join("out").display()
            ),
            is_enabled: true,
            last_run_at: None,
            last_status: None,
            last_error: None,
            created_at: "now".into(),
        };
        let ctx = crate::io_bus::IoContext {
            pool: crate::db::init(&temp.path().join("db.sqlite"))
                .await
                .expect("db"),
            paths: crate::paths::AppPaths::discover().expect("paths"),
        };
        let written = run(&ctx, &sink, &output).await.expect("run");
        assert!(written[0].ends_with("a.txt"));
    }

    #[test]
    fn atomic_copy_replaces_without_leaving_temp() {
        let temp = TempDir::new().expect("temp");
        let source = temp.path().join("a.txt");
        let target = temp.path().join("out/b.txt");
        std::fs::write(&source, b"novo").expect("source");
        atomic_copy(&source, &target).expect("copy");
        assert_eq!(std::fs::read_to_string(&target).expect("target"), "novo");
        assert!(!temp.path().join("out/.tmp_b.txt").exists());
    }
}

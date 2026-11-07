use crate::{
    error::{AppError, AppResult},
    io_bus::tools,
};
use serde::Deserialize;
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Debug, Clone, Default, Deserialize)]
pub struct RsyncConfig {
    #[serde(default)]
    pub ssh_key_path: Option<String>,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub delete: bool,
    #[serde(default)]
    pub checksum: bool,
    #[serde(default)]
    pub timeout_secs: Option<u64>,
    #[serde(default)]
    pub extra_args: Vec<String>,
}

pub async fn push(
    sources: &[PathBuf],
    destination: &str,
    config: &RsyncConfig,
) -> AppResult<Vec<String>> {
    if sources.is_empty() {
        return Err(AppError::BadRequest(
            "Rsync não encontrou arquivos para enviar".into(),
        ));
    }
    let mut args = base_args(config);
    args.extend(
        sources
            .iter()
            .map(|path| path.to_string_lossy().to_string()),
    );
    args.push(destination.to_string());
    run(&args).await?;
    Ok(vec![destination.to_string()])
}

pub async fn pull(
    source: &str,
    destination: &Path,
    config: &RsyncConfig,
) -> AppResult<Vec<String>> {
    std::fs::create_dir_all(destination)?;
    let mut args = base_args(config);
    let source = if source.ends_with('/') {
        source.to_string()
    } else {
        format!("{source}/")
    };
    args.push(source);
    args.push(format!("{}/", destination.display()));
    run(&args).await?;
    Ok(vec![destination.to_string_lossy().to_string()])
}

fn base_args(config: &RsyncConfig) -> Vec<String> {
    let mut args = vec![
        "-a".to_string(),
        "--protect-args".to_string(),
        "--delay-updates".to_string(),
        "--partial-dir=.rsync-partial".to_string(),
        format!("--timeout={}", config.timeout_secs.unwrap_or(120)),
    ];
    if config.checksum {
        args.push("--checksum".into());
    }
    if config.delete {
        args.push("--delete".into());
    }
    if let Some(key) = config.ssh_key_path.as_deref() {
        let mut ssh = format!("ssh -i {key} -o StrictHostKeyChecking=yes");
        if let Some(port) = config.port {
            ssh.push_str(&format!(" -p {port}"));
        }
        args.push("-e".into());
        args.push(ssh);
    }
    for argument in &config.extra_args {
        if !argument.trim().is_empty() {
            args.push(argument.clone());
        }
    }
    args
}

async fn run(args: &[String]) -> AppResult<()> {
    let output = tokio::time::timeout(
        Duration::from_secs(300),
        tokio::process::Command::new(tools::command("rsync"))
            .args(args)
            .output(),
    )
    .await
    .map_err(|_| AppError::Internal("rsync timeout".into()))?
    .map_err(|error| AppError::BadRequest(format!("rsync indisponível: {error}")))?;
    if !output.status.success() {
        return Err(AppError::Daemon(format!(
            "rsync falhou: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn builds_safe_arguments_without_shell() {
        let args = base_args(&RsyncConfig {
            checksum: true,
            delete: true,
            port: Some(2222),
            ssh_key_path: Some("/tmp/key".into()),
            timeout_secs: Some(30),
            ..Default::default()
        });
        assert!(args.contains(&"--checksum".to_string()));
        assert!(args.contains(&"--delete".to_string()));
        assert!(
            args.iter()
                .any(|arg| arg.contains("StrictHostKeyChecking=yes"))
        );
        assert!(!args.iter().any(|arg| arg.contains("`")));
    }

    #[tokio::test]
    async fn pushes_and_pulls_local_folder() {
        if tools::command("rsync").to_string_lossy() == "rsync"
            && std::process::Command::new("rsync")
                .arg("--version")
                .output()
                .map(|output| !output.status.success())
                .unwrap_or(true)
        {
            return;
        }
        let temp = TempDir::new().expect("temp");
        let source = temp.path().join("source");
        let destination = temp.path().join("destination");
        let pulled = temp.path().join("pulled");
        std::fs::create_dir_all(&source).expect("source");
        std::fs::create_dir_all(&destination).expect("dest");
        std::fs::write(source.join("a.txt"), "a").expect("a");
        let config = RsyncConfig::default();
        push(
            &[source.join("a.txt")],
            destination.to_str().unwrap(),
            &config,
        )
        .await
        .expect("push");
        assert!(destination.join("a.txt").exists());
        pull(destination.to_str().unwrap(), &pulled, &config)
            .await
            .expect("pull");
        assert!(pulled.join("a.txt").exists());
    }
}

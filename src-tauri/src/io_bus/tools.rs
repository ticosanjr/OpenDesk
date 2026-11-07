use crate::{
    error::{AppError, AppResult},
    paths::AppPaths,
};
use serde::Serialize;
use std::{
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Debug, Clone, Copy)]
pub struct ToolSpec {
    pub name: &'static str,
    pub label: &'static str,
    pub purpose: &'static str,
    pub version: &'static str,
    pub optional: bool,
    pub download_url: Option<&'static str>,
    pub archive: bool,
    pub manual_hint: &'static str,
}

pub const TOOLS: &[ToolSpec] = &[
    ToolSpec {
        name: "typst",
        label: "Typst",
        purpose: "Compilar PDFs dos entregáveis.",
        version: "latest",
        optional: true,
        download_url: Some(
            "https://github.com/typst/typst/releases/latest/download/typst-x86_64-unknown-linux-musl.tar.xz",
        ),
        archive: true,
        manual_hint: "Instale typst no PATH ou use o botão de download.",
    },
    ToolSpec {
        name: "tesseract",
        label: "Tesseract OCR",
        purpose: "Extrair texto de imagens e PDFs escaneados.",
        version: "system",
        optional: true,
        download_url: None,
        archive: false,
        manual_hint: "Instale tesseract pelo gerenciador de pacotes do sistema.",
    },
    ToolSpec {
        name: "whisper-cli",
        label: "Whisper.cpp",
        purpose: "Transcrever áudio para texto.",
        version: "system",
        optional: true,
        download_url: None,
        archive: false,
        manual_hint: "Compile o whisper.cpp e coloque whisper-cli no PATH.",
    },
    ToolSpec {
        name: "git",
        label: "Git",
        purpose: "Commit e push automático de entregáveis.",
        version: "system",
        optional: true,
        download_url: None,
        archive: false,
        manual_hint: "Instale git pelo gerenciador de pacotes.",
    },
    ToolSpec {
        name: "rsync",
        label: "Rsync",
        purpose: "Sincronizar pastas locais e remotas (SSH).",
        version: "system",
        optional: true,
        download_url: None,
        archive: false,
        manual_hint: "Instale rsync pelo gerenciador de pacotes.",
    },
    ToolSpec {
        name: "ffprobe",
        label: "FFprobe",
        purpose: "Metadados de vídeo e áudio.",
        version: "system",
        optional: true,
        download_url: None,
        archive: false,
        manual_hint: "Instale ffmpeg (inclui ffprobe).",
    },
];

#[derive(Debug, Clone, Serialize)]
pub struct ToolInfo {
    pub name: String,
    pub label: String,
    pub purpose: String,
    pub version: String,
    pub optional: bool,
    pub installed: bool,
    pub managed: bool,
    pub path: Option<String>,
    pub detected_version: Option<String>,
    pub download_url: Option<String>,
    pub manual_hint: String,
}

pub fn tools_dir() -> PathBuf {
    AppPaths::discover()
        .map(|paths| paths.data_dir.join("tools"))
        .unwrap_or_else(|_| PathBuf::from("tools"))
}

pub fn command(name: &str) -> PathBuf {
    if let Ok(value) = std::env::var(format!(
        "OPENDESK_TOOL_{}",
        name.to_uppercase().replace('-', "_")
    )) && !value.trim().is_empty()
    {
        return PathBuf::from(value);
    }
    let managed = tools_dir().join(name).join("bin").join(name);
    if managed.is_file() {
        return managed;
    }
    PathBuf::from(name)
}

pub fn info(spec: ToolSpec) -> ToolInfo {
    let managed_path = tools_dir().join(spec.name).join("bin").join(spec.name);
    let managed = managed_path.is_file();
    let path = if managed {
        Some(managed_path)
    } else {
        detect_path(spec.name)
    };
    let detected_version = path.as_deref().and_then(version_of);
    ToolInfo {
        name: spec.name.to_string(),
        label: spec.label.to_string(),
        purpose: spec.purpose.to_string(),
        version: spec.version.to_string(),
        optional: spec.optional,
        installed: path.is_some(),
        managed,
        path: path.map(|path| path.to_string_lossy().to_string()),
        detected_version,
        download_url: spec.download_url.map(str::to_string),
        manual_hint: spec.manual_hint.to_string(),
    }
}

pub fn list() -> Vec<ToolInfo> {
    TOOLS.iter().copied().map(info).collect()
}

pub async fn install(name: &str) -> AppResult<ToolInfo> {
    let spec = TOOLS
        .iter()
        .copied()
        .find(|tool| tool.name == name)
        .ok_or_else(|| AppError::NotFound(format!("ferramenta {name}")))?;
    let before = info(spec);
    if before.installed {
        return Ok(before);
    }
    let Some(url) = tool_url(spec) else {
        return Err(AppError::BadRequest(format!(
            "{} não tem download automático. {}",
            spec.label, spec.manual_hint
        )));
    };
    let root = tools_dir().join(spec.name);
    let downloads = tools_dir().join(".downloads");
    std::fs::create_dir_all(&downloads)?;
    let archive = downloads.join(format!(
        "{}-{}.download",
        spec.name,
        chrono::Utc::now().timestamp()
    ));
    let bytes = reqwest::Client::new()
        .get(&url)
        .send()
        .await
        .map_err(|error| AppError::Daemon(format!("download de {}: {error}", spec.label)))?
        .error_for_status()
        .map_err(|error| AppError::Daemon(format!("download de {}: {error}", spec.label)))?
        .bytes()
        .await
        .map_err(|error| AppError::Daemon(format!("download de {}: {error}", spec.label)))?;
    if bytes.len() > 250 * 1024 * 1024 {
        return Err(AppError::BadRequest("download maior que 250 MB".into()));
    }
    if let Ok(expected) = std::env::var(format!(
        "OPENDESK_TOOL_{}_SHA256",
        spec.name.to_uppercase().replace('-', "_")
    )) {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        let actual = hasher
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        if actual != expected.trim().to_lowercase() {
            let _ = std::fs::remove_file(&archive);
            return Err(AppError::BadRequest(format!(
                "checksum de {} não confere",
                spec.label
            )));
        }
    }
    std::fs::write(&archive, &bytes)?;
    let bin_dir = root.join("bin");
    std::fs::create_dir_all(&bin_dir)?;
    if spec.name == "typst" && spec.archive {
        extract_tar_xz(&archive, &root, &bin_dir)?;
    } else {
        let target = bin_dir.join(spec.name);
        std::fs::copy(&archive, &target)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o755))?;
        }
        let _ = std::fs::remove_file(&archive);
    }
    std::fs::write(
        root.join("manifest.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "name": spec.name,
            "url": url,
            "installed_at": chrono::Utc::now().to_rfc3339(),
            "source": "managed",
        }))?,
    )?;
    let after = info(spec);
    if !after.installed {
        return Err(AppError::Internal(format!(
            "{} foi baixado mas o binário não foi encontrado",
            spec.label
        )));
    }
    Ok(after)
}

fn tool_url(spec: ToolSpec) -> Option<String> {
    std::env::var(format!(
        "OPENDESK_TOOL_{}_URL",
        spec.name.to_uppercase().replace('-', "_")
    ))
    .ok()
    .filter(|value| !value.trim().is_empty())
    .or_else(|| spec.download_url.map(str::to_string))
}

fn extract_tar_xz(archive: &Path, root: &Path, bin_dir: &Path) -> AppResult<()> {
    let status = Command::new("tar")
        .args([
            "-xJf",
            &archive.to_string_lossy(),
            "-C",
            &root.to_string_lossy(),
        ])
        .status()
        .map_err(|error| AppError::BadRequest(format!("tar indisponível: {error}")))?;
    if !status.success() {
        return Err(AppError::Internal(
            "falha ao extrair o arquivo .tar.xz".into(),
        ));
    }
    let binary = find_file(root, "typst")
        .ok_or_else(|| AppError::Internal("typst não encontrado no arquivo".into()))?;
    std::fs::copy(&binary, bin_dir.join("typst"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(
            bin_dir.join("typst"),
            std::fs::Permissions::from_mode(0o755),
        )?;
    }
    let _ = std::fs::remove_file(archive);
    Ok(())
}

fn find_file(root: &Path, name: &str) -> Option<PathBuf> {
    for entry in std::fs::read_dir(root).ok()?.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_file(&path, name) {
                return Some(found);
            }
        } else if path.file_name().and_then(|value| value.to_str()) == Some(name) {
            return Some(path);
        }
    }
    None
}

fn detect_path(name: &str) -> Option<PathBuf> {
    if let Ok(value) = std::env::var(format!(
        "OPENDESK_TOOL_{}",
        name.to_uppercase().replace('-', "_")
    )) && Path::new(&value).is_file()
    {
        return Some(PathBuf::from(value));
    }
    let output = Command::new(if cfg!(windows) { "where" } else { "which" })
        .arg(name)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let path = String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()?
        .trim()
        .to_string();
    (!path.is_empty()).then(|| PathBuf::from(path))
}

fn version_of(path: &Path) -> Option<String> {
    let output = Command::new(path).arg("--version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()?
        .trim()
        .to_string();
    (!value.is_empty()).then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_info_handles_missing_binaries() {
        let info = info(TOOLS[1]);
        assert_eq!(info.name, "tesseract");
        assert!(info.manual_hint.contains("tesseract"));
        assert!(find_file(std::path::Path::new("/tmp"), "definitely-missing").is_none());
    }

    #[test]
    fn lists_known_tools_and_optional_flags() {
        let list = list();
        assert!(
            list.iter()
                .any(|tool| tool.name == "typst" && tool.optional)
        );
        assert!(list.iter().any(|tool| tool.name == "git"));
        assert_eq!(
            command("definitely-missing-tool"),
            PathBuf::from("definitely-missing-tool")
        );
    }
}

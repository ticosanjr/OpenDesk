use crate::error::{AppError, AppResult};
use semver::Version;
use serde::Serialize;
use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::{Arc, Mutex},
};

#[derive(Debug, Clone, Serialize)]
pub struct OpenCodeBinaryInfo {
    pub path: String,
    pub version: String,
    pub method: Option<String>,
    pub candidates: Vec<String>,
}

#[derive(Clone)]
pub struct OpenCodeLocator {
    configured: Arc<Mutex<Option<PathBuf>>>,
}

impl OpenCodeLocator {
    pub fn new(configured: Option<PathBuf>) -> Self {
        Self {
            configured: Arc::new(Mutex::new(configured)),
        }
    }

    pub fn set_configured(&self, path: Option<PathBuf>) {
        *self.configured.lock().expect("opencode locator mutex") = path;
    }

    pub fn configured(&self) -> Option<PathBuf> {
        self.configured
            .lock()
            .expect("opencode locator mutex")
            .clone()
    }

    pub fn candidates(&self) -> Vec<PathBuf> {
        let mut candidates = Vec::new();
        let mut push = |path: PathBuf| {
            if !path.as_os_str().is_empty() && !candidates.contains(&path) {
                candidates.push(path);
            }
        };

        if let Some(path) = self.configured() {
            push(path);
        }
        for key in ["OPENCODE_BIN", "OPENDESK_OPENCODE_BIN"] {
            if let Some(path) = std::env::var_os(key) {
                push(PathBuf::from(path));
            }
        }
        if let Some(paths) = std::env::var_os("PATH") {
            for directory in std::env::split_paths(&paths) {
                push(directory.join(exe_name("opencode")));
            }
        }
        if let Some(home) = home_dir() {
            push(home.join(".opencode/bin").join(exe_name("opencode")));
            push(home.join(".local/bin").join(exe_name("opencode")));
            push(home.join(".bun/bin").join(exe_name("opencode")));
            #[cfg(target_os = "macos")]
            push(PathBuf::from("/opt/homebrew/bin").join(exe_name("opencode")));
            #[cfg(target_os = "windows")]
            {
                if let Some(local) = std::env::var_os("LOCALAPPDATA") {
                    push(
                        PathBuf::from(local)
                            .join("opencode/bin")
                            .join(exe_name("opencode")),
                    );
                }
                if let Some(appdata) = std::env::var_os("APPDATA") {
                    push(
                        PathBuf::from(appdata)
                            .join("npm")
                            .join(exe_name("opencode")),
                    );
                }
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            push(PathBuf::from("/usr/local/bin").join(exe_name("opencode")));
            push(PathBuf::from("/usr/bin").join(exe_name("opencode")));
        }

        candidates
    }

    pub fn resolve(&self) -> AppResult<OpenCodeBinaryInfo> {
        let candidates = self.candidates();
        let mut rejected = Vec::new();

        for candidate in &candidates {
            if !candidate.is_file() {
                continue;
            }
            match probe(candidate) {
                Ok(version) if version.major >= 2 => {
                    return Ok(OpenCodeBinaryInfo {
                        path: candidate.to_string_lossy().to_string(),
                        version: version.to_string(),
                        method: install_method(candidate).map(str::to_string),
                        candidates: candidates
                            .iter()
                            .map(|path| path.to_string_lossy().to_string())
                            .collect(),
                    });
                }
                Ok(version) => rejected.push(format!(
                    "{} (versão {version}; OpenDesk exige V2)",
                    candidate.display()
                )),
                Err(error) => rejected.push(format!("{} ({error})", candidate.display())),
            }
        }

        let attempted = candidates
            .iter()
            .map(|path| path.to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join(", ");
        let rejected = if rejected.is_empty() {
            String::new()
        } else {
            format!(" Tentativas: {}.", rejected.join("; "))
        };
        Err(AppError::Daemon(format!(
            "OpenCode V2 não encontrado. Instale o OpenCode ou configure o caminho do binário. Caminhos testados: {attempted}.{rejected}"
        )))
    }

    pub fn resolve_path(&self, path: &Path) -> AppResult<OpenCodeBinaryInfo> {
        if !path.is_file() {
            return Err(AppError::BadRequest(format!(
                "OpenCode binary not found at {}",
                path.display()
            )));
        }
        let version = probe(path)?;
        if version.major < 2 {
            return Err(AppError::BadRequest(format!(
                "OpenDesk requires OpenCode V2; {} reports {version}",
                path.display()
            )));
        }
        Ok(OpenCodeBinaryInfo {
            path: path.to_string_lossy().to_string(),
            version: version.to_string(),
            method: install_method(path).map(str::to_string),
            candidates: self
                .candidates()
                .iter()
                .map(|path| path.to_string_lossy().to_string())
                .collect(),
        })
    }
}

pub fn probe(path: &Path) -> AppResult<Version> {
    let output = Command::new(path)
        .arg("--version")
        .output()
        .map_err(|error| {
            AppError::Daemon(format!("could not execute {}: {error}", path.display()))
        })?;
    if !output.status.success() {
        return Err(AppError::Daemon(format!(
            "{} --version exited with {}",
            path.display(),
            output.status
        )));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    parse_version(&format!("{stdout} {stderr}")).ok_or_else(|| {
        AppError::Daemon(format!(
            "could not parse OpenCode version from {} output",
            path.display()
        ))
    })
}

pub fn parse_version(output: &str) -> Option<Version> {
    output.split_whitespace().find_map(|token| {
        let token = token.trim_matches(|character: char| {
            !character.is_ascii_alphanumeric()
                && character != '.'
                && character != '-'
                && character != '+'
        });
        let token = token.strip_prefix('v').unwrap_or(token);
        Version::parse(token).ok()
    })
}

pub fn install_method(path: &Path) -> Option<&'static str> {
    // Resolve symlinks first: the common workaround
    // `ln -sf ~/.opencode/bin/opencode /usr/local/bin/opencode` would otherwise
    // hide the real install method and block the in-app upgrade.
    let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let value = canonical.to_string_lossy().to_lowercase();
    if value.contains("/.opencode/bin/") {
        Some("curl")
    } else if value.contains("node_modules") || value.contains("/npm/") {
        Some("npm")
    } else if value.contains("/.bun/") {
        Some("bun")
    } else if value.contains("/homebrew/") || value.contains("/cellar/") {
        Some("brew")
    } else {
        None
    }
}

fn exe_name(name: &str) -> String {
    if cfg!(target_os = "windows") {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("USERPROFILE").map(PathBuf::from))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn fake_binary(dir: &std::path::Path, version: &str) -> PathBuf {
        let path = dir.join("opencode");
        let mut file = std::fs::File::create(&path).expect("fake binary");
        writeln!(file, "#!/bin/sh").expect("shebang");
        writeln!(file, "echo \"opencode v{version}\"").expect("version");
        file.sync_all().expect("sync");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
        }
        path
    }

    #[test]
    fn parses_semver_from_version_output() {
        assert_eq!(parse_version("opencode v2.2.5").unwrap().major, 2);
        assert_eq!(parse_version("2.2.5\n").unwrap().minor, 2);
        assert_eq!(parse_version("v2.2.5-beta.1").unwrap().patch, 5);
        assert!(parse_version("no version here").is_none());
        assert!(parse_version("").is_none());
    }

    #[test]
    fn detects_install_methods() {
        assert_eq!(
            install_method(Path::new("/home/u/.opencode/bin/opencode")),
            Some("curl")
        );
        assert_eq!(
            install_method(Path::new("/usr/lib/node_modules/opencode/bin/opencode")),
            Some("npm")
        );
        assert_eq!(
            install_method(Path::new("/home/u/.bun/bin/opencode")),
            Some("bun")
        );
        assert_eq!(
            install_method(Path::new("/opt/homebrew/bin/opencode")),
            Some("brew")
        );
        assert_eq!(install_method(Path::new("/tmp/opencode")), None);
    }

    #[test]
    fn configured_binary_is_the_first_candidate() {
        let temp = tempfile::TempDir::new().expect("temp");
        let path = fake_binary(temp.path(), "2.2.5");
        let locator = OpenCodeLocator::new(Some(path.clone()));
        assert_eq!(locator.candidates().first(), Some(&path));
        assert_eq!(locator.configured(), Some(path));
    }

    #[test]
    fn resolve_path_returns_version_and_rejects_v1() {
        let temp = tempfile::TempDir::new().expect("temp");
        let path = fake_binary(temp.path(), "2.2.5");
        let locator = OpenCodeLocator::new(None);
        let info = locator.resolve_path(&path).expect("v2");
        assert_eq!(info.version, "2.2.5");
        assert_eq!(info.path, path.to_string_lossy());

        let v1 = fake_binary(temp.path(), "1.19.5");
        let error = locator.resolve_path(&v1).expect_err("v1 must be rejected");
        assert!(format!("{error}").contains("requires OpenCode V2"));
    }

    #[test]
    fn resolve_path_reports_missing_files() {
        let locator = OpenCodeLocator::new(None);
        let error = locator
            .resolve_path(Path::new("/nonexistent/opencode"))
            .expect_err("missing");
        assert!(format!("{error}").contains("not found"));
    }

    #[test]
    fn resolve_prefers_the_configured_v2_binary() {
        let temp = tempfile::TempDir::new().expect("temp");
        let path = fake_binary(temp.path(), "2.3.1");
        let locator = OpenCodeLocator::new(Some(path.clone()));
        let info = locator.resolve().expect("resolved");
        assert_eq!(info.path, path.to_string_lossy());
        assert_eq!(info.version, "2.3.1");
    }
}

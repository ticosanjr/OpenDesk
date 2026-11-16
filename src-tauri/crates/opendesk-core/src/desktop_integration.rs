//! Desktop integration for AppImage builds.
//!
//! The AppImage runs without a `.desktop` entry unless the user integrates it
//! manually. Desktop environments (Cinnamon, GNOME, KDE) resolve the taskbar
//! icon through that entry, so an unintegrated AppImage falls back to the
//! generic executable icon even though the window exposes the right
//! `_NET_WM_ICON`. Writing a small user-level entry fixes the taskbar, the
//! application menu and pinned launchers.

use crate::{error::AppError, paths::AppPaths};
use serde::Serialize;
use std::{
    path::{Path, PathBuf},
    process::Command,
};

const DESKTOP_FILE_NAME: &str = "opendesk.desktop";
const ICON_FILE_NAME: &str = "opendesk.png";

#[derive(Debug, Clone, Serialize)]
pub struct DesktopIntegration {
    pub supported: bool,
    pub appimage: Option<String>,
    pub desktop_file: Option<String>,
    pub icon_path: Option<String>,
    pub installed: bool,
    pub message: Option<String>,
}

/// Current integration state without touching the filesystem.
pub fn status(paths: &AppPaths) -> DesktopIntegration {
    let appimage = appimage_path();
    let desktop_dir = applications_dir();
    let desktop_file = desktop_dir
        .as_ref()
        .map(|dir| dir.join(DESKTOP_FILE_NAME))
        .filter(|path| path.is_file());
    let icon_path = paths.data_dir.join(ICON_FILE_NAME);
    DesktopIntegration {
        supported: cfg!(target_os = "linux"),
        installed: desktop_file.is_some(),
        appimage: appimage.map(|path| path.to_string_lossy().to_string()),
        desktop_file: desktop_file.map(|path| path.to_string_lossy().to_string()),
        icon_path: icon_path
            .is_file()
            .then(|| icon_path.to_string_lossy().to_string()),
        message: None,
    }
}

/// Create or refresh the user-level desktop entry for the running AppImage.
pub fn ensure(paths: &AppPaths, version: &str) -> Result<DesktopIntegration, AppError> {
    let mut current = status(paths);
    if !current.supported {
        current.message = Some("integração de atalho disponível apenas no Linux".into());
        return Ok(current);
    }
    let Some(appimage) = appimage_path() else {
        current.message = Some(
            "a integração automática vale para o AppImage; pacotes .deb/.rpm já trazem o atalho"
                .into(),
        );
        return Ok(current);
    };
    let Some(desktop_dir) = applications_dir() else {
        current.message = Some("diretório de aplicações do usuário não encontrado".into());
        return Ok(current);
    };
    let appdir = std::env::var_os("APPDIR").map(PathBuf::from);
    ensure_at(paths, version, &appimage, appdir.as_deref(), &desktop_dir)
}

fn ensure_at(
    paths: &AppPaths,
    version: &str,
    appimage: &Path,
    appdir: Option<&Path>,
    desktop_dir: &Path,
) -> Result<DesktopIntegration, AppError> {
    let icon_path = install_icon(paths, appdir)?;
    std::fs::create_dir_all(desktop_dir).map_err(|error| {
        AppError::Internal(format!(
            "não foi possível criar {}: {error}",
            desktop_dir.display()
        ))
    })?;
    let desktop_file = desktop_dir.join(DESKTOP_FILE_NAME);
    let contents = desktop_entry(appimage, &icon_path, version);
    let changed = std::fs::read_to_string(&desktop_file)
        .map(|existing| existing != contents)
        .unwrap_or(true);
    if changed {
        std::fs::write(&desktop_file, contents).map_err(|error| {
            AppError::Internal(format!(
                "não foi possível escrever {}: {error}",
                desktop_file.display()
            ))
        })?;
        set_mode(&desktop_file, 0o644);
    }
    refresh_desktop_database(desktop_dir);

    Ok(DesktopIntegration {
        supported: true,
        appimage: Some(appimage.to_string_lossy().to_string()),
        desktop_file: Some(desktop_file.to_string_lossy().to_string()),
        icon_path: Some(icon_path.to_string_lossy().to_string()),
        installed: true,
        message: Some(if changed {
            "atalho criado no menu e na barra de tarefas".into()
        } else {
            "atalho já estava atualizado".into()
        }),
    })
}

fn appimage_path() -> Option<PathBuf> {
    std::env::var_os("APPIMAGE")
        .map(PathBuf::from)
        .filter(|path| path.is_file())
}

fn applications_dir() -> Option<PathBuf> {
    directories::BaseDirs::new().map(|dirs| dirs.data_dir().join("applications"))
}

fn install_icon(paths: &AppPaths, appdir: Option<&Path>) -> Result<PathBuf, AppError> {
    let source = appdir
        .and_then(|dir| {
            [
                dir.join(".DirIcon"),
                dir.join("OpenDesk.png"),
                dir.join("opendesk.png"),
            ]
            .into_iter()
            .find(|path| path.is_file())
        })
        .ok_or_else(|| {
            AppError::BadRequest(
                "o AppImage não expôs um ícone em APPDIR; use Reintegrar atalho pelo AppImage"
                    .into(),
            )
        })?;
    let destination = paths.data_dir.join(ICON_FILE_NAME);
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let current = std::fs::read(&destination).ok();
    let source_bytes = std::fs::read(&source)?;
    if current.as_deref() != Some(source_bytes.as_slice()) {
        std::fs::write(&destination, source_bytes)?;
        set_mode(&destination, 0o644);
    }
    Ok(destination)
}

fn desktop_entry(appimage: &Path, icon: &Path, version: &str) -> String {
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=OpenDesk\n\
         Comment=Visual governance for OpenCode\n\
         Exec={} %U\n\
         Icon={}\n\
         Terminal=false\n\
         Categories=Development;Utility;\n\
         StartupWMClass=Opendesk\n\
         X-AppImage-Version={version}\n",
        quote_exec(&appimage.to_string_lossy()),
        icon.to_string_lossy()
    )
}

/// Quote a path for the desktop entry `Exec` key. The specification escapes
/// backslashes, quotes, backticks and dollar signs inside double quotes.
fn quote_exec(path: &str) -> String {
    let mut escaped = String::with_capacity(path.len() + 2);
    escaped.push('"');
    for character in path.chars() {
        if character == '%' {
            escaped.push_str("%%");
            continue;
        }
        if matches!(character, '\\' | '"' | '`' | '$') {
            escaped.push('\\');
        }
        escaped.push(character);
    }
    escaped.push('"');
    escaped
}

fn refresh_desktop_database(dir: &Path) {
    let _ = Command::new("update-desktop-database")
        .arg(dir)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
}

fn set_mode(path: &Path, mode: u32) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode));
    }
    #[cfg(not(unix))]
    let _ = (path, mode);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_entry_matches_the_window_class_and_icon() {
        let entry = desktop_entry(
            Path::new("/opt/Open Desk/OpenDesk.AppImage"),
            Path::new("/home/user/.local/share/opendesk/opendesk.png"),
            "0.52.0",
        );
        assert!(entry.contains("Name=OpenDesk"));
        assert!(entry.contains("Exec=\"/opt/Open Desk/OpenDesk.AppImage\" %U"));
        assert!(entry.contains("Icon=/home/user/.local/share/opendesk/opendesk.png"));
        assert!(entry.contains("StartupWMClass=Opendesk"));
        assert!(entry.contains("X-AppImage-Version=0.52.0"));
    }

    #[test]
    fn ensure_at_writes_entry_and_icon_idempotently() {
        let temp = tempfile::TempDir::new().expect("temp");
        let paths = AppPaths {
            data_dir: temp.path().join("data"),
            config_dir: temp.path().join("config"),
            cache_dir: temp.path().join("cache"),
            log_dir: temp.path().join("logs"),
            db_path: temp.path().join("data/opendesk.db"),
            backups_dir: temp.path().join("backups"),
            artifacts_dir: temp.path().join("artifacts"),
            run_logs_dir: temp.path().join("runs"),
            daemon_lock_path: temp.path().join("daemon.json"),
            api_lock_path: temp.path().join("api.json"),
            allow_temp_workspaces: true,
        };
        std::fs::create_dir_all(&paths.data_dir).expect("data");
        let appdir = temp.path().join("appdir");
        std::fs::create_dir_all(&appdir).expect("appdir");
        std::fs::write(appdir.join(".DirIcon"), b"png-bytes").expect("icon");
        let appimage = temp.path().join("OpenDesk.AppImage");
        std::fs::write(&appimage, b"appimage").expect("appimage");
        let desktop_dir = temp.path().join("applications");

        let first =
            ensure_at(&paths, "0.53.0", &appimage, Some(&appdir), &desktop_dir).expect("first");
        assert!(first.installed);
        let entry = std::fs::read_to_string(desktop_dir.join(DESKTOP_FILE_NAME)).expect("desktop");
        assert!(entry.contains("StartupWMClass=Opendesk"));
        assert!(entry.contains("X-AppImage-Version=0.53.0"));
        assert_eq!(
            std::fs::read(paths.data_dir.join(ICON_FILE_NAME)).expect("icon"),
            b"png-bytes"
        );

        let second =
            ensure_at(&paths, "0.53.0", &appimage, Some(&appdir), &desktop_dir).expect("second");
        assert_eq!(
            second.message.as_deref(),
            Some("atalho já estava atualizado")
        );
    }

    #[test]
    fn quote_exec_escapes_special_characters() {
        assert_eq!(quote_exec("/tmp/a b"), "\"/tmp/a b\"");
        assert_eq!(quote_exec("/tmp/a$b"), "\"/tmp/a\\$b\"");
        assert_eq!(quote_exec("/tmp/a\"b"), "\"/tmp/a\\\"b\"");
        assert_eq!(quote_exec("/tmp/a%b"), "\"/tmp/a%%b\"");
    }
}

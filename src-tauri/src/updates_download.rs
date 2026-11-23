use crate::{
    app_state::AppState,
    error::{AppError, AppResult},
};
use sha2::{Digest, Sha256};
use std::{path::Path, time::Duration};

use super::DownloadResult;
use super::check::{download_asset, fetch_latest, token};
use super::install::{
    find_asset_id, select_asset, spawn_replacer, verify_sha256, write_executable,
};
use super::platform::{
    compare, current_version, installer_kind, manifest_matches_platform, platform_id, running_image,
};

pub async fn download_and_stage(state: &AppState) -> AppResult<DownloadResult> {
    let installer = installer_kind();
    if installer == "development" {
        return Ok(DownloadResult {
            ok: false,
            action: "unsupported".into(),
            path: None,
            message: "Esta é uma build de desenvolvimento; a auto-atualização é desabilitada. Rode `scripts/release.sh` para gerar a versão instalável.".into(),
        });
    }

    let Some(token) = token(&state.secrets) else {
        return Err(AppError::BadRequest(
            "Configure um token do GitHub para baixar a atualização.".into(),
        ));
    };

    if !cfg!(target_os = "linux") {
        return Ok(DownloadResult {
            ok: false,
            action: "unsupported".into(),
            path: None,
            message: "A auto-atualização no app é suportada no Linux. Baixe o instalador da release para atualizar no macOS ou Windows.".into(),
        });
    }

    let manifest = fetch_latest(&token).await?;
    if !manifest_matches_platform(&manifest) {
        return Err(AppError::BadRequest(format!(
            "a release {} é para a plataforma {}; esta instalação é {}",
            manifest.version,
            manifest.platform,
            platform_id()
        )));
    }
    if !compare(&current_version(), &manifest.version) {
        return Ok(DownloadResult {
            ok: false,
            action: "up-to-date".into(),
            path: None,
            message: format!("O OpenDesk já está na versão {}.", manifest.version),
        });
    }

    let asset = select_asset(&manifest, installer).ok_or_else(|| {
        AppError::BadRequest(format!(
            "a release {} não publicou um asset compatível com {}",
            manifest.version, installer
        ))
    })?;

    if asset.sha256.trim().is_empty() {
        return Err(AppError::BadRequest(format!(
            "o asset {} não tem SHA256 no latest.json; publique uma release com scripts/release.sh para habilitar a verificação.",
            asset.name
        )));
    }
    let asset_name = asset.name.clone();
    let expected_sha = asset.sha256.clone();

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(300))
        .user_agent(format!("OpenDesk/{}", current_version()))
        .build()
        .map_err(|error| AppError::Internal(format!("could not build the HTTP client: {error}")))?;
    let asset_id = find_asset_id(&client, &token, &manifest, &asset_name).await?;
    let bytes = download_asset(&client, &token, asset_id).await?;

    let digest = Sha256::digest(&bytes);
    let digest = digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    if !verify_sha256(&digest, &expected_sha) {
        return Err(AppError::Internal(format!(
            "SHA256 mismatch for {}: expected {}, got {}",
            asset_name, expected_sha, digest
        )));
    }

    let staging = state.paths.data_dir.join("updates").join(&manifest.version);
    std::fs::create_dir_all(&staging)?;
    let staged = staging.join(&asset_name);
    write_executable(&staged, &bytes)?;

    if installer == "system" {
        let download_dir = directories::UserDirs::new()
            .and_then(|dirs| dirs.download_dir().map(Path::to_path_buf))
            .unwrap_or_else(|| state.paths.data_dir.clone());
        std::fs::create_dir_all(&download_dir)?;
        let target = download_dir.join(&asset_name);
        std::fs::copy(&staged, &target)?;
        return Ok(DownloadResult {
            ok: true,
            action: "manual-package".into(),
            path: Some(target.to_string_lossy().to_string()),
            message: format!(
                "Pacote salvo em {}. Instale com: sudo dpkg -i {}",
                target.display(),
                target.display()
            ),
        });
    }

    let target = running_image().ok_or_else(|| {
        AppError::Internal("não foi possível resolver o caminho do OpenDesk em execução".into())
    })?;
    if !target.is_file() {
        return Err(AppError::BadRequest(format!(
            "o arquivo em execução ({}) não existe mais; baixe manualmente a release.",
            target.display()
        )));
    }
    let backup = target.with_file_name(format!(
        "{}.bak",
        target
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "opendesk".into())
    ));
    if backup.exists() {
        let _ = std::fs::remove_file(&backup);
    }
    std::fs::copy(&target, &backup)?;
    spawn_replacer(&staged, &target, std::process::id())?;

    Ok(DownloadResult {
        ok: true,
        action: "restart-required".into(),
        path: Some(target.to_string_lossy().to_string()),
        message: format!(
            "Atualização {} baixada e verificada. Encerre o OpenDesk (inclusive pelo ícone da bandeja); o binário será substituído e o app reaberto automaticamente. Backup: {}.",
            manifest.version,
            backup.display()
        ),
    })
}

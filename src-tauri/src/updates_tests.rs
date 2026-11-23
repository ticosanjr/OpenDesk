use super::install::{select_asset, verify_sha256};
use super::platform::{REPO, compare, manifest_matches_platform};
use super::*;

fn manifest(assets: Vec<(&str, &str)>) -> LatestManifest {
    LatestManifest {
        version: "0.7.0".into(),
        tag: "v0.7.0".into(),
        repo: REPO.into(),
        platform: platform_id(),
        published_at: String::new(),
        notes: String::new(),
        assets: assets
            .into_iter()
            .map(|(name, sha)| UpdateAsset {
                name: name.into(),
                size: 0,
                sha256: sha.into(),
            })
            .collect(),
    }
}

#[test]
fn selects_assets_per_installer() {
    let value = manifest(vec![
        ("OpenDesk_0.7.0_amd64.AppImage", "a"),
        ("OpenDesk_0.7.0_amd64.deb", "b"),
        ("opendesk-linux-x86_64", "c"),
    ]);
    assert_eq!(
        select_asset(&value, "appimage").unwrap().name,
        "OpenDesk_0.7.0_amd64.AppImage"
    );
    assert_eq!(
        select_asset(&value, "system").unwrap().name,
        "OpenDesk_0.7.0_amd64.deb"
    );
    assert_eq!(
        select_asset(&value, "portable").unwrap().name,
        "opendesk-linux-x86_64"
    );
}

#[test]
fn portable_asset_must_match_the_current_platform() {
    let value = manifest(vec![
        ("opendesk-macos-arm64", "a"),
        ("opendesk-windows-x86_64.exe", "b"),
        ("opendesk-linux-x86_64", "c"),
    ]);
    let selected = select_asset(&value, "portable");
    if cfg!(target_os = "linux") {
        assert_eq!(selected.unwrap().name, "opendesk-linux-x86_64");
    } else {
        assert!(selected.is_none());
    }
}

#[test]
fn rejects_manifests_from_other_platforms() {
    let mut value = manifest(vec![("opendesk-linux-x86_64", "c")]);
    assert!(manifest_matches_platform(&value));
    value.platform = "macos-arm64".into();
    assert!(!manifest_matches_platform(&value));
    value.platform.clear();
    assert!(manifest_matches_platform(&value));
}

#[test]
fn compares_versions_with_and_without_prefix() {
    assert!(compare("0.6.0", "0.7.0"));
    assert!(compare("0.6.0", "v0.7.0"));
    assert!(!compare("0.7.0", "0.7.0"));
    assert!(!compare("0.7.0", "0.6.9"));
    assert!(!compare("dev", "0.7.0"));
}

#[test]
fn development_build_skips_asset_selection() {
    let value = manifest(vec![("opendesk-linux-x86_64", "c")]);
    assert!(select_asset(&value, "development").is_none());
}

#[test]
fn verifies_sha256_case_insensitively() {
    assert!(verify_sha256("abc123", "ABC123"));
    assert!(verify_sha256("abc123", " abc123 "));
    assert!(!verify_sha256("abc123", "abc124"));
    assert!(!verify_sha256("abc123", ""));
}

#[test]
fn parses_latest_manifest() {
    let raw = r#"{
        "version": "0.6.0",
        "tag": "v0.6.0",
        "repo": "ticosanjr/OpenDesk",
        "platform": "linux-x86_64",
        "published_at": "2026-11-06T00:00:00Z",
        "notes": "release",
        "assets": [
            {"name": "OpenDesk_0.6.0_amd64.AppImage", "size": 10, "sha256": "aa"}
        ]
    }"#;
    let manifest: LatestManifest = serde_json::from_str(raw).expect("manifest");
    assert_eq!(manifest.version, "0.6.0");
    assert_eq!(manifest.assets.len(), 1);
    assert_eq!(
        select_asset(&manifest, "appimage").unwrap().name,
        "OpenDesk_0.6.0_amd64.AppImage"
    );
}

#[test]
fn exposes_the_build_version_and_development_installer() {
    assert_eq!(current_version(), env!("CARGO_PKG_VERSION"));
    assert_eq!(installer_kind(), "development");
}

#[test]
fn selects_nothing_for_unknown_installers_or_empty_manifests() {
    let empty = manifest(Vec::new());
    assert!(select_asset(&empty, "appimage").is_none());
    let value = manifest(vec![("opendesk-linux-x86_64", "c")]);
    assert_eq!(
        select_asset(&value, "unknown").expect("fallback").name,
        "opendesk-linux-x86_64"
    );
    assert!(select_asset(&value, "portable").is_some());
}

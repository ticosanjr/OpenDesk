use super::*;
use std::path::PathBuf;

#[test]
fn enriched_path_contains_the_binary_directory() {
    let value = enriched_path("/home/user/.opencode/bin/opencode");
    let paths = std::env::split_paths(&value).collect::<Vec<_>>();
    assert!(paths.contains(&PathBuf::from("/home/user/.opencode/bin")));
    assert!(paths.contains(&PathBuf::from("/usr/bin")));
}

#[test]
fn find_free_port_stays_in_the_daemon_range() {
    let port = find_free_port().expect("free port");
    assert!((DEFAULT_PORT..=LAST_PORT).contains(&port));
}

#[test]
fn service_state_paths_target_opencode_service_json() {
    let paths = service_state_paths();
    assert!(!paths.is_empty());
    assert!(
        paths
            .iter()
            .all(|path| path.ends_with("opencode/service.json"))
    );
}

//! Headless OpenDesk control plane. The shell library is built without the
//! `desktop` feature, so this binary does not link Tauri.
fn main() {
    opendesk_lib::run_daemon();
}

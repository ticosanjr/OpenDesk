//! Headless control-plane daemon. It reuses the same core state and API as the
//! desktop app, but does not create a window or tray.

fn main() {
    opendesk_lib::run_daemon();
}

// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if std::env::args().any(|argument| argument == "mcp-google") {
        opendesk_lib::run_mcp_google();
    } else if std::env::args().any(|argument| argument == "--headless") {
        opendesk_lib::run_headless();
    } else {
        opendesk_lib::run();
    }
}

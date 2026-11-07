// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if std::env::args().any(|argument| argument == "doctor") {
        let json = std::env::args().any(|argument| argument == "--json");
        let fix_daemons = std::env::args().any(|argument| argument == "--fix-daemons");
        opendesk_lib::run_doctor(json, fix_daemons);
    } else if std::env::args().any(|argument| argument == "mcp-config") {
        opendesk_lib::run_mcp_config();
    } else if std::env::args().any(|argument| argument == "mcp-google") {
        opendesk_lib::run_mcp_google();
    } else if std::env::args().any(|argument| argument == "--headless") {
        opendesk_lib::run_headless();
    } else {
        opendesk_lib::run();
    }
}

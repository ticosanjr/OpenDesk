// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let cli_args = std::env::args().skip(1).collect::<Vec<_>>();
    if cli_args.first().is_some_and(|argument| {
        matches!(
            argument.as_str(),
            "status"
                | "workspaces"
                | "tasks"
                | "runs"
                | "queue"
                | "backups"
                | "export"
                | "--help"
                | "-h"
        )
    }) {
        opendesk_lib::run_cli(cli_args);
    } else if cli_args.first().is_some_and(|argument| {
        argument == "keygen-update" || argument == "sign-update" || argument == "verify-update"
    }) {
        opendesk_lib::run_update_signing(cli_args);
    } else if std::env::args().any(|argument| argument == "doctor") {
        let json = std::env::args().any(|argument| argument == "--json");
        let fix_daemons = std::env::args().any(|argument| argument == "--fix-daemons");
        let live = std::env::args().any(|argument| argument == "--live");
        opendesk_lib::run_doctor(json, fix_daemons, live);
    } else if std::env::args().any(|argument| argument == "mcp-config") {
        opendesk_lib::run_mcp_config();
    } else if std::env::args().any(|argument| argument == "mcp-control") {
        opendesk_lib::run_mcp_control();
    } else if std::env::args().any(|argument| argument == "mcp-google") {
        opendesk_lib::run_mcp_google();
    } else if std::env::args().any(|argument| argument == "--headless") {
        opendesk_lib::run_headless();
    } else {
        opendesk_lib::run();
    }
}

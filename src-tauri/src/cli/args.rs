//! Argument parsing and shared helpers for the headless CLI.

use std::collections::HashMap;

pub(super) const HELP: &str = "OpenDesk CLI

Uso:
  opendesk status [--json]
  opendesk workspaces list [--json]
  opendesk tasks list [--workspace <id>] [--json]
  opendesk tasks create --title <título> --description <descrição>
        [--agent <id>] [--model <ref>] [--mode <modo>] [--priority <1-4>]
        [--tags a,b] [--workspace <id>] [--json]
  opendesk tasks run <task-id> [--json]
  opendesk chat [mensagem] [--workspace <id>] [--thread <id>] [--agent <id>] [--json]
  opendesk approvals list [--workspace <id>] [--json]
  opendesk approvals resolve <id> approve|deny [texto] [--json]
  opendesk diff <run-id> [--json]
  opendesk nodes list [--json]
  opendesk tui [--workspace <id>]
  opendesk node run --central <url> --token <token> [--once] [--poll <segundos>]
        [--opencode-url <url>] [--opencode-password <senha>]
        [--sync] [--workdir <diretório>]
  opendesk runs list [--workspace <id>] [--limit <n>] [--json]
  opendesk runs show <run-id> [--json]
  opendesk queue list [--workspace <id>] [--json]
  opendesk backups list [--workspace <id>] [--json]
  opendesk backups create [--workspace <id>] [--json]
  opendesk backups restore <backup-id> --path <diretório> [--name <nome>]
  opendesk backups download <backup-id> --out <arquivo>
  opendesk backups delete <backup-id>
  opendesk config export <arquivo.json> [--workspace <id>] [--sections a,b]
  opendesk config import <arquivo.json> [--workspace <id>] [--dry-run]
        [--mode merge-skip|merge-overwrite|replace]
  opendesk export <run-id> [--format md|html|json] [--out <arquivo>]

O servidor local precisa estar rodando (app desktop ou --headless).";

#[derive(Default)]
pub(super) struct Args {
    pub(super) positionals: Vec<String>,
    pub(super) flags: HashMap<String, String>,
    pub(super) json: bool,
    pub(super) help: bool,
}

pub(super) fn parse(args: &[String]) -> Args {
    let mut parsed = Args::default();
    let mut index = 0;
    while index < args.len() {
        let argument = &args[index];
        if argument == "--json" {
            parsed.json = true;
        } else if argument == "--help" || argument == "-h" {
            parsed.help = true;
        } else if let Some(rest) = argument.strip_prefix("--") {
            if let Some((key, value)) = rest.split_once('=') {
                parsed.flags.insert(key.to_string(), value.to_string());
            } else if index + 1 < args.len() && !args[index + 1].starts_with("--") {
                parsed
                    .flags
                    .insert(rest.to_string(), args[index + 1].clone());
                index += 1;
            } else {
                parsed.flags.insert(rest.to_string(), "true".into());
            }
        } else {
            parsed.positionals.push(argument.clone());
        }
        index += 1;
    }
    parsed
}

pub(super) fn shorten(id: &str) -> &str {
    id.get(..8).unwrap_or(id)
}

pub(super) fn workspace_query(args: &Args) -> String {
    args.flags
        .get("workspace")
        .map(|id| format!("?workspace_id={}", urlencoding::encode(id)))
        .unwrap_or_default()
}

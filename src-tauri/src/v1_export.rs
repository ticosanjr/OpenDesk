//! `opendesk migrate-v1-export`: export a V1 database from the desktop binary.
//!
//! The AppImage ships `opendesk` and `opendeskd` but not the separate CLI, so
//! the same export must be reachable from the desktop binary.

use crate::paths::AppPaths;
use std::{path::PathBuf, process::exit};

const HELP: &str =
    "uso: opendesk migrate-v1-export [--db <v1.db>] [--out <dir>] [--format json|yaml]";

pub fn run(args: Vec<String>) {
    if let Err(error) = execute(&args) {
        eprintln!("migrate-v1-export: {error}\n{HELP}");
        exit(1);
    }
}

fn execute(args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{HELP}");
        return Ok(());
    }
    let mut db: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut yaml = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--db" => {
                index += 1;
                db = Some(PathBuf::from(
                    args.get(index).ok_or("--db exige um caminho")?,
                ));
            }
            "--out" => {
                index += 1;
                out = Some(PathBuf::from(
                    args.get(index).ok_or("--out exige um diretório")?,
                ));
            }
            "--format" => {
                index += 1;
                yaml = match args.get(index).map(String::as_str) {
                    Some("json") => false,
                    Some("yaml") => true,
                    Some(other) => return Err(format!("formato inválido '{other}' (json|yaml)")),
                    None => return Err("--format exige json ou yaml".into()),
                };
            }
            other => return Err(format!("argumento desconhecido '{other}'")),
        }
        index += 1;
    }
    let paths = AppPaths::discover().map_err(|error| format!("{error:#}"))?;
    let db = db.unwrap_or_else(|| paths.db_path.clone());
    let out = out.unwrap_or_else(|| {
        paths.data_dir.join(format!(
            "v1-export-{}",
            chrono::Utc::now().format("%Y%m%d-%H%M%S")
        ))
    });
    let runtime = tokio::runtime::Runtime::new().map_err(|error| error.to_string())?;
    let summary = runtime
        .block_on(opendesk_storage::legacy::export_v1(&db, &out, yaml))
        .map_err(|error| format!("{error:#}"))?;
    println!(
        "exportado: {} workspace(s), {} receita(s), {} setting(s) em {}",
        summary.workspaces,
        summary.recipes,
        summary.settings,
        summary.out.display()
    );
    Ok(())
}
